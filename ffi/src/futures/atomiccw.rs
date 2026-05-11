use std::{
  cell::UnsafeCell,
  hint::spin_loop,
  mem::zeroed,
  sync::atomic::{AtomicU64, Ordering},
  thread::yield_now,
};

use crate::futures::{CWaker, WAKER_VTABLE, WakerVTable};

pub(crate) struct AtomicFFICWaker {
  state: AtomicU64, // [63: LOCKED] [62: NOTIFIED] [61: NEW] [0-60: REF COUNT]
  data: UnsafeCell<CWaker>,
  vtable: &'static WakerVTable, // Immutable & Stack-friendly
}

const LOCKED: u64 = 1 << 63;
const NOTIFIED: u64 = 1 << 62;
const NEW: u64 = 1 << 61;

impl AtomicFFICWaker {
  pub fn new() -> Self {
    unsafe {
      Self {
        data: UnsafeCell::new(zeroed()),
        state: AtomicU64::new(0x2000000000000000 | 1),
        vtable: &WAKER_VTABLE,
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
    let mask: u64 = (NEW) | (NOTIFIED) | (LOCKED);

    if (prev & !mask) == 1 {
      unsafe {
        let n_mask = NEW;

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
    unsafe { (self.vtable.free_waker)(*self.data.get()) };
  }

  pub fn set_vtable(&mut self, vtable: *const WakerVTable) {
    self.vtable = unsafe { &*vtable };
  }

  #[inline(always)]
  pub fn wake(&self) {
    let mut current_state = self.state.load(Ordering::Relaxed);

    let mut iterations: u32 = 0;
    loop {
      iterations += 1;

      let mut newstate = current_state | NOTIFIED;

      // no `LOCK`
      if current_state & LOCKED == 0 {
        newstate |= LOCKED;
      }

      match self.state.compare_exchange_weak(
        current_state,
        newstate,
        Ordering::Acquire,
        Ordering::Relaxed,
      ) {
        Ok(_) => {
          // Had no lock Initially
          if current_state & LOCKED == 0 {
            // Now lock has been acquired
            // Let's wake the waker
            break;
          }

          return;
        }
        Err(new) => {
          current_state = new;

          if iterations % 20 == 0 {
            iterations = 0;
            yield_now();
          } else {
            spin_loop();
          }
        }
      }
    }

    // --- WE NOW HOLD THE LOCK (Bit 63 is 1) ---
    // The `Acquire` from Path B guarantees `self.data` is completely visible and stable.

    unsafe {
      if current_state & (NEW) == 0 {
        let d = *self.data.get();
        (self.vtable.wake_no_free)(d);
      }
    }

    self
      .state
      .fetch_and(!(LOCKED | NOTIFIED), Ordering::Release);
  }

  pub fn update(&self, new_data: CWaker) {
    let mut current_state = self.state.load(Ordering::Relaxed);

    let mut iterations: u32 = 0;
    loop {
      iterations += 1;

      // If already locked
      // We wait
      if current_state & LOCKED != 0 {
        if iterations % 20 == 0 {
          iterations = 0;
          yield_now();
        } else {
          spin_loop();
        }

        current_state = self.state.load(Ordering::Relaxed);
        continue;
      }

      // We try to assert LOCK
      match self.state.compare_exchange_weak(
        current_state,
        current_state | LOCKED,
        Ordering::Acquire,
        Ordering::Relaxed,
      ) {
        // Lock Acquired, let's go
        Ok(_) => break,
        Err(new) => current_state = new,
      }
    }

    // CRITICAL SECTION - Set Waker
    let old_data = unsafe {
      // 2. Swap the data.
      // Since we hold bit 63, no 'wake()' will touch this right now.
      std::ptr::replace(self.data.get(), new_data)
    };
    // END CRITICAL

    // 3. Release Lock AND Clear Notified Bit, Locked Bit simultaneously
    let prev = self
      .state
      // Remove LOCK, NOTIFIED, NEW
      .fetch_and(!(LOCKED | NOTIFIED | NEW), Ordering::Release);

    // 4. If someone tried to wake us while we were swapping...
    if (prev & NOTIFIED) != 0 {
      // ...we do the work they skipped.
      unsafe { (self.vtable.wake_no_free)(new_data) };
    }

    // 5. Cleanup the old data
    // We ensure NEW bit is not set
    if prev & NEW == 0 {
      unsafe { (self.vtable.free_waker)(old_data) };
    }
  }
}
