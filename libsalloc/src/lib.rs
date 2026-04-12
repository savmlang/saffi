#![cfg_attr(not(debug_assertions), no_std)]

use core::ffi::{c_int, c_void};
use libmimalloc_sys::{mi_free, mi_malloc_aligned, mi_realloc_aligned, mi_zalloc_aligned};

#[unsafe(no_mangle)]
pub unsafe extern "C" fn aligned_malloc(size: usize, align: usize) -> *mut c_void {
  unsafe { mi_malloc_aligned(size, align) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn aligned_zalloc(size: usize, align: usize) -> *mut c_void {
  unsafe { mi_zalloc_aligned(size, align) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn aligned_free(ptr: *mut c_void) {
  unsafe { mi_free(ptr) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn aligned_realloc(
  ptr: *mut c_void,
  size: usize,
  align: usize,
) -> *mut c_void {
  unsafe { mi_realloc_aligned(ptr, size, align) }
}

#[cfg_attr(target_os = "macos", link(name = "System"))]
unsafe extern "C" {
  pub fn exit(status: c_int) -> !;
}

#[cfg(not(debug_assertions))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
  unsafe { exit(1) };
}
