use core::slice;
use parking_lot::Mutex;
use rayon::{
  iter::{Filter, IntoParallelIterator, IntoParallelRefIterator, ParallelIterator},
  slice::Iter,
};
use std::{
  alloc::{Layout, alloc, dealloc},
  iter,
  mem::MaybeUninit,
  ops::{ControlFlow, Deref, Mul},
  ptr::{addr_of, addr_of_mut},
  sync::atomic::{AtomicBool, AtomicPtr, AtomicU8, AtomicUsize, Ordering},
};

use crate::Fn;

pub struct Registrations {
  pub vector: AtomicPtr<VectorDescriptor>,

  pub writerlock: Mutex<()>,
  pub gc: Mutex<Vec<*mut VectorDescriptor>>,
}

unsafe impl Send for Registrations {}
unsafe impl Sync for Registrations {}

impl Registrations {
  pub fn new_init() -> Self {
    let ptr = unsafe {
      let layout = Self::vectalign(16);
      let pt = alloc(layout) as *mut VectorDescriptor;

      Self::vectwrite(pt, 16, layout, [].iter());

      pt
    };

    Self {
      vector: AtomicPtr::new(ptr),

      writerlock: Mutex::new(()),
      gc: Mutex::new(Vec::with_capacity(16)),
    }
  }

  fn vectwrite<'a, T>(pt: *mut VectorDescriptor, len: usize, layout: Layout, slice: T)
  where
    T: Iterator<Item = &'a Registration>,
  {
    let descriptor = unsafe { &mut *(pt as *mut MaybeUninit<VectorDescriptor>) };
    let descriptor = descriptor.write(VectorDescriptor {
      owners: AtomicUsize::new(1),
      len,
      layout,
      item1: MaybeUninit::uninit(),
    });

    let dest = unsafe { slice::from_raw_parts_mut(addr_of_mut!(descriptor.item1), len) };

    for (k, v) in dest.iter_mut().zip(
      slice
        .map(|x| Registration {
          active: AtomicBool::new(x.active.load(Ordering::Acquire)),
          fnptr: AtomicPtr::new(x.fnptr.load(Ordering::Acquire)),
          uid: AtomicU8::new(x.uid.load(Ordering::Acquire)),
        })
        .chain(iter::repeat_with(|| Registration {
          active: AtomicBool::new(false),
          fnptr: Default::default(),
          uid: Default::default(),
        })),
    ) {
      k.write(v);
    }
  }

  const fn vectalign(len: usize) -> Layout {
    let Ok(x) = Layout::from_size_align(
      size_of::<VectorDescriptor>() + len.saturating_sub(1) * size_of::<Registration>(),
      align_of::<VectorDescriptor>(),
    ) else {
      unreachable!();
    };

    x
  }

  pub fn write(&self, id: u8, f: Fn) {
    let writerlock = self.writerlock.lock();

    // Critical Section
    {
      let sliced = unsafe { &*self.descriptor() }.as_raw_slice();

      // Check if there any any free slots
      match sliced
        .iter()
        .find(|&x| x.active.load(Ordering::Acquire) == false)
      {
        Some(x) => {
          x.uid.store(id, Ordering::Relaxed);
          x.fnptr.store(f as _, Ordering::Relaxed);

          // SeqCst to ensure the above are fully flushed before active is true
          x.active.store(true, Ordering::Release);
        }
        None => {
          let ptr = unsafe {
            let layout = Self::vectalign(sliced.len().mul(2));
            let pt = alloc(layout) as *mut VectorDescriptor;

            let reg = Registration {
              active: AtomicBool::new(true),
              uid: AtomicU8::new(id),
              fnptr: AtomicPtr::new(f as _),
            };

            Self::vectwrite(
              pt,
              sliced.len().mul(2),
              layout,
              sliced.iter().chain(iter::once(&reg)),
            );

            pt
          };

          let old = self.vector.swap(ptr, Ordering::AcqRel);

          // Remove the host's ownership
          drop(VectorGuard(old));

          // Push to the GC Queue
          self.gc.lock().push(old);
        }
      }
    }

    drop(writerlock);
  }

