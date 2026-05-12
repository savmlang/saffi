#![no_std]

use core::ffi::c_void;

#[cfg_attr(windows, link(name = "salloc", kind = "raw-dylib"))]
#[cfg_attr(not(windows), link(name = "salloc", kind = "dylib"))]
unsafe extern "C" {
  pub unsafe fn aligned_malloc(size: usize, align: usize) -> *mut c_void;

  pub unsafe fn aligned_zalloc(size: usize, align: usize) -> *mut c_void;

  /// Reallocate a previously aligned allocation.
  ///
  /// # Safety
  ///
  /// - `ptr` must either be null, or must have been returned by one of
  ///   `aligned_malloc`, `aligned_zalloc`, or `aligned_realloc` from this
  ///   library.
  /// - If `ptr` is non-null, `align` **must** be equal to the alignment that
  ///   was used for the original allocation. Passing a different alignment is
  ///   unsupported and may result in undefined behavior.
  /// - `align` must satisfy the same requirements as for `aligned_malloc`
  ///   (typically a power of two and a multiple of the platform pointer size).
  ///
  /// # Behavior
  ///
  /// - If `ptr` is null, this function behaves like `aligned_malloc(size,
  ///   align)`.
  /// - On success, it returns a pointer to a block of memory of at least
  ///   `size` bytes with the requested alignment. The contents up to the
  ///   lesser of the old and new sizes are preserved.
  /// - On failure, it returns a null pointer and the original allocation (if
  ///   any) remains valid and unchanged.
  pub unsafe fn aligned_realloc(ptr: *mut c_void, size: usize, align: usize) -> *mut c_void;

  pub unsafe fn aligned_free(ptr: *mut c_void);
}
