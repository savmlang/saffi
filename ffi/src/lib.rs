use core::ffi::c_void;

pub mod boxed;
pub mod futures;
pub mod string;
pub mod vector;

pub use salloc;

#[doc(hidden)]
pub struct IAmFFISafe(());

#[doc(hidden)]
pub const I_DECLARE_THAT_I_AND_MY_CODEBASE_IS_FFI_SAFE_AND_THAT_UNDEFINED_BEHAVIOUR_ARISING_DUE_TO_DECLARING_MY_TYPES_FFI_SAFE_DOES_NOT_CONDONE_THE_SAFETY_AND_SECURITY_OF_THIS_PROJECT: IAmFFISafe = IAmFFISafe(());

pub unsafe trait FFISafe: Sized {
  #[doc(hidden)]
  fn i_am_ffisafe() -> IAmFFISafe;
}

macro_rules! ffisafe {
  ($($x:ty),+) => {
    $(
      unsafe impl FFISafe for $x {
        fn i_am_ffisafe() -> IAmFFISafe {
          I_DECLARE_THAT_I_AND_MY_CODEBASE_IS_FFI_SAFE_AND_THAT_UNDEFINED_BEHAVIOUR_ARISING_DUE_TO_DECLARING_MY_TYPES_FFI_SAFE_DOES_NOT_CONDONE_THE_SAFETY_AND_SECURITY_OF_THIS_PROJECT
        }
      }
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

unsafe impl<T> FFISafe for *const T {
  fn i_am_ffisafe() -> IAmFFISafe {
    I_DECLARE_THAT_I_AND_MY_CODEBASE_IS_FFI_SAFE_AND_THAT_UNDEFINED_BEHAVIOUR_ARISING_DUE_TO_DECLARING_MY_TYPES_FFI_SAFE_DOES_NOT_CONDONE_THE_SAFETY_AND_SECURITY_OF_THIS_PROJECT
  }
}
unsafe impl<T> FFISafe for *mut T {
  fn i_am_ffisafe() -> IAmFFISafe {
    I_DECLARE_THAT_I_AND_MY_CODEBASE_IS_FFI_SAFE_AND_THAT_UNDEFINED_BEHAVIOUR_ARISING_DUE_TO_DECLARING_MY_TYPES_FFI_SAFE_DOES_NOT_CONDONE_THE_SAFETY_AND_SECURITY_OF_THIS_PROJECT
  }
}

#[cfg(test)]
pub mod tests;
