use std::{
  cell::UnsafeCell,
  mem::zeroed,
  sync::atomic::{AtomicUsize, Ordering},
};

use crate::futures::{CWaker, WakerVTable};

pub(crate) struct AtomicFFICWaker {
  state: AtomicUsize, // [63: LOCKED] [62: NOTIFIED] [61: NEW] [0-60: REF COUNT]
  data: UnsafeCell<CWaker>,
  vtable: &'static WakerVTable, // Immutable & Stack-friendly
}

impl AtomicFFICWaker {
  pub fn new(vtable: *const WakerVTable) -> Self {
    unsafe {
      Self {
        data: UnsafeCell::new(zeroed()),
        state: AtomicUsize::new(0x2000000000000000 | 1),
        vtable: &*vtable,
      }
    }
  }

  pub fn inc(&self) {
    self.state.fetch_add(1, Ordering::AcqRel);
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
          self.drop();
        }
      }

      return true;
    }

    false
  }

  unsafe fn drop(&self) {
    unsafe { (self.vtable.free_waker)((&*self.data.get()).unsafe_bitcopy()) };
  }

  pub fn set_vtable(&mut self, vtable: *const WakerVTable) {
    self.vtable = unsafe { &*vtable };
  }

  #[inline(always)]
  pub fn wake(&self) {
    // Step 1: Mark as notified.
    // Single instruction. No branches yet.
    let old = self.state.fetch_or(1 << 62, Ordering::AcqRel);

    // Step 2: Check the Lock bit.
    // If 0, WE are the ones who must call the FFI.
    if (old & ((1 << 63) | (1 << 61))) == 0 {
      // Because it's not LOCKED, data is stable.
      unsafe {
        let d = &*self.data.get();

        (self.vtable.wake_no_free)(d);
      }
    }
    // If old & (1 << 63) != 0, we just walk away.
    // The thread holding the lock WILL see the bit we just set.
  }

  pub fn update(&self, new_data: CWaker) {
    // 1. Grab the Lock (Bit 63)
    while self.state.fetch_or(1 << 63, Ordering::Acquire) & (1 << 63) != 0 {
      std::hint::spin_loop();
    }

    let old_data = unsafe {
      // 2. Swap the data.
      // Since we hold bit 63, no 'wake()' will touch this right now.
      std::ptr::replace(self.data.get(), new_data.unsafe_bitcopy())
    };

    // 3. Release Lock AND Clear Notified Bit, Locked Bit simultaneously
    let prev = self
      .state
      .fetch_and(!((1 << 63) | (1 << 62) | (1 << 61)), Ordering::Release);

    // 4. If someone tried to wake us while we were swapping...
    if (prev & (1 << 62)) != 0 {
      // ...we do the work they skipped.
      (self.vtable.wake_no_free)(&new_data);
    }

    // 5. Cleanup the old data
    // We ensure NEW bit is not set
    if prev & (1 << 61) == 0 {
      (self.vtable.free_waker)(old_data);
    }
  }
}
