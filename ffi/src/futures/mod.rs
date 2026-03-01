use std::{
  cell::UnsafeCell,
  hint::cold_path,
  marker::PhantomPinned,
  mem::transmute,
  ops::BitAnd,
  os::raw::c_void,
  ptr::{addr_of, null},
  task::{Poll, Waker},
};

use crate::FFISafe;

pub mod atomiccw;
pub mod implements;

pub type State = *mut c_void;

#[repr(C)]
pub enum CBReason {
  /// It is expected to make NO asynchronous progress
  /// during this stage as otherwise, it will make things messy
  SealWakerVTable {
    /// Please make sure to copy the data from here!!
    vtable: *const WakerVTable,
  },

  PollCollect,
  /// Function to call to wake it up
  ///
  /// ## Please Note
  /// You must also correctly handle deallocation by using the provided methods above
  /// to responsibly call the correct method
  Waker {
    waker: CWaker,
  },

  Abort,
  Cleanup,
}

#[repr(C)]
pub struct WakerVTable {
  wake_and_free: extern "C" fn(CWaker),
  wake_no_free: extern "C" fn(*const CWaker),
  waker_clone: extern "C" fn(*const CWaker) -> CWaker,
  free_waker: extern "C" fn(CWaker),
}

#[repr(C, align(0x8))]
#[derive(Debug)]
pub struct CWaker {
  /// This stores a rust structure
  _unknown: [u8; 16],
}

impl CWaker {
  pub unsafe fn unsafe_bitcopy(&self) -> Self {
    CWaker {
      _unknown: self._unknown,
    }
  }
}

pub struct CWakerInternal {
  waker: Waker,
}

const _WAKER_OK1: () = assert!(align_of::<CWaker>() == align_of::<CWakerInternal>());
const _WAKER_OK2: () = assert!(size_of::<CWaker>() == size_of::<CWakerInternal>());
const _WAKER_OK3: () = assert!(size_of::<CWaker>() == 0x10);
const _WAKER_OK4: () = assert!(align_of::<CWaker>() == 0x8);

extern "C" fn call_no_drop(data: *const CWaker) {
  unsafe {
    transmute::<&CWaker, &CWakerInternal>(&*data)
      .waker
      .wake_by_ref();
  }
}

extern "C" fn call_drop(data: CWaker) {
  unsafe {
    transmute::<CWaker, CWakerInternal>(data).waker.wake();
  }
}

extern "C" fn drop_cwaker(data: CWaker) {
  unsafe {
    drop(transmute::<CWaker, CWakerInternal>(data).waker);
  }
}

extern "C" fn clone_waker(data: *const CWaker) -> CWaker {
  unsafe { transmute(transmute::<&CWaker, &CWakerInternal>(&*data).waker.clone()) }
}

#[repr(C)]
pub enum MaybeData<T> {
  None,
  Some(T),
}

#[repr(C)]
pub struct Result<T: FFISafe> {
  flag: u8,

  /// Case A:
  /// If the flag is not 0
  /// It means that Future had been collected before
  ///
  /// Case B:
  /// If the flag is 0
  /// It means the Future is pending completion
  ///
  /// For CaseB, a new waker is sent via channel
  /// shortly
  output: MaybeData<T>,
}

#[repr(C)]
pub struct FutureTask<T: FFISafe> {
  pub _state: State,

  /// This is the function you're supposed to correctly handle!
  ///
  /// Return NULL once it has been consumed & When data is not available
  pub _cb: extern "C" fn(State, CBReason) -> Result<T>,
}

unsafe impl<T: FFISafe> FFISafe for FutureTask<T> {}

static WAKER_VTABLE: WakerVTable = WakerVTable {
  wake_and_free: call_drop,
  wake_no_free: call_no_drop,
  free_waker: drop_cwaker,
  waker_clone: clone_waker,
};

#[repr(C, align(64))]
pub struct FFIFuture<T: FFISafe + Sized> {
  task: FutureTask<T>,
  flags: UnsafeCell<u8>,

  last_waker_data: UnsafeCell<*const ()>,
  last_waker_vtable: UnsafeCell<*const ()>,

  _pin: std::marker::PhantomPinned,
}

impl<T: FFISafe + Sized> FFIFuture<T> {
  pub fn new(task: FutureTask<T>) -> Self {
    (task._cb)(
      task._state,
      CBReason::SealWakerVTable {
        vtable: addr_of!(WAKER_VTABLE),
      },
    );

    Self {
      last_waker_data: UnsafeCell::new(null()),
      last_waker_vtable: UnsafeCell::new(null()),
      flags: UnsafeCell::new(0),
      task,
      _pin: PhantomPinned,
    }
  }
}

impl<T: FFISafe + Sized> Future for FFIFuture<T> {
  type Output = T;

  fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
    let waker = cx.waker();

    // 1. Extract the raw waker parts manually.
    // A Waker is effectively: struct { data: *const (), vtable: *const () }
    let (data_ptr, vtable_ptr) = (waker.data(), waker.vtable() as *const _ as *const ());

    // 2. The "Fingerprint" Check
    // This is essentially what will_wake does, but without the function call.
    if unsafe { *self.last_waker_data.get() != data_ptr }
      || unsafe { *self.last_waker_vtable.get() != vtable_ptr }
    {
      unsafe {
        *self.last_waker_data.get() = data_ptr;
        *self.last_waker_vtable.get() = vtable_ptr;
      }

      (self.task._cb)(
        self.task._state,
        CBReason::Waker {
          waker: unsafe {
            transmute(CWakerInternal {
              waker: waker.clone(),
            })
          },
        },
      );
    }

    let out = (self.task._cb)(self.task._state, CBReason::PollCollect {});

    if let MaybeData::Some(out) = out.output {
      unsafe {
        *self.flags.get() |= 1 << 2;
      }

      return Poll::Ready(out);
    }

    if out.flag != 0 {
      panic!("[ERR] ASYNCHRONOUS GLITCHING AT CALLING FFI ASYNC FUNCTION");
    }

    Poll::Pending
  }
}

impl<T: FFISafe + Sized> Drop for FFIFuture<T> {
  fn drop(&mut self) {
    if self.flags.get_mut().bitand(1 << 2) != 0 {
      cold_path();
      (self.task._cb)(self.task._state, CBReason::Cleanup);
    } else {
      (self.task._cb)(self.task._state, CBReason::Abort);
    }
  }
}
