use std::{
  cell::UnsafeCell,
  hint::cold_path,
  marker::PhantomPinned,
  mem::ManuallyDrop,
  ops::BitAnd,
  os::raw::c_void,
  ptr::addr_of,
  sync::atomic::{AtomicUsize, Ordering},
  task::{Poll, RawWakerVTable, Waker},
};

use crate::{
  FFISafe,
  I_DECLARE_THAT_I_AND_MY_CODEBASE_IS_FFI_SAFE_AND_THAT_UNDEFINED_BEHAVIOUR_ARISING_DUE_TO_DECLARING_MY_TYPES_FFI_SAFE_DOES_NOT_CONDONE_THE_SAFETY_AND_SECURITY_OF_THIS_PROJECT,
};

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
  pub wake_and_free: unsafe extern "C" fn(CWaker),
  pub wake_no_free: unsafe extern "C" fn(CWaker),
  pub waker_clone: unsafe extern "C" fn(CWaker) -> CWaker,
  pub free_waker: unsafe extern "C" fn(CWaker),
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct CWaker {
  data: *const (),
  vtable: *const c_void,
}

extern "C" fn call_no_drop(waker: CWaker) {
  unsafe {
    let waker = ManuallyDrop::new(Waker::new(
      waker.data,
      &*(waker.vtable as *const RawWakerVTable),
    ));
    waker.wake_by_ref();
  }
}

extern "C" fn call_drop(waker: CWaker) {
  unsafe {
    let waker = Waker::new(waker.data, &*(waker.vtable as *const RawWakerVTable));

    waker.wake();
  }
}

extern "C" fn drop_cwaker(waker: CWaker) {
  unsafe {
    drop(Waker::new(
      waker.data,
      &*(waker.vtable as *const RawWakerVTable),
    ));
  }
}

extern "C" fn clone_waker(waker: CWaker) -> CWaker {
  unsafe {
    let waker = ManuallyDrop::new(Waker::new(
      waker.data,
      &*(waker.vtable as *const RawWakerVTable),
    ));

    let newwaker = ManuallyDrop::new(waker.clone());

    CWaker {
      data: newwaker.data(),
      vtable: newwaker.vtable() as *const _ as _,
    }
  }
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

unsafe impl<T: FFISafe> FFISafe for FutureTask<T> {
  fn i_am_ffisafe() -> crate::IAmFFISafe {
    I_DECLARE_THAT_I_AND_MY_CODEBASE_IS_FFI_SAFE_AND_THAT_UNDEFINED_BEHAVIOUR_ARISING_DUE_TO_DECLARING_MY_TYPES_FFI_SAFE_DOES_NOT_CONDONE_THE_SAFETY_AND_SECURITY_OF_THIS_PROJECT
  }
}

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

  last_waker_data: AtomicUsize,
  last_waker_vtable: AtomicUsize,

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
      last_waker_data: AtomicUsize::new(0),
      last_waker_vtable: AtomicUsize::new(0),
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
    if { self.last_waker_data.load(Ordering::Relaxed) != data_ptr.addr() } || {
      self.last_waker_vtable.load(Ordering::Relaxed) != vtable_ptr.addr()
    } {
      {
        self
          .last_waker_data
          .store(data_ptr.addr(), Ordering::Relaxed);
        self
          .last_waker_vtable
          .store(vtable_ptr.addr(), Ordering::Relaxed);
      }

      let new_internal = ManuallyDrop::new(waker.clone());

      (self.task._cb)(
        self.task._state,
        CBReason::Waker {
          waker: CWaker {
            data: new_internal.data(),
            vtable: new_internal.vtable() as *const _ as _,
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
