use core::{
    ops::Deref,
    ptr, slice,
    str::{self, Utf8Error},
};

#[repr(C)]
pub struct SharableStr {
    _raw: *mut u8,
    len: usize,
}

impl SharableStr {
    pub fn create(data: &str) -> Self {
        let length = data.len();

        let _raw = unsafe { salloc::aligned_malloc(length * size_of::<u8>(), align_of::<u8>()) }
            as *mut u8;

        let pointer = data.as_ptr();
        unsafe { ptr::copy_nonoverlapping(pointer, _raw, length) };

        Self {
            _raw: _raw,
            len: length,
        }
    }

    /// Please note that the lifetime <'a> refers to the lifetime of the
    /// const reference
    /// Please ensure that the const reference stays as long as <'a>
    ///
    /// Also, this function does not check if the data is valid utf8 or not
    pub unsafe fn as_str_unchecked<'a>(data: *const Self) -> &'a str {
        let data = unsafe { &*data };

        unsafe { str::from_utf8_unchecked(slice::from_raw_parts(data._raw, data.len)) }
    }

    /// Please note that the lifetime <'a> refers to the lifetime of the
    /// const reference
    /// Please ensure that the const reference stays as long as <'a>
    pub unsafe fn as_str<'a>(data: *const Self) -> Result<&'a str, Utf8Error> {
        let data = unsafe { &*data };

        unsafe { str::from_utf8(slice::from_raw_parts(data._raw, data.len)) }
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
            salloc::aligned_free(self._raw as _);
        }
    }
}
