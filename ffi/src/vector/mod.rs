use core::ffi::c_void;
use core::{
  num::NonZeroUsize,
  ops::{Index, IndexMut},
  ptr,
};
use std::hint::cold_path;
use std::mem::{MaybeUninit, forget, needs_drop, offset_of};
use std::num::NonZero;
use std::ops::{Deref, DerefMut};
use std::ptr::NonNull;

use crate::{
  FFISafe,
  I_DECLARE_THAT_I_AND_MY_CODEBASE_IS_FFI_SAFE_AND_THAT_UNDEFINED_BEHAVIOUR_ARISING_DUE_TO_DECLARING_MY_TYPES_FFI_SAFE_DOES_NOT_CONDONE_THE_SAFETY_AND_SECURITY_OF_THIS_PROJECT,
};

#[repr(C)]
pub struct VectorHeaderVTable<T: FFISafe + Sized> {
  len: usize,
  cap: usize,

  data: MaybeUninit<T>,
}

/// Returns the offset of T within RTBoxWrapper<T>.
/// This is guaranteed to be correct regardless of padding.
pub const fn data_offset<T: FFISafe>() -> isize {
  offset_of!(VectorHeaderVTable<T>, data) as _
}

/// Negative offset from T back to the Header:
pub const fn header_offset<T: FFISafe>() -> isize {
  -data_offset::<T>()
}

#[repr(C)]
/// Please make sure that the type given is a repr(C) type
/// The vector struct is based on this assumption
pub struct Vector<T: FFISafe + Sized> {
  ptr: NonNull<T>,
}

unsafe impl<T: FFISafe + Sized> FFISafe for Vector<T> {
  fn i_am_ffisafe() -> crate::IAmFFISafe {
    I_DECLARE_THAT_I_AND_MY_CODEBASE_IS_FFI_SAFE_AND_THAT_UNDEFINED_BEHAVIOUR_ARISING_DUE_TO_DECLARING_MY_TYPES_FFI_SAFE_DOES_NOT_CONDONE_THE_SAFETY_AND_SECURITY_OF_THIS_PROJECT
  }
}

const fn calc<T: FFISafe + Sized>(count: NonZeroUsize) -> usize {
  ((count.get() - 1) * size_of::<T>()) + size_of::<VectorHeaderVTable<T>>()
}

impl<T: FFISafe + Sized> Vector<T> {
  pub fn new() -> Self {
    const DEF_CAP: usize = 2;

    let ptr = unsafe {
      salloc::aligned_malloc(
        calc::<T>(NonZeroUsize::new_unchecked(DEF_CAP)),
        align_of::<VectorHeaderVTable<T>>().max(size_of::<*const c_void>()),
      )
    };

    if ptr.is_null() {
      panic!("Allocation Failed");
    }

    unsafe {
      // SAFETY:
      //
      // The data is not accessed, and hence is safe
      *(ptr as *mut VectorHeaderVTable<T>) = VectorHeaderVTable {
        len: 0,
        cap: DEF_CAP,
        data: MaybeUninit::uninit(),
      };
    }

    Self {
      ptr: unsafe { NonNull::new_unchecked(ptr.byte_offset(data_offset::<T>()) as *mut T) },
    }
  }

  #[inline(always)]
  pub fn len(&self) -> usize {
    unsafe {
      let nnull: *mut VectorHeaderVTable<T> =
        self.ptr.as_ptr().byte_offset(header_offset::<T>()) as _;

      (&*nnull).len
    }
  }

  #[inline(always)]
  fn set_len(&mut self, len: usize) {
    unsafe {
      let nnull: *mut VectorHeaderVTable<T> =
        self.ptr.as_ptr().byte_offset(header_offset::<T>()) as _;

      (&mut *nnull).len = len;
    }
  }

  #[inline(always)]
  pub fn cap(&self) -> usize {
    unsafe {
      let nnull: *mut VectorHeaderVTable<T> =
        self.ptr.as_ptr().byte_offset(header_offset::<T>()) as _;

      (&*nnull).cap
    }
  }

