use std::{mem::zeroed, sync::atomic::Ordering};

use loom::{
  cell::UnsafeCell,
  hint::spin_loop,
  sync::atomic::{AtomicBool, AtomicUsize},
  thread::yield_now,
};

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CWaker {
  pub _waker_data: [u8; 16],
}

#[repr(C)]
pub struct CRawWakerVTable {
  pub wake_no_free: extern "C" fn(data: CWaker) -> (),
  pub free_waker: extern "C" fn(data: CWaker) -> (),
}

pub struct AtomicFFICWaker {
  state: AtomicUsize, // [63: LOCKED] [62: NOTIFIED] [61: NEW] [0-60: REF COUNT]
  data: UnsafeCell<CWaker>,
  vtable: &'static CRawWakerVTable, // Immutable & Stack-friendly
  freed: AtomicBool,
}

impl AtomicFFICWaker {
  pub fn new(vtable: *const CRawWakerVTable) -> Self {
    unsafe {
      Self {
        data: UnsafeCell::new(zeroed()),
        state: AtomicUsize::new(0x2000000000000000 | 1),
        // We allow invalid value to correctly test loom
        vtable: &*vtable,
        freed: AtomicBool::new(false),
      }
    }
  }

  pub fn inc(&self) {
    self.state.fetch_add(1, Ordering::Relaxed);
  }

  // This decrements the Waker and also, if
  // there are no more references, it automatically
  // frees the waker
  pub fn dec(&self) -> bool {
    let prev = self.state.fetch_sub(1, Ordering::AcqRel);

    // 0 0 1 1
    // i.e. hex `3`
    let mask: usize = (1 << 61) | (1 << 62) | (1 << 63);

    if (prev & !mask) == 1 {
      unsafe {
        let n_mask = 1 << 61;

        // Verify that NEW bit is not set
        if prev & n_mask == 0 {
          self.data.get_mut().deref()._waker_data[0] = 30;
          self.drop();
        }
      }

      return true;
    }

    false
  }

  unsafe fn drop(&self) {
    self
      .freed
      .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
      .expect("This must be false");
    unsafe { (self.vtable.free_waker)(*self.data.get().deref()) };
  }

  #[inline(always)]
  pub fn wake(&self) {
    let mut current_state = self.state.load(Ordering::Relaxed);

    // This is locked
    let mut iteration: u32 = 0;
    loop {
      iteration += 1;

      let mut newstate = current_state | (1 << 62);

      // No `LOCK`
      if current_state & (1 << 63) == 0 {
        newstate |= 1 << 63;
      }

      match self.state.compare_exchange_weak(
        current_state,
        newstate,
        Ordering::Acquire,
        Ordering::Relaxed,
      ) {
        // data set
        Ok(_) => {
          // Initially had no lock
          if current_state & (1 << 63) == 0 {
            break;
          }

          return;
        }
        Err(new) => {
          current_state = new;

          if iteration % 20 == 0 {
            iteration = 0;
            yield_now();
          } else {
            spin_loop();
          }
        }
      }
    }

    // We hold the LOCK, do our stuff, exit
    unsafe {
      if current_state & (1 << 61) == 0 {
        let d = *self.data.get().deref();

        assert_eq!(d._waker_data[0], 1);

        (self.vtable.wake_no_free)(d);
      }
    }

    self
      .state
      .fetch_and(!(1 << 63 | 1 << 62), Ordering::Release);
  }

  pub fn update(&self, new_data: CWaker) {
    let mut current_state = self.state.load(Ordering::Relaxed);

    let mut iteration: u32 = 0;
    loop {
      iteration += 1;
      // If already locked, we must wait (with a yield for Loom)
      if current_state & (1 << 63) != 0 {
        if iteration % 20 == 0 {
          iteration = 0;
          yield_now();
        } else {
          spin_loop();
        }
        current_state = self.state.load(Ordering::Relaxed);
        continue;
      }

      match self.state.compare_exchange(
        current_state,
        current_state | (1 << 63),
        Ordering::Acquire, // Acquire to see 'data'
        Ordering::Relaxed,
      ) {
        Ok(_) => break, // Lock acquired!
        Err(new) => current_state = new,
      }
    }

    // --- CRITICAL SECTION START ---
    let old_data = unsafe { std::ptr::replace(self.data.get_mut().deref(), new_data) };
    // --- CRITICAL SECTION END ---

    // 2. Release Lock AND Clear Notified/New bits simultaneously
    // We use Release to push the new_data to any future Acquire-readers
    let prev = self
      .state
      .fetch_and(!((1 << 63) | (1 << 62) | (1 << 61)), Ordering::Release);

    // 3. Delegate Wake if someone signaled while we were busy
    if (prev & (1 << 62)) != 0 {
      (self.vtable.wake_no_free)(new_data);
    }

    // 4. Cleanup old data (Birth Certificate Check)
    if prev & (1 << 61) == 0 {
      (self.vtable.free_waker)(old_data);
    }
  }
}