  pub fn remove(&self, id: u8, f: Fn) {
    let writelock = self.writerlock.lock();

    {
      let remove = |desc: &VectorDescriptor| {
        _ = desc.as_raw_slice().iter().try_for_each(|x| {
          if x.uid.load(Ordering::Acquire) == id && x.fnptr.load(Ordering::Acquire) == f as _ {
            x.active.store(false, Ordering::Release);
            return ControlFlow::Break(());
          }

          ControlFlow::Continue(())
        });
      };

      remove(unsafe { &*self.vector.load(Ordering::Acquire) });
      self.gc_left(remove);
    }

    drop(writelock);
  }

  fn gc_left<'a, T>(&'a self, mut desc: T)
  where
    T: FnMut(&'a VectorDescriptor),
  {
    // Wait until lock is not freed
    let mut gc = self.gc.lock();

    for i in (0..gc.len()).rev() {
      let x = unsafe { *gc.get_unchecked(i) };
      let descriptor = unsafe { &*x };
      let orphan = descriptor.owners.load(Ordering::Acquire) == 0;

      if orphan {
        let layout = descriptor.layout;
        gc.swap_remove(i);
        unsafe { dealloc(x as _, layout) };
      } else {
        desc(descriptor);
      }
    }

    drop(gc);
  }

  pub fn try_gc(&self) {
    // If it is locked - someone else is doing GC or pushing to GC
    if let Some(mut gc) = self.gc.try_lock() {
      for i in (0..gc.len()).rev() {
        let x = unsafe { *gc.get_unchecked(i) };
        let descriptor = unsafe { &*x };
        let orphan = descriptor.owners.load(Ordering::Acquire) == 0;

        if orphan {
          let layout = descriptor.layout;
          gc.swap_remove(i);
          unsafe { dealloc(x as _, layout) };
        }
      }
    }
  }

  fn descriptor(&self) -> *mut VectorDescriptor {
    self.vector.load(Ordering::Acquire)
  }

  pub fn get(&self) -> VectorGuard {
    VectorGuard::new(self.descriptor())
  }
}

pub struct VectorGuard(*mut VectorDescriptor);

unsafe impl Send for VectorGuard {}
unsafe impl Sync for VectorGuard {}

impl VectorGuard {
  fn new(v: *mut VectorDescriptor) -> Self {
    unsafe {
      (*v).owners.fetch_add(1, Ordering::Relaxed);
    }
    Self(v)
  }
}

impl Deref for VectorGuard {
  type Target = VectorDescriptor;

  fn deref(&self) -> &Self::Target {
    unsafe { &*self.0 }
  }
}

impl Drop for VectorGuard {
  fn drop(&mut self) {
    unsafe {
      (*self.0).owners.fetch_sub(1, Ordering::Release);
    }
  }
}

#[repr(C)]
pub struct VectorDescriptor {
  pub owners: AtomicUsize,
  pub len: usize,
  pub layout: Layout,

  pub item1: MaybeUninit<Registration>,
}

impl VectorDescriptor {
  pub fn as_raw_slice<'a>(&'a self) -> &'a [Registration] {
    unsafe { slice::from_raw_parts(addr_of!(self.item1) as *mut Registration, self.len) }
  }
}

impl<'a> IntoIterator for &'a VectorDescriptor {
  type Item = &'a Registration;
  type IntoIter = std::iter::Filter<std::slice::Iter<'a, Registration>, fn(&Self::Item) -> bool>;

  fn into_iter(self) -> Self::IntoIter {
    self.as_raw_slice().iter().filter(process)
  }
}

impl<'a> IntoParallelIterator for &'a VectorDescriptor {
  type Item = &'a Registration;
  type Iter = Filter<Iter<'a, Registration>, fn(&Self::Item) -> bool>;

  fn into_par_iter(self) -> Self::Iter {
    self.as_raw_slice().par_iter().filter(process)
  }
}

fn process(&x: &&Registration) -> bool {
  x.active.load(Ordering::Acquire) == true
}

#[repr(C)]
pub struct Registration {
  pub active: AtomicBool,
  pub uid: AtomicU8,
  pub fnptr: AtomicPtr<()>,
}
