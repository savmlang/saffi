use core::{ffi::c_void, ptr::null_mut};

use std::{sync::Arc, task::Waker};

use tokio::{
  spawn,
  sync::{
    Mutex,
    mpsc::{Sender, channel},
  },
  task::JoinHandle,
};

use crate::{
  FFISafe,
  boxed::{RTBox, RTSafeBoxWrapper},
  futures::{FFIWaker, FutureTask, WakerData},
};

struct FutState {
  output: Option<SafeContainer<*mut RTSafeBoxWrapper>>,
  finished: bool,
}

struct StateData {
  fut: Arc<Mutex<FutState>>,
  tx: Sender<FFIWaker>,
}

#[repr(transparent)]
struct SafeContainer<T>(T);

unsafe impl<T> Send for SafeContainer<T> {}
unsafe impl<T> Sync for SafeContainer<T> {}

extern "C" fn use_state(ptr: *mut c_void) -> *mut RTSafeBoxWrapper {
  let mut out = null_mut() as *mut _;
  unsafe {
    let data = Box::from_raw(ptr as *mut StateData);

    if let Ok(x) = data.fut.try_lock() {
      if let Some(x) = x.output.as_ref() {
        out = x.0;
      }
    }

    // No output returned, do not destroy state data
    _ = Box::into_raw(data);
  }

  out
}

extern "C" fn use_ready(ptr: *mut c_void) -> bool {
  let mut out = false;
  unsafe {
    let data = Box::from_raw(ptr as *mut StateData);

    if let Ok(x) = data.fut.try_lock() {
      out = x.finished;
    }

    _ = Box::into_raw(data);
  }

  out
}

extern "C" fn waker(ptr: *mut c_void, waker: *mut WakerData) {
  unsafe {
    let data = Box::from_raw(ptr as *mut StateData);

    _ = data.tx.try_send(FFIWaker::use_waker(waker));

    _ = Box::into_raw(data);
  }
}

extern "C" fn clean_state(ptr: *mut c_void) {
  unsafe {
    drop(Box::from_raw(ptr as *mut StateData));
  }
}

struct FutureState<F: Future> {
  future: Pin<Box<F>>,
  result: Option<F::Output>,
  // We store the waker here so the bridge can update it on every poll
  waker: Option<Waker>,
}

pub fn create_future<T: FFISafe + 'static>(fut: JoinHandle<RTBox<T>>) -> FutureTask {
  let local_state = Arc::new(Mutex::new(FutState {
    output: None,
    finished: false,
  }));
  let (tx, mut rx) = channel::<FFIWaker>(10);

  let state = local_state.clone();

  spawn(async move {
    let mut hwnd = fut;

    let mut waker: Option<FFIWaker> = None;
    loop {
      tokio::select! {
        res = &mut hwnd => {
          let mut lock = state.lock().await;
          lock.finished = true;
          lock.output = Some(SafeContainer(res.expect("JIT execution panic").into_raw()));
          break;
        }
        wk = rx.recv() => {
          waker = wk;
        }
      }
    }

    if let Some(x) = waker {
      x.call();
    }

    // Prevent Race Conditions
    while let Some(x) = rx.recv().await {
      x.call();
    }
  });

  let state = Box::into_raw(Box::new(StateData {
    fut: local_state,
    tx,
  })) as *mut c_void;

  FutureTask {
    _state: state,
    _collect: use_state,
    _ready: use_ready,
    _waker: waker,
    _clean: clean_state,
  }
}
