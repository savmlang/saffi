use std::{
  ffi::c_void,
  pin::Pin,
  ptr::null,
  task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
};

use crate::{
  FFISafe,
  futures::{CBReason, FutureTask, MaybeData, Result, atomiccw::AtomicFFICWaker},
};

#[repr(C, align(64))]
struct FutureState<F: Future> {
  // Our high-speed, lock-free waker state (Inlined!)
  waker_atomic: AtomicFFICWaker,

  // The actual task
  future: Option<Pin<Box<F>>>,
  // The cached Rust Waker handle
  raw_waker: Option<Waker>,
}

impl<F: Future> FutureState<F> {
  const VTABLE: RawWakerVTable = RawWakerVTable::new(
    Self::clone_waker,
    Self::wake_consume,
    Self::wake_by_reference,
    Self::drop_waker,
  );

  // Literally a zero-operation
  //
  // Clones the Arc and hands it over to them!
  unsafe fn clone_waker(state_ptr: *const ()) -> RawWaker {
    // Since the AtomicFFICWaker is inlined, we just pass the pointer.
    // If you need ref-counting for the Task itself, increment it here.
    unsafe { (&*(state_ptr as *const AtomicFFICWaker)).inc() };
    RawWaker::new(state_ptr, &Self::VTABLE)
  }

  // NOTE:
  // This does not actually consume unlike it seems to be to improve performance, but
  // it does de-ref the Waker
  unsafe fn wake_consume(ptr: *const ()) {
    let state = unsafe { &*(ptr as *const AtomicFFICWaker) };
    state.wake();

    if state.dec() {
      drop(unsafe { Box::from_raw(ptr as *mut FutureState<F>) });
    }
  }

  unsafe fn wake_by_reference(ptr: *const ()) {
    let state = unsafe { &*(ptr as *const AtomicFFICWaker) };
    state.wake();
  }

  unsafe fn drop_waker(ptr: *const ()) {
    let state = unsafe { &*(ptr as *const AtomicFFICWaker) };

    if state.dec() {
      drop(unsafe { Box::from_raw(ptr as *mut FutureState<F>) });
    }
  }
}

pub fn create_future<F: Future>(fut: F) -> FutureTask<F::Output>
where
  F::Output: FFISafe,
{
  FutureTask {
    #[allow(invalid_value)]
    _state: Box::into_raw(Box::new(FutureState {
      future: Some(Box::pin(fut)),
      raw_waker: None,
      waker_atomic: AtomicFFICWaker::new(null()),
    })) as _,
    _cb: poll_future::<F>,
  }
}

extern "C" fn poll_future<F: Future>(state_ptr: *mut c_void, action: CBReason) -> Result<F::Output>
where
  F::Output: FFISafe,
{
  let state = unsafe { &mut *(state_ptr as *mut FutureState<F>) };

  match action {
    CBReason::SealWakerVTable { vtable } => {
      state.waker_atomic.set_vtable(vtable);
      state.waker_atomic.inc();

      // Construct the Waker once.
      let w = unsafe {
        Waker::from_raw(RawWaker::new(
          &state.waker_atomic as *const _ as *const (),
          &FutureState::<F>::VTABLE,
        ))
      };
      state.raw_waker = Some(w);
    }
    CBReason::Waker { waker } => {
      state.waker_atomic.update(waker);
    }
    CBReason::Cleanup | CBReason::Abort => unsafe {
      _ = state.future.take();

      // The drop is auto handled by rust Waker's drop
      _ = state.raw_waker.take();

      if state.waker_atomic.dec() {
        drop(Box::from_raw(state as *mut FutureState<F>));
      }
    },
    CBReason::PollCollect => {
      if let Some(x) = state.future.as_mut() {
        let waker = unsafe { state.raw_waker.as_ref().unwrap_unchecked() };
        let mut ctx = Context::from_waker(waker);

        match x.as_mut().poll(&mut ctx) {
          Poll::Ready(x) => {
            return Result {
              flag: 0,
              output: MaybeData::Some(x),
            };
          }
          Poll::Pending => {
            return Result {
              flag: 0,
              output: MaybeData::None,
            };
          }
        }
      }
    }
  }

  Result {
    flag: 0,
    output: MaybeData::None,
  }
}
