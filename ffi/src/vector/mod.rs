use core::ffi::c_void;
use core::{
    num::NonZeroUsize,
    ops::{Index, IndexMut},
    ptr,
};

use crate::FFISafe;

#[repr(C)]
/// Please make sure that the type given is a repr(C) type
/// The vector struct is based on this assumption
pub struct Vector<T: FFISafe + Sized> {
    ptr: *mut T,
    len: usize,
    cap: usize,
}

unsafe impl<T: FFISafe + Sized> FFISafe for Vector<T> {}

const fn calc<T>(count: usize) -> usize {
    count * size_of::<T>()
}

impl<T: FFISafe + Sized> Vector<T> {
    pub fn new() -> Self {
        let default_cap = 2;
        let ptr = unsafe {
            salloc::aligned_malloc(
                calc::<T>(default_cap),
                align_of::<T>().max(size_of::<*const c_void>()),
            )
        };

        if ptr.is_null() {
            panic!("Allocation Failed");
        }

        Self {
            ptr: ptr as _,
            len: 0,
            cap: default_cap,
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn cap(&self) -> usize {
        self.cap
    }

    pub fn allocate(&mut self, capacity: NonZeroUsize) {
        let capacity = capacity.get();

        if self.cap <= capacity {
            let new_cap = (self.cap * 2).max(capacity);
            let new_block = unsafe {
                salloc::aligned_realloc(
                    self.ptr as _,
                    calc::<T>(self.cap),
                    calc::<T>(new_cap),
                    align_of::<T>().max(size_of::<*const c_void>()),
                )
            };

            if new_block.is_null() {
                panic!("Allocation Failed");
            }

            self.ptr = new_block as _;
            self.cap = new_cap;
        }
    }

    pub fn push(&mut self, value: T) {
        // Capacity for a push is always at least 1, so this is safe.
        self.allocate(unsafe { NonZeroUsize::new_unchecked(self.len + 1) });

        unsafe {
            let ptr = self.ptr as *mut T;

            let dst = ptr.add(self.len);
            ptr::write(dst, value);

            self.len += 1;
        }
    }

    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        }

        unsafe {
            let ptr = self.ptr as *mut T;

            let to_drop = ptr.add(self.len - 1);

            self.len -= 1;

            Some(ptr::read(to_drop))
        }
    }
}

impl<T: FFISafe + Sized> Index<usize> for Vector<T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        if index >= self.len {
            panic!(
                "index out of bounds: the len is {} but the index is {}",
                self.len, index
            );
        }

        unsafe { &*self.ptr.add(index) as &T }
    }
}

impl<T: FFISafe + Sized> IndexMut<usize> for Vector<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        if index >= self.len {
            panic!(
                "index out of bounds: the len is {} but the index is {}",
                self.len, index
            );
        }

        unsafe { &mut *self.ptr.add(index) as &mut T }
    }
}

impl<T: FFISafe + Sized> Drop for Vector<T> {
    fn drop(&mut self) {
        unsafe {
            for i in (0..self.len).rev() {
                let ptr = self.ptr as *mut T;
                let to_drop = ptr.add(i);
                ptr::drop_in_place(to_drop);
            }

            salloc::aligned_free(self.ptr as _)
        };
    }
}
