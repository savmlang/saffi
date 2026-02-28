#![feature(type_alias_impl_trait, cold_path)]

use core::ffi::c_void;

pub mod arc;
pub mod boxed;
pub mod futures;
pub mod string;
pub mod vector;

pub unsafe trait FFISafe {}

macro_rules! ffisafe {
  ($($x:ty),+) => {
    $(
      unsafe impl FFISafe for $x {}
    )*
  }
}

ffisafe! {
  u8,
  u16,
  u32,
  u64,
  i8,
  i16,
  i32,
  i64,
  usize,
  isize,
  c_void
}

unsafe impl<T> FFISafe for *const T {}
unsafe impl<T> FFISafe for *mut T {}
