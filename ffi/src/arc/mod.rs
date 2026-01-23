use core::{ffi::c_void, marker::PhantomData, ops::Deref};

use std::sync::Arc;

use crate::FFISafe;

#[repr(C)]
pub enum Maybe {
  Yes(*mut c_void),
  No,
}

#[repr(C)]
/// Please note that the Arc may not be like you think
/// You need to make sure that you've not overused the Arc
///
/// You must also be aware that this Arc might be destroyed at any moment!
///
/// This data type uses the rust allocator as mutual ownership is not possible to implement for
/// this structure
///
/// ## DANGER
///
/// The Arc struct is automatically deallocated as soon as no dynamic link library
/// has an [Arced] struct regardless if you have the pointer of not.
pub struct Arced<T: FFISafe> {
  _inner: *const c_void,
  _use: extern "C" fn(ptr: *const c_void),
  _unuse: extern "C" fn(ptr: *const c_void),
  _marker: PhantomData<T>,
}

extern "C" fn _use_arc<T>(ptr: *const c_void) {
  unsafe { Arc::increment_strong_count(ptr) };
}

extern "C" fn _uunuse_arc<T>(ptr: *const c_void) {
  unsafe { Arc::decrement_strong_count(ptr) };
}

impl<T: FFISafe> Arced<T> {
  /// Returns the arc along with a free function that can be called to free it directly
  pub fn new(data: T) -> Self {
    let data = Arc::into_raw(Arc::new(data));

    Self {
      _inner: data as *const c_void,
      _unuse: _uunuse_arc::<T>,
      _use: _use_arc::<T>,
      _marker: PhantomData,
    }
  }

  pub fn from_raw(arc: *const Self) -> Self {
    let rf = unsafe { &*arc };

    (rf._use)(rf._inner);

    Self {
      _use: rf._use,
      _inner: rf._inner,
      _unuse: rf._unuse,
      _marker: PhantomData::<T>,
    }
  }

  pub fn as_raw(&self) -> *const Self {
    self as _
  }
}

impl<T: FFISafe> Deref for Arced<T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    unsafe { &*(self._inner as *const T) }
  }
}

impl<T: FFISafe> Drop for Arced<T> {
  fn drop(&mut self) {
    (self._unuse)(self._inner);
  }
}
