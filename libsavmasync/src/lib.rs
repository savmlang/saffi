use dashmap::DashSet;
use parking_lot::RwLock;
use rapidhash::fast::RandomState;
use rayon::{
  ThreadPoolBuilder,
  iter::{IntoParallelRefIterator, ParallelIterator},
};
use std::{
  hint::spin_loop,
  sync::{LazyLock, OnceLock},
  thread::{self, available_parallelism, sleep},
  time::Duration,
};

static SPACE: LazyLock<DashSet<(u8, Fn), RandomState>> =
  LazyLock::new(|| DashSet::with_capacity_and_hasher(32, RandomState::new()));
static LOCK: RwLock<()> = RwLock::new(());
static SLICER: OnceLock<()> = OnceLock::new();

pub type Fn = extern "C" fn() -> bool;

#[unsafe(no_mangle)]
pub extern "C" fn register(id: u8, f: Fn) {
  init();
  let lck = LOCK.read();
  SPACE.insert((id, f));
  drop(lck);
}

#[unsafe(no_mangle)]
pub extern "C" fn unregister(id: u8, f: Fn) {
  let lck = LOCK.write();
  _ = SPACE.remove(&(id, f));
  drop(lck);
}

#[unsafe(no_mangle)]
pub extern "C" fn init() {
  SLICER.get_or_init(|| {
    thread::spawn(|| {
      let mut spins: u16 = 0;

      let pool = ThreadPoolBuilder::new()
        .num_threads(available_parallelism().unwrap().get().div_ceil(4))
        .build()
        .unwrap();

      loop {
        let mut executed_work = false;

        if let Some(lock) = LOCK.try_read() {
          executed_work = pool.install(|| {
            SPACE
              .par_iter()
              .map(|v| (v.key().1)())
              .reduce(|| false, |a, b| a | b)
          });

          drop(lock);
        }

        if executed_work {
          spins = 0;
        } else if spins < 1000 {
          spin_loop();
          spins += 1;
        } else if spins < 1010 {
          thread::yield_now();
          spins += 1;
        } else {
          sleep(Duration::from_millis(1));
        }
      }
    });

    ()
  });
}
