use std::ffi::c_void;

#[cfg_attr(windows, link(name = "salloc", kind = "raw-dylib"))]
#[cfg_attr(not(windows), link(name = "salloc", kind = "dylib"))]
unsafe extern "C" {
  pub unsafe fn aligned_malloc(size: usize, align: usize) -> *mut c_void;
  pub unsafe fn aligned_realloc(
    ptr: *mut c_void,
    old: usize,
    size: usize,
    align: usize,
  ) -> *mut c_void;
  pub unsafe fn aligned_free(ptr: *mut c_void);
}
