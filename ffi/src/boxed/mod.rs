use std::{
  ffi::c_void,
  mem::{forget, offset_of},
  ops::{Deref, DerefMut},
  ptr::{self, NonNull, addr_of_mut},
};

use crate::{
  FFISafe,
  I_DECLARE_THAT_I_AND_MY_CODEBASE_IS_FFI_SAFE_AND_THAT_UNDEFINED_BEHAVIOUR_ARISING_DUE_TO_DECLARING_MY_TYPES_FFI_SAFE_DOES_NOT_CONDONE_THE_SAFETY_AND_SECURITY_OF_THIS_PROJECT,
};

#[repr(C)]
pub struct RTBoxWrapper<T: FFISafe> {
  _free: unsafe extern "C" fn(data: *mut c_void),
  _t: T,
}

pub struct RTBox<T: FFISafe> {
  ptr: NonNull<T>,
}

unsafe impl<T: FFISafe> FFISafe for RTBox<T> {
  fn i_am_ffisafe() -> crate::IAmFFISafe {
    I_DECLARE_THAT_I_AND_MY_CODEBASE_IS_FFI_SAFE_AND_THAT_UNDEFINED_BEHAVIOUR_ARISING_DUE_TO_DECLARING_MY_TYPES_FFI_SAFE_DOES_NOT_CONDONE_THE_SAFETY_AND_SECURITY_OF_THIS_PROJECT
  }
}
unsafe impl<T: FFISafe + Send> Send for RTBox<T> {}
unsafe impl<T: FFISafe + Sync> Sync for RTBox<T> {}

impl<T: FFISafe> RTBox<T> {
  pub fn new(data: T) -> Option<Self> {
    // SAFETY:
    //
    // This implementation is defined safe.
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
        ptr: NonNull::new(addr_of_mut!((*out)._t))?,
      })
    }
  }

  pub fn into_raw(self) -> *mut T {
    let ptr = self.ptr;
    forget(self);

    ptr.as_ptr()
  }

  pub fn as_ptr(&self) -> *const T {
    self.ptr.as_ptr() as _
  }

  pub fn as_mut_ptr(&self) -> *mut T {
    self.ptr.as_ptr()
  }

  #[inline(always)]
  /// Creates an RTBox from the associated pointer
  ///
  /// # Safety
  /// This pointer must have previously been provided by an RTBox wrapper
  /// having the same SaFFI version. Failure to account for that will lead to Undefined Behaviour.
  ///
  /// Pointers across boundaries can be legally used here.
  pub unsafe fn from_raw(data: *mut T) -> Option<Self> {
    Some(Self {
      ptr: NonNull::new(data)?,
    })
  }

  /// Safety:
  ///
  /// Safety constraints cannot be displayed for deprecated, potentially dangerous
  /// functions.
  #[deprecated(
    since = "0.1.0",
    note = "Use of this method is highly discouraged due to potential (disastrous) consequences"
  )]
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
    // SAFETY: Using this is technically safe because RTBox
    // is immutably borrowed, so safe code cannot
    // trigger a UAF or
    unsafe { &*self.as_ptr() }
  }
}

impl<T: FFISafe + Sized> DerefMut for RTBox<T> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    // SAFETY: Using this is technically safe because RTBox
    // is mutably borrowed, so safe code cannot
    // trigger errors
    unsafe { &mut *self.as_mut_ptr() }
  }
}

/// Negative offset from T back to the Header:
const fn rt_header_offset<T: FFISafe>() -> isize {
  -(offset_of!(RTBoxWrapper<T>, _t) as isize)
}

// Internal machinery to maintain owner.
unsafe extern "C" fn mfree<T: FFISafe>(data: *mut c_void) {
  unsafe {
    ptr::drop_in_place(data as *mut T);

    let base_ptr = (data as *mut u8).byte_offset(rt_header_offset::<T>());
    salloc::aligned_free(base_ptr as _);
  }
}

impl<T: FFISafe + Sized> Drop for RTBox<T> {
  fn drop(&mut self) {
    // SAFETY:
    // Since it is owned by the context
    // It is safe to access pointer fields.
    unsafe {
      let header_ptr =
        self.ptr.as_ptr().byte_offset(rt_header_offset::<T>()) as *mut RTBoxWrapper<T>;

      ((*header_ptr)._free)(self.ptr.as_ptr() as _);
    }
  }
}

/// Drops the associated RTBox
///
/// # Safety
///
/// Failure to account for multiple owners WILL lead to a
/// Use-After-Free or, even worse - Silent Corruption.
///
/// The pointer must be exclusively owned by the holder
/// of this pointer.
pub unsafe fn drop_rtbox<T: FFISafe + Sized>(wrap: *mut T) {
  debug_assert_eq!(wrap.is_null(), false);

  unsafe {
    drop(RTBox::from_raw(wrap));
  }
}

/// Peeks at the value pointed to by the RTBox
///
/// # Safety
///
/// Failure to account for aliasing violations will lead to undefined behaviour
/// Also, the pointer must be valid, aligned.
pub unsafe fn peek<T: FFISafe + Copy>(wrap: *const T) -> T {
  debug_assert_eq!(wrap.is_null(), false);

  unsafe { *wrap }
}
