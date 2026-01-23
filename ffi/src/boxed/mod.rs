use core::{
  ffi::c_void,
  marker::PhantomData,
  ops::{Deref, DerefMut},
  ptr,
};

pub mod spawn;

use crate::FFISafe;

#[repr(C)]
pub struct RTSafeBoxWrapper {
  _data: *mut c_void,
  _free: unsafe extern "C" fn(data: *mut c_void),
}

unsafe extern "C" fn mfree<T>(data: *mut c_void) {
  unsafe {
    ptr::drop_in_place(data as *mut T);
    salloc::aligned_free(data);
  }
}

impl RTSafeBoxWrapper {
  pub fn new<T: FFISafe>(data: T) -> RTBox<T> {
    RTBox {
      _wrap: unsafe { Self::new_raw(data) },
      _data: PhantomData,
      poisoned: false,
    }
  }

  pub unsafe fn new_raw<T: FFISafe>(data: T) -> *mut RTSafeBoxWrapper {
    unsafe {
      let alignment = align_of::<T>();

      let _data = salloc::aligned_malloc(size_of::<T>(), alignment);

      #[cfg(debug_assertions)]
      if _data.is_null() {
        panic!("Unable to construct");
      }

      ptr::write(_data as _, data);

      let structdata = Self {
        _data: _data as _,
        _free: mfree::<T>,
      };

      let structdata_ptr =
        salloc::aligned_malloc(size_of::<Self>(), align_of::<Self>()) as *mut Self;

      #[cfg(debug_assertions)]
      if structdata_ptr.is_null() {
        salloc::aligned_free(_data);
        panic!("Unable to construct");
      }

      ptr::write(structdata_ptr, structdata);

      structdata_ptr
    }
  }

  /// You, the developer is required to ensure that `T` is correct
  /// This constructs a Wrapper Type that's not FFI-able
  pub unsafe fn construct<T: FFISafe>(pointer: *mut RTSafeBoxWrapper) -> RTBox<T> {
    debug_assert_eq!(pointer.is_null(), false);

    RTBox {
      _wrap: pointer,
      _data: PhantomData,
      poisoned: false,
    }
  }
}

pub unsafe fn drop_rtbox(wrap: *mut RTSafeBoxWrapper) {
  debug_assert_eq!(wrap.is_null(), false);
  let boxed = unsafe { &*wrap };

  unsafe {
    (boxed._free)(boxed._data);
    salloc::aligned_free(wrap as *mut c_void);
  }
}

pub unsafe fn peek<T: Copy>(wrap: *const RTSafeBoxWrapper) -> T {
  debug_assert_eq!(wrap.is_null(), false);
  let boxed = unsafe { &*wrap };

  *unsafe { &*(boxed._data as *const _ as *const T) }
}

pub unsafe fn reference<'a, T>(wrap: *const RTSafeBoxWrapper) -> &'a T {
  debug_assert_eq!(wrap.is_null(), false);

  unsafe { &*(&*wrap)._data.cast::<T>() }
}

pub unsafe fn reference_mut<'a, T>(wrap: *mut RTSafeBoxWrapper) -> &'a mut T {
  debug_assert_eq!(wrap.is_null(), false);

  unsafe { &mut *(&*wrap)._data.cast::<T>() }
}

pub struct ContainedRTBox {
  pub _wrap: *mut RTSafeBoxWrapper,
  drop: bool,
}

impl ContainedRTBox {
  pub fn new(data: *mut RTSafeBoxWrapper) -> Self {
    Self {
      _wrap: data,
      drop: true,
    }
  }

  pub fn get_const(&self) -> *const c_void {
    debug_assert_eq!(self._wrap.is_null(), false);

    unsafe { (&*self._wrap)._data as _ }
  }

  pub fn into_raw(mut self) -> *mut RTSafeBoxWrapper {
    self.drop = false;
    self._wrap
  }
}

impl Drop for ContainedRTBox {
  fn drop(&mut self) {
    if self.drop {
      debug_assert_eq!(self._wrap.is_null(), false);
      let boxed = unsafe { &*self._wrap };

      unsafe {
        (boxed._free)(boxed._data);
        salloc::aligned_free(self._wrap as *mut c_void);
      }
    }
  }
}

pub struct RTBox<T: FFISafe> {
  _wrap: *mut RTSafeBoxWrapper,
  _data: PhantomData<T>,
  poisoned: bool,
}

unsafe impl<T: FFISafe> Send for RTBox<T> {}
unsafe impl<T: FFISafe> Sync for RTBox<T> {}

impl<T: FFISafe> RTBox<T> {
  pub fn into_raw(mut self) -> *mut RTSafeBoxWrapper {
    self.poisoned = true;

    self._wrap
  }
}

impl<T: FFISafe + Clone> RTBox<T> {
  pub fn unwrap(self) -> T {
    (&*self).clone()
  }
}

impl<T: FFISafe> Deref for RTBox<T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    unsafe { &*(*self._wrap)._data.cast::<T>() }
  }
}

impl<T: FFISafe> DerefMut for RTBox<T> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    unsafe { &mut *(*self._wrap)._data.cast::<T>() }
  }
}

impl<T: FFISafe> Drop for RTBox<T> {
  fn drop(&mut self) {
    if !self.poisoned {
      let boxed = unsafe { &*self._wrap };

      unsafe {
        (boxed._free)(boxed._data);
        salloc::aligned_free(self._wrap as *mut c_void);
      }
    }
  }
}