  #[inline(always)]
  fn set_cap(&mut self, cap: usize) {
    unsafe {
      let nnull: *mut VectorHeaderVTable<T> =
        self.ptr.as_ptr().byte_offset(header_offset::<T>()) as _;

      (&mut *nnull).cap = cap;
    }
  }

  #[inline(always)]
  pub fn allocate(&mut self, known_cap: Option<usize>, capacity: NonZeroUsize) {
    let capacity = capacity.get();

    let cap = known_cap.unwrap_or(self.cap());

    if cap < capacity {
      let new_cap = (cap * 2).max(capacity);

      let new_block = unsafe {
        salloc::aligned_realloc(
          self.ptr.as_ptr().byte_offset(header_offset::<T>()) as _,
          calc::<T>(NonZero::new_unchecked(new_cap)),
          align_of::<VectorHeaderVTable<T>>().max(size_of::<*const c_void>()),
        )
      };

      if new_block.is_null() {
        panic!("Allocation Failed");
      }

      unsafe {
        self.ptr = NonNull::new_unchecked(new_block.byte_offset(data_offset::<T>()) as _);

        self.set_cap(new_cap);
      }
    }
  }

  #[inline(always)]
  pub fn push(&mut self, value: T) {
    unsafe {
      self.push_aided(None, None, value);
    }
  }

  #[inline(always)]
  /// This is like jumping off a cliff with an untested parachute
  /// you might mess up, you might not, or, even better - you might invite someone never known
  ///
  /// Be careful and explicit and only pass the known_* if you're absolutely-drabsolutely sure about it!
  pub unsafe fn push_aided(
    &mut self,
    known_len: Option<usize>,
    known_cap: Option<usize>,
    value: T,
  ) {
    let len = known_len.unwrap_or(self.len());

    // Capacity for a push is always at least 1, so this is safe.
    self.allocate(known_cap, unsafe { NonZeroUsize::new_unchecked(len + 1) });

    unsafe {
      let ptr = self.ptr.as_ptr() as *mut T;

      let dst = ptr.add(len);
      ptr::write(dst, value);

      self.set_len(len + 1);
    }
  }

  #[inline(always)]
  pub fn extend<I>(&mut self, iter: I)
  where
    I: IntoIterator<Item = T>,
  {
    unsafe {
      self.extend_aided(None, None, iter);
    }
  }

  #[inline(always)]
  pub unsafe fn extend_aided<I>(
    &mut self,
    known_len: Option<usize>,
    known_cap: Option<usize>,
    iter: I,
  ) where
    I: IntoIterator<Item = T>,
  {
    let mut iterator = iter.into_iter();
    let (lower, _) = iterator.size_hint();

    let mut len = known_len.unwrap_or(self.len());

    // Pre-allocate based on the lower bound to save on reallocs
    if lower > 0 {
      self.allocate(known_cap, unsafe {
        NonZeroUsize::new_unchecked(len + lower)
      });
    }

    // We still have to loop, but if the iterator is 'TrustedLen',
    // the compiler can optimize this loop into a single block move.
    while let Some(item) = iterator.next() {
      unsafe {
        self.push_aided(Some(len), known_cap, item);
      }

      len += 1;
    }
  }

  pub fn into_raw(self) -> *mut T {
    let data = self.ptr.as_ptr();
    forget(self);

    data
  }

  pub unsafe fn from_raw(ptr: NonNull<T>) -> Self {
    Self { ptr }
  }

  pub fn extend_array<const N: usize>(&mut self, value: [T; N]) {
    unsafe { self.extend_array_aided(None, None, value) };
  }

  pub unsafe fn extend_array_aided<const N: usize>(
    &mut self,
    known_len: Option<usize>,
    known_cap: Option<usize>,
    value: [T; N],
  ) {
    let len = known_len.unwrap_or(self.len());

    self.allocate(known_cap, unsafe { NonZeroUsize::new_unchecked(len + N) });

    unsafe {
      let dst = self.ptr.as_ptr().add(len);
      // Move the bits
      ptr::copy_nonoverlapping(value.as_ptr(), dst, N);

      self.set_len(len + N);

      // CRITICAL: Prevent the stack-based clones from dropping!
      // This is safe because we just took ownership and moved them.
      forget(value);
    }
  }

