use std::{ops::Deref, sync::Arc};

use crate::arcstore::im::{SwappableCodeStore, U8_JIT};

mod im;

#[test]
pub fn loom_atomic_waker() {
  loom::model(|| {
    let store = Arc::new(SwappableCodeStore::new(10));

    let s1 = store.clone();
    let jh1 = loom::thread::spawn(move || {
      let (_flags, val) = s1.get();

      assert!(*val == 10 || *val == 20);
    });

    let s2 = store.clone();
    let jh2 = loom::thread::spawn(move || {
      unsafe { s2.set(U8_JIT, 20, None) };
    });

    jh1.join().unwrap();
    jh2.join().unwrap();

    let (_, final_val) = store.get();
    assert!(*final_val == 20);
  });
}

#[test]
pub fn loom_atomic_waker_owned() {
  loom::model(|| {
    let store = Arc::new(SwappableCodeStore::<Box<str>>::new(Box::from(
      "Hello World",
    )));

    let s1 = store.clone();
    let jh1 = loom::thread::spawn(move || {
      let (flags, val) = s1.get();

      if val.as_ref() == "Hello Trello" {
        assert!(flags & U8_JIT != 0);
      }

      assert!(val.as_ref() == "Hello World" || val.as_ref() == "Hello Trello");
    });

    let s2 = store.clone();
    let jh2 = loom::thread::spawn(move || {
      unsafe { s2.set(U8_JIT, Box::from("Hello Trello") as _, None) };
    });

    jh1.join().unwrap();
    jh2.join().unwrap();

    let (flags, final_val) = store.get();
    assert!(final_val.as_ref() == "Hello Trello");
    assert!(flags & U8_JIT != 0);
  });
}

#[test]
pub fn loom_atomic_waker_storm() {
  let mut builder = loom::model::Builder::new();

  builder.preemption_bound = Some(2);
  builder.max_branches = Some(100_000);

  builder.check(|| {
    let store = Arc::new(SwappableCodeStore::new(10));

    let s1 = store.clone();
    let jh1 = loom::thread::spawn(move || {
      let (_flags, val) = s1.get();

      assert!(*val == 10 || *val == 20);
    });

    let s2 = store.clone();
    let jh2 = loom::thread::spawn(move || {
      let (_flags, val) = s2.get();

      assert!(*val == 10 || *val == 20);
    });

    let s3 = store.clone();
    let jh3 = loom::thread::spawn(move || {
      unsafe { s3.set(U8_JIT, 20, None) };
    });

    jh1.join().unwrap();
    jh2.join().unwrap();
    jh3.join().unwrap();

    let (_, final_val) = store.get();
    assert!(*final_val == 20);
  });
}
