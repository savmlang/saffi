use std::ffi::c_void;

#[unsafe(no_mangle)]
#[cfg(windows)]
pub unsafe extern "C" fn aligned_malloc(size: usize, align: usize) -> *mut c_void {
  debug_assert!(size.is_multiple_of(align));

  unsafe { libc::aligned_malloc(size, align) }
}

#[unsafe(no_mangle)]
#[cfg(not(windows))]
pub unsafe extern "C" fn aligned_malloc(size: usize, align: usize) -> *mut c_void {
  debug_assert!(size.is_multiple_of(align));

  unsafe { libc::aligned_alloc(align, size) }
}

#[unsafe(no_mangle)]
#[cfg(windows)]
pub unsafe extern "C" fn aligned_free(ptr: *mut c_void) {
  unsafe { libc::aligned_free(ptr) }
}

#[unsafe(no_mangle)]
#[cfg(not(windows))]
pub unsafe extern "C" fn aligned_free(ptr: *mut c_void) {
  unsafe { libc::free(ptr) }
}

#[unsafe(no_mangle)]
#[cfg(windows)]
pub unsafe extern "C" fn aligned_realloc(
  ptr: *mut c_void,
  _old: usize,
  size: usize,
  align: usize,
) -> *mut c_void {
  debug_assert!(size.is_multiple_of(align));

  unsafe { libc::aligned_realloc(ptr, size, align) }
}

#[unsafe(no_mangle)]
#[cfg(not(windows))]
pub unsafe extern "C" fn aligned_realloc(
  ptr: *mut c_void,
  old: usize,
  size: usize,
  align: usize,
) -> *mut c_void {
  debug_assert!(size.is_multiple_of(align));

  unsafe {
    if ptr.is_null() {
      return libc::aligned_alloc(align, size);
    }

    if size == 0 {
      libc::free(ptr);
      return core::ptr::null_mut();
    }

    let new_block = libc::aligned_alloc(align, size);

    if new_block.is_null() {
      return core::ptr::null_mut();
    }

    let copy_size = if old < size { old } else { size };

    libc::memcpy(new_block, ptr, copy_size);

    libc::free(ptr);

    new_block
  }
}