  pub fn extend_from_slice(&mut self, value: &[T])
  where
    T: Copy,
  {
    unsafe { self.extend_from_slice_aided(None, None, value) };
  }

  pub unsafe fn extend_from_slice_aided(
    &mut self,
    known_cap: Option<usize>,
    known_len: Option<usize>,
    value: &[T],
  ) where
    T: Copy,
  {
    let len = known_len.unwrap_or(self.len());
    let new_len = len + value.len();

    if new_len == 0 {
      return;
    }

    self.allocate(known_cap, unsafe { NonZeroUsize::new_unchecked(new_len) });

    unsafe {
      let dst = self.ptr.as_ptr().add(len);

      ptr::copy_nonoverlapping(value.as_ptr(), dst, value.len());

      self.set_len(new_len);
    }
  }

  #[inline(always)]
  pub unsafe fn get_aided(&self, known_len: Option<usize>, index: usize) -> Option<&T> {
    let len = known_len.unwrap_or(self.len());

    if index >= len {
      return None;
    }

    Some(unsafe { &*self.ptr.as_ptr().add(index) as &T })
  }

  #[inline(always)]
  pub unsafe fn get_mut_aided(&mut self, known_len: Option<usize>, index: usize) -> Option<&mut T> {
    let len = known_len.unwrap_or(self.len());

    if index >= len {
      return None;
    }

    Some(unsafe { &mut *self.ptr.as_ptr().add(index) as &mut T })
  }

  #[inline(always)]
  pub fn pop(&mut self) -> Option<T> {
    unsafe { self.pop_aided(None) }
  }

  #[inline(always)]
  pub unsafe fn pop_aided(&mut self, known_len: Option<usize>) -> Option<T> {
    let len = known_len.unwrap_or(self.len());

    if len == 0 {
      cold_path();
      return None;
    }

    unsafe {
      let ptr = self.ptr.as_ptr() as *mut T;

      let to_drop = ptr.add(len - 1);

      self.set_len(len - 1);

      Some(ptr::read(to_drop))
    }
  }
}

impl<T: FFISafe + Sized> Index<usize> for Vector<T> {
  type Output = T;

  fn index(&self, index: usize) -> &Self::Output {
    unsafe {
      let Some(out) = self.get_aided(None, index) else {
        cold_path();

        panic!(
          "index out of bounds: the len is {} but the index is {}",
          self.len(),
          index
        );
      };

      out
    }
  }
}

impl<T: FFISafe + Sized> IndexMut<usize> for Vector<T> {
  fn index_mut(&mut self, index: usize) -> &mut Self::Output {
    unsafe {
      let Some(out) = self.get_mut_aided(None, index) else {
        cold_path();

        panic!("index out of bounds: the index {} is out of bounds", index);
      };

      out
    }
  }
}

impl<T: FFISafe + Sized> Deref for Vector<T> {
  type Target = [T];

  fn deref(&self) -> &Self::Target {
    unsafe { core::slice::from_raw_parts(self.ptr.as_ptr(), self.len()) }
  }
}

impl<T: FFISafe + Sized> DerefMut for Vector<T> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    unsafe { core::slice::from_raw_parts_mut(self.ptr.as_ptr(), self.len()) }
  }
}

impl<T: FFISafe + Sized> Drop for Vector<T> {
  fn drop(&mut self) {
    unsafe {
      if needs_drop::<T>() {
        for i in (0..self.len()).rev() {
          let ptr = self.ptr.as_ptr() as *mut T;
          let to_drop = ptr.add(i);
          ptr::drop_in_place(to_drop);
        }
      }

      salloc::aligned_free(self.ptr.as_ptr().byte_offset(header_offset::<T>()) as _)
    };
  }
}
