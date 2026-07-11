use rayon::{
  ThreadPoolBuilder,
  iter::{IntoParallelRefIterator, ParallelIterator},
};
use std::{
  hint::spin_loop,
  mem::transmute,
  sync::{LazyLock, OnceLock, atomic::Ordering},
  thread::{self, Thread, available_parallelism},
  time::Duration,
};

use crate::space::Registrations;

pub(crate) mod space;

static SPACE: LazyLock<Registrations> = LazyLock::new(|| Registrations::new_init());
static SLICER: OnceLock<Thread> = OnceLock::new();

pub type Fn = extern "C" fn() -> bool;

#[unsafe(no_mangle)]
pub extern "C" fn register(id: u8, f: Fn) {
  init();

  SPACE.write(id, f);
}

#[unsafe(no_mangle)]
pub extern "C" fn unregister(id: u8, f: Fn) {
  SPACE.remove(id, f);
}

#[unsafe(no_mangle)]
pub extern "C" fn signal_init() {
  let Some(x) = SLICER.get() else { return };

  x.unpark();
}

#[unsafe(no_mangle)]
pub extern "C" fn init() {
  SLICER.get_or_init(|| {
    let t = thread::spawn(|| {
      let mut spins: u16 = 0;

      let pool = ThreadPoolBuilder::new()
        .num_threads(available_parallelism().unwrap().get().div_ceil(4))
        .build()
        .unwrap();

      loop {
        let guard = (&*SPACE).get();
        let executed_work = pool.install(|| {
          guard
            .par_iter()
            .map(|v| {
              let fptr = v.fnptr.load(Ordering::Acquire);

              // Sanity Pass
              if fptr.is_null() {
                return false;
              }

              unsafe { transmute::<_, Fn>(fptr)() }
            })
            .reduce(|| false, |a, b| a || b)
        });

        if executed_work {
          spins = 0;
        } else if spins % 10 == 0 {
          SPACE.try_gc();

          spins = spins.saturating_add(1);
        } else if spins < 1000 {
          spin_loop();

          spins = spins.saturating_add(1);
        } else if spins < 1010 {
          thread::yield_now();

          spins = spins.saturating_add(1);
        } else {
          SPACE.try_gc();

          thread::park_timeout(Duration::from_millis(1));
        }
      }
    });

    t.thread().clone()
  });
}
