use std::{
  ffi::c_void,
  mem::{forget, offset_of},
  ops::{Deref, DerefMut},
  ptr::{self, NonNull},
};

use crate::FFISafe;

pub mod spawn;

#[repr(C)]
pub struct RTBoxWrapper<T: FFISafe + Sized> {
  _free: unsafe extern "C" fn(data: *mut c_void),
  _t: T,
}

/// Returns the offset of T within RTBoxWrapper<T>.
/// This is guaranteed to be correct regardless of padding.
pub const fn rt_t_offset<T: FFISafe>() -> usize {
  offset_of!(RTBoxWrapper<T>, _t)
}

/// Negative offset from T back to the Header:
pub const fn rt_header_offset<T: FFISafe>() -> isize {
  -(rt_t_offset::<T>() as isize)
}

pub struct RTBox<T: FFISafe + Sized> {
  ptr: NonNull<T>,
}

impl<T: FFISafe + Sized> RTBox<T> {
  pub fn new(data: T) -> Option<Self> {
    unsafe {
      // Use our own allocator
      let out: *mut RTBoxWrapper<T> =
        salloc::aligned_malloc(size_of::<RTBoxWrapper<T>>(), align_of::<RTBoxWrapper<T>>()) as _;

      if out.is_null() {
        return None;
      }

      ptr::write(
        out,
        RTBoxWrapper {
          _free: mfree::<T>,
          _t: data,
        },
      );

      Some(Self {
        ptr: NonNull::new_unchecked(out.byte_add(rt_t_offset::<T>()) as _),
      })
    }
  }

  pub fn into_raw(self) -> *mut T {
    let ptr = self.ptr;
    forget(self);

    ptr.as_ptr()
  }

  pub unsafe fn as_ptr(&self) -> *const T {
    self.ptr.as_ptr() as _
  }

  pub unsafe fn as_mut_ptr(&self) -> *mut T {
    self.ptr.as_ptr()
  }

  #[inline(always)]
  pub unsafe fn from_raw(data: *mut T) -> Option<Self> {
    Some(Self {
      ptr: NonNull::new(data)?,
    })
  }

  /// This is only allowed by the version
  /// the allocated the RTBox with RTBox::new
  pub unsafe fn unbox(self) -> T {
    let ptr = self.ptr;

    let out = unsafe { ptr::read(ptr.as_ptr()) };

    // Deallocate shim
    unsafe {
      let base_ptr = (self.ptr.as_ptr() as *mut u8).byte_offset(rt_header_offset::<T>());
      salloc::aligned_free(base_ptr as _);
    }

    forget(self);

    out
  }
}

impl<T: FFISafe + Sized> Deref for RTBox<T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    unsafe { &*self.as_ptr() }
  }
}

impl<T: FFISafe + Sized> DerefMut for RTBox<T> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    unsafe { &mut *self.as_mut_ptr() }
  }
}

unsafe extern "C" fn mfree<T: FFISafe>(data: *mut c_void) {
  unsafe {
    ptr::drop_in_place(data as *mut T);

    let base_ptr = (data as *mut u8).byte_offset(rt_header_offset::<T>());
    salloc::aligned_free(base_ptr as _);
  }
}

impl<T: FFISafe + Sized> Drop for RTBox<T> {
  fn drop(&mut self) {
    unsafe {
      let header_ptr = self.ptr.as_ptr().byte_offset(rt_header_offset::<T>())
        as *mut unsafe extern "C" fn(*mut c_void);

      (*header_ptr)(self.ptr.as_ptr() as _);
    }
  }
}

pub unsafe fn drop_rtbox<T: FFISafe + Sized>(wrap: *mut T) {
  debug_assert_eq!(wrap.is_null(), false);

  unsafe {
    drop(RTBox::from_raw(wrap));
  }
}

pub unsafe fn peek<T: FFISafe + Copy>(wrap: *const T) -> T {
  debug_assert_eq!(wrap.is_null(), false);

  unsafe { *wrap }
}
