use std::ptr::{addr_of, addr_of_mut};

use loom::{sync::Arc, thread};

use crate::waker::atomicwaker::{AtomicFFICWaker, CRawWakerVTable, CWaker};

mod atomicwaker;

extern "C" fn wake_no_free(_data: CWaker) {}

extern "C" fn free_waker(_data: CWaker) {}

static WAKER_VTABLE: CRawWakerVTable = CRawWakerVTable {
  free_waker,
  wake_no_free,
};

#[test]
pub fn loom_atomic_waker() {
  loom::model(|| {
    let waker = AtomicFFICWaker::new(addr_of!(WAKER_VTABLE));
    let waker = Arc::new(waker);

    let updater = waker.clone();
    updater.inc();

    // let updater2 = waker.clone();
    // updater2.inc();

    thread::spawn(move || {
      waker.update(CWaker {
        _waker_data: [1u8; 16],
      });
      waker.dec();
    });

    thread::spawn(move || {
      updater.wake();
      updater.dec();
    });

    // thread::spawn(move || {
    //   updater2.wake();
    //   updater2.dec();
    // });
  });
}

#[test]
pub fn loom_multi_wakes() {
  loom::model(|| {
    let waker = AtomicFFICWaker::new(addr_of!(WAKER_VTABLE));
    let waker = Arc::new(waker);

    waker.update(CWaker {
      _waker_data: [1u8; 16],
    });

    let updater = waker.clone();
    updater.inc();

    // let updater2 = waker.clone();
    // updater2.inc();

    thread::spawn(move || {
      waker.wake();
      waker.wake();
      waker.dec();
    });

    thread::spawn(move || {
      updater.wake();
      updater.dec();
    });

    // thread::spawn(move || {
    //   updater2.wake();
    //   updater2.dec();
    // });
  });
}

#[test]
pub fn loom_concurrent_updates() {
  loom::model(|| {
    let waker = Arc::new(AtomicFFICWaker::new(addr_of!(WAKER_VTABLE)));

    let w3 = waker.clone();
    w3.inc();

    thread::spawn(move || {
      waker.update(CWaker {
        _waker_data: [2u8; 16],
      });
      waker.dec();
    });

    thread::spawn(move || {
      w3.update(CWaker {
        _waker_data: [3u8; 16],
      });
      w3.dec();
    });
  });
}
