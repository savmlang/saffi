use core::{
  ops::Deref,
  ptr, slice,
  str::{self, Utf8Error},
};
use std::{mem::offset_of, ptr::NonNull};

#[repr(C)]
pub struct SharedStrVTHelper {
  len: usize,
  raw: (),
}

const OFFSET: isize = offset_of!(SharedStrVTHelper, raw) as isize;
const NEG_OFFSET: isize = -(offset_of!(SharedStrVTHelper, raw) as isize);

#[repr(C)]
pub struct SharableStr {
  ptr: NonNull<u8>,
}

impl SharableStr {
  pub fn create(data: &str) -> Self {
    let length = data.len();
    let alloc_size = data.len() * size_of::<u8>() + size_of::<SharedStrVTHelper>();

    let _raw = unsafe { salloc::aligned_malloc(alloc_size, align_of::<usize>()) } as *mut u8;

    unsafe {
      ptr::write(
        _raw as *mut SharedStrVTHelper,
        SharedStrVTHelper {
          len: length,
          raw: (),
        },
      );
    }

    unsafe {
      let pointer = data.as_ptr();
      let dst = _raw.offset(OFFSET);

      ptr::copy_nonoverlapping(pointer, dst, length);

      Self {
        ptr: NonNull::new_unchecked(dst),
      }
    }
  }

  pub const fn into_raw(&mut self) -> *mut u8 {
    self.ptr.as_ptr()
  }

  pub unsafe fn from_raw(ptr: *mut u8) -> Option<Self> {
    Some(Self {
      ptr: NonNull::new(ptr)?,
    })
  }

  pub const unsafe fn from_nonnull(ptr: NonNull<u8>) -> Self {
    Self { ptr }
  }

  /// Please note that the lifetime <'a> refers to the lifetime of the
  /// const reference
  /// Please ensure that the const reference stays as long as <'a>
  ///
  /// Also, this function does not check if the data is valid utf8 or not
  pub const unsafe fn as_str_unchecked<'a>(data: &Self) -> &'a str {
    let data_ptr = data.ptr.as_ptr();

    let len = unsafe { ptr::read((data_ptr as *mut usize).byte_offset(NEG_OFFSET)) };

    unsafe { str::from_utf8_unchecked(slice::from_raw_parts(data_ptr, len)) }
  }

  /// Please note that the lifetime <'a> refers to the lifetime of the
  /// const reference
  /// Please ensure that the const reference stays as long as <'a>
  pub const unsafe fn as_str<'a>(data: &Self) -> Result<&'a str, Utf8Error> {
    let data_ptr = data.ptr.as_ptr();

    let len = unsafe { ptr::read((data_ptr as *mut usize).byte_offset(NEG_OFFSET)) };

    unsafe { str::from_utf8(slice::from_raw_parts(data_ptr, len)) }
  }
}

impl Deref for SharableStr {
  type Target = str;

  fn deref(&self) -> &Self::Target {
    unsafe { Self::as_str(self).expect("Invalid UTF8 Data") }
  }
}

impl Drop for SharableStr {
  fn drop(&mut self) {
    unsafe {
      salloc::aligned_free(self.ptr.as_ptr().byte_offset(NEG_OFFSET) as _);
    }
  }
}
