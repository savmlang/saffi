#![no_std]

use core::ffi::c_void;

#[cfg_attr(windows, link(name = "salloc", kind = "raw-dylib"))]
#[cfg_attr(not(windows), link(name = "salloc", kind = "dylib"))]
unsafe extern "C" {
  /// Allocates a pointer using the size, align.
  pub unsafe fn aligned_malloc(size: usize, align: usize) -> *mut c_void;

  /// Zero allocates a pointer using the size, align.
  pub unsafe fn aligned_zalloc(size: usize, align: usize) -> *mut c_void;

  /// Reallocate a previously aligned allocation.
  ///
  /// # Safety
  ///
  /// `ptr` must have been returned by one of `aligned_malloc`, `aligned_zalloc`, or `aligned_realloc` from 
  /// this dylib.
  ///
  /// # Behavior
  ///
  /// Refer to MiMalloc documentation for more details
  pub unsafe fn aligned_realloc(ptr: *mut c_void, size: usize, align: usize) -> *mut c_void;

  /// Frees an allocated pointer.
  pub unsafe fn aligned_free(ptr: *mut c_void);
}
