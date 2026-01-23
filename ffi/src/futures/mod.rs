use std::{
    os::raw::c_void,
    ptr::{self, null_mut},
    task::{Poll, Waker},
};

use crate::{
    FFISafe,
    boxed::{ContainedRTBox, RTBox, RTSafeBoxWrapper},
};

pub mod implements;

#[repr(C)]
pub struct FutureTask {
    // We don't care about the state
    pub _state: *mut c_void,
    // We expect you to clear state here!
    pub _collect: extern "C" fn(*mut c_void) -> *mut RTSafeBoxWrapper,
    pub _waker: extern "C" fn(*mut c_void, *mut WakerData) -> (),
    pub _ready: extern "C" fn(*mut c_void) -> bool,
    pub _clean: extern "C" fn(*mut c_void) -> (),
}

unsafe impl FFISafe for FutureTask {}

#[repr(C)]
pub struct WakerData {
    waker: *mut c_void,
    call: extern "C" fn(*mut c_void),
    drop: extern "C" fn(*mut c_void),
}

unsafe impl Send for WakerData {}
unsafe impl Sync for WakerData {}

pub struct FFIWaker {
    _data: *mut WakerData,
}

unsafe impl Send for FFIWaker {}
unsafe impl Sync for FFIWaker {}

impl FFIWaker {
    pub fn use_waker(ptr: *mut WakerData) -> Self {
        Self { _data: ptr }
    }

    pub fn call(&self) {
        unsafe {
            let data = &*(self._data);

            (data.call)(data.waker);
        }
    }
}

impl Drop for FFIWaker {
    fn drop(&mut self) {
        unsafe {
            let data = &*self._data;

            (data.drop)(data.waker);
        }
    }
}

extern "C" fn drop_waker(ptr: *mut c_void) {
    unsafe {
        _ = Box::from_raw(ptr as *mut Waker);
    }
}

extern "C" fn wake(ptr: *mut c_void) {
    unsafe {
        let waker = Box::from_raw(ptr as *mut Waker);

        waker.wake_by_ref();

        _ = Box::into_raw(waker);
    }
}

impl Future for RTBox<FutureTask> {
    type Output = ContainedRTBox;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        unsafe {
            let has_data = (self._ready)(self._state);

            let val = if has_data {
                (self._collect)(self._state)
            } else {
                null_mut::<RTSafeBoxWrapper>()
            };

            if !has_data {
                let waker = Box::into_raw(Box::new(cx.waker().clone())) as *mut c_void;

                let waker_struct = WakerData {
                    call: wake,
                    waker,
                    drop: drop_waker,
                };

                let alloc = salloc::aligned_malloc(size_of::<WakerData>(), align_of::<WakerData>())
                    as *mut WakerData;

                if !alloc.is_null() {
                    ptr::write(alloc, waker_struct);

                    (self._waker)(self._state, alloc);
                }

                return Poll::Pending;
            }

            (self._clean)(self._state);

            Poll::Ready(ContainedRTBox::new(val))
        }
    }
}
