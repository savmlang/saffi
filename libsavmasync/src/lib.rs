use parking_lot::RwLock;
use rapidhash::fast::RandomState;
use rayon::{
  ThreadPoolBuilder,
  iter::{IntoParallelRefIterator, ParallelIterator},
};
use std::{
  collections::HashSet,
  hash::Hash,
  hint::spin_loop,
  ptr::fn_addr_eq,
  sync::{
    LazyLock, OnceLock,
    atomic::{AtomicBool, Ordering},
  },
  thread::{self, available_parallelism, sleep},
  time::Duration,
};

static SPACE: LazyLock<RwLock<HashSet<Key, RandomState>>> =
  LazyLock::new(|| RwLock::new(HashSet::with_capacity_and_hasher(32, RandomState::new())));
static SLICER: OnceLock<()> = OnceLock::new();

pub type Fn = extern "C" fn() -> bool;

pub struct Key(AtomicBool, u8, Fn);

impl Hash for Key {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.1, self.2).hash(state);
  }
}
impl PartialEq for Key {
  fn eq(&self, other: &Self) -> bool {
    self.1 == other.1 && fn_addr_eq(self.2, other.2)
  }
}
impl Eq for Key {}

#[unsafe(no_mangle)]
pub extern "C" fn register(id: u8, f: Fn) {
  init();

  SPACE.write().insert(Key(AtomicBool::new(true), id, f));
}

#[unsafe(no_mangle)]
pub extern "C" fn unregister(id: u8, f: Fn) {
  if let Some(x) = SPACE
    .read()
    .iter()
    .find(|&&Key(_, x, f_)| x == id && fn_addr_eq(f_, f))
  {
    x.0.store(false, Ordering::Release);
  }
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

        let dirty = AtomicBool::new(false);
        if let Some(space) = SPACE.try_read() {
          println!("Entering pool");
          executed_work = pool.install(|| {
            space
              .par_iter()
              .map(|v| {
                if v.0.load(Ordering::Acquire) {
                  (v.2)()
                } else {
                  dirty.store(true, Ordering::Release);
                  false
                }
              })
              .reduce(|| false, |a, b| a || b)
          });
          println!("Exiting pool");
        }

        if dirty.load(Ordering::Acquire) {
          println!("Cleaning space");
          SPACE.write().retain(|x| x.0.load(Ordering::Acquire));
        }

        if executed_work {
          spins = 0;
        } else if spins < 1000 {
          spin_loop();
          spins = spins.saturating_add(1);
        } else if spins < 1010 {
          thread::yield_now();
          spins = spins.saturating_add(1);
        } else {
          sleep(Duration::from_millis(1));
        }
      }
    });

    ()
  });
}
