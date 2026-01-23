#![feature(prelude_import)]
#[macro_use]
extern crate std;
use std::ffi::c_void;
#[prelude_import]
use std::prelude::rust_2024::*;
pub mod arc {
    use crate::FFISafe;
    use std::{ffi::c_void, marker::PhantomData, ops::Deref, sync::Arc};
    #[repr(C)]
    pub enum Maybe {
        Yes(*mut c_void),
        No,
    }
    #[repr(C)]
    /// Please note that the Arc may not be like you think
    /// You need to make sure that you've not overused the Arc
    ///
    /// You must also be aware that this Arc might be destroyed at any moment!
    ///
    /// This data type uses the rust allocator as mutual ownership is not possible to implement for
    /// this structure
    ///
    /// ## DANGER
    ///
    /// The Arc struct is automatically deallocated as soon as no dynamic link library
    /// has an [Arced] struct regardless if you have the pointer of not.
    pub struct Arced<T: FFISafe> {
        _inner: *const c_void,
        _use: extern "C" fn(ptr: *const c_void),
        _unuse: extern "C" fn(ptr: *const c_void),
        _marker: PhantomData<T>,
    }
    extern "C" fn _use_arc<T>(ptr: *const c_void) {
        unsafe { Arc::increment_strong_count(ptr) };
    }
    extern "C" fn _uunuse_arc<T>(ptr: *const c_void) {
        unsafe { Arc::decrement_strong_count(ptr) };
    }
    impl<T: FFISafe> Arced<T> {
        /// Returns the arc along with a free function that can be called to free it directly
        pub fn new(data: T) -> Self {
            let data = Arc::into_raw(Arc::new(data));
            Self {
                _inner: data as *const c_void,
                _unuse: _uunuse_arc::<T>,
                _use: _use_arc::<T>,
                _marker: PhantomData,
            }
        }
        pub fn from_raw(arc: *const Self) -> Self {
            let rf = unsafe { &*arc };
            (rf._use)(rf._inner);
            Self {
                _use: rf._use,
                _inner: rf._inner,
                _unuse: rf._unuse,
                _marker: PhantomData::<T>,
            }
        }
        pub fn as_raw(&self) -> *const Self {
            self as _
        }
    }
    impl<T: FFISafe> Deref for Arced<T> {
        type Target = T;
        fn deref(&self) -> &Self::Target {
            unsafe { &*(self._inner as *const T) }
        }
    }
    impl<T: FFISafe> Drop for Arced<T> {
        fn drop(&mut self) {
            (self._unuse)(self._inner);
        }
    }
}
pub mod boxed {
    use crate::FFISafe;
    use std::{
        ffi::c_void,
        marker::PhantomData,
        ops::{Deref, DerefMut},
        ptr,
    };
    #[repr(C)]
    pub struct RTSafeBoxWrapper {
        _data: *mut c_void,
        _free: unsafe extern "C" fn(data: *mut c_void),
    }
    unsafe extern "C" fn mfree<T>(data: *mut c_void) {
        unsafe {
            ptr::drop_in_place(data as *mut T);
            salloc::aligned_free(data);
        }
    }
    impl RTSafeBoxWrapper {
        pub fn new<T: FFISafe>(data: T) -> RTBox<T> {
            unsafe {
                let alignment = align_of::<T>();
                let _data = salloc::aligned_malloc(size_of::<T>(), alignment);
                if _data.is_null() {
                    {
                        ::core::panicking::panic_fmt(format_args!("Unable to construct"));
                    };
                }
                ptr::write(_data as _, data);
                let structdata = Self {
                    _data: _data as _,
                    _free: mfree::<T>,
                };
                let structdata_ptr =
                    salloc::aligned_malloc(size_of::<Self>(), align_of::<Self>()) as *mut Self;
                if structdata_ptr.is_null() {
                    salloc::aligned_free(_data);
                    {
                        ::core::panicking::panic_fmt(format_args!("Unable to construct"));
                    };
                }
                ptr::write(structdata_ptr, structdata);
                RTBox {
                    _wrap: structdata_ptr,
                    _data: PhantomData,
                    poisoned: false,
                }
            }
        }
        /// You, the developer is required to ensure that `T` is correct
        /// This constructs a Wrapper Type that's not FFI-able
        pub unsafe fn construct<T: FFISafe>(pointer: *mut RTSafeBoxWrapper) -> RTBox<T> {
            RTBox {
                _wrap: pointer,
                _data: PhantomData,
                poisoned: false,
            }
        }
    }
    pub struct RTBox<T: FFISafe> {
        _wrap: *mut RTSafeBoxWrapper,
        _data: PhantomData<T>,
        poisoned: bool,
    }
    unsafe impl<T: FFISafe> Send for RTBox<T> {}
    unsafe impl<T: FFISafe> Sync for RTBox<T> {}
    impl<T: FFISafe> RTBox<T> {
        pub fn into_raw(mut self) -> *mut RTSafeBoxWrapper {
            self.poisoned = true;
            self._wrap
        }
    }
    impl<T: FFISafe + Clone> RTBox<T> {
        pub fn unwrap(self) -> T {
            (&*self).clone()
        }
    }
    impl<T: FFISafe> Deref for RTBox<T> {
        type Target = T;
        fn deref(&self) -> &Self::Target {
            unsafe { &*(*self._wrap)._data.cast::<T>() }
        }
    }
    impl<T: FFISafe> DerefMut for RTBox<T> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            unsafe { &mut *(*self._wrap)._data.cast::<T>() }
        }
    }
    impl<T: FFISafe> Drop for RTBox<T> {
        fn drop(&mut self) {
            if !self.poisoned {
                let boxed = unsafe { &*self._wrap };
                unsafe {
                    (boxed._free)(boxed._data);
                    salloc::aligned_free(self._wrap as *mut c_void);
                }
            }
        }
    }
}
pub mod ctr {
    use crate::boxed::RTSafeBoxWrapper;
    use std::num::NonZeroU32;
    /// This is the program registry state
    /// This is created for each thread executed
    /// under the Runtime
    pub struct TaskState {
        pub r1: Option<NonZeroU32>,
        pub r2: Option<NonZeroU32>,
        pub r3: Option<NonZeroU32>,
        pub r4: Option<NonZeroU32>,
        pub r5: Option<NonZeroU32>,
        pub r6: Option<NonZeroU32>,
        pub r7: Option<NonZeroU32>,
        pub r8: *mut RTSafeBoxWrapper,
    }
    unsafe impl Send for TaskState {}
    pub const INTRUCTION_MOV: u8 = 0x01;
    #[allow(non_upper_case_globals, dead_code)]
    const _FORFIXING_0x01: u8 = 0;
    pub const INTRUCTION_CLR: u8 = 0x02;
    #[allow(non_upper_case_globals, dead_code)]
    const _FORFIXING_0x02: u8 = 0;
    pub const INTRUCTION_CLRS: u8 = 0x03;
    #[allow(non_upper_case_globals, dead_code)]
    const _FORFIXING_0x03: u8 = 0;
    pub const INTRUCTION_ALLOC: u8 = 0x04;
    #[allow(non_upper_case_globals, dead_code)]
    const _FORFIXING_0x04: u8 = 0;
    pub const INTRUCTION_ARALC: u8 = 0x05;
    #[allow(non_upper_case_globals, dead_code)]
    const _FORFIXING_0x05: u8 = 0;
    pub const INTRUCTION_LOAD: u8 = 0x06;
    #[allow(non_upper_case_globals, dead_code)]
    const _FORFIXING_0x06: u8 = 0;
    pub const INTRUCTION_FREE: u8 = 0x07;
    #[allow(non_upper_case_globals, dead_code)]
    const _FORFIXING_0x07: u8 = 0;
    pub const INTRUCTION_OWN: u8 = 0x08;
    #[allow(non_upper_case_globals, dead_code)]
    const _FORFIXING_0x08: u8 = 0;
    pub const INTRUCTION_ADD: u8 = 0x09;
    #[allow(non_upper_case_globals, dead_code)]
    const _FORFIXING_0x09: u8 = 0;
    pub const INTRUCTION_SUB: u8 = 0x0A;
    #[allow(non_upper_case_globals, dead_code)]
    const _FORFIXING_0x0A: u8 = 0;
    pub const INTRUCTION_MUL: u8 = 0x0B;
    #[allow(non_upper_case_globals, dead_code)]
    const _FORFIXING_0x0B: u8 = 0;
    pub const INTRUCTION_DIV: u8 = 0x0C;
    #[allow(non_upper_case_globals, dead_code)]
    const _FORFIXING_0x0C: u8 = 0;
    pub const INTRUCTION_REM: u8 = 0x0D;
    #[allow(non_upper_case_globals, dead_code)]
    const _FORFIXING_0x0D: u8 = 0;
    pub const INTRUCTION_ADD_MUT: u8 = 0x0E;
    #[allow(non_upper_case_globals, dead_code)]
    const _FORFIXING_0x0E: u8 = 0;
    pub const INTRUCTION_SUB_MUT: u8 = 0x0F;
    #[allow(non_upper_case_globals, dead_code)]
    const _FORFIXING_0x0F: u8 = 0;
    pub const INTRUCTION_MUL_MUT: u8 = 0x10;
    #[allow(non_upper_case_globals, dead_code)]
    const _FORFIXING_0x10: u8 = 0;
    pub const INTRUCTION_DIV_MUT: u8 = 0x11;
    #[allow(non_upper_case_globals, dead_code)]
    const _FORFIXING_0x11: u8 = 0;
    pub const INTRUCTION_REM_MUT: u8 = 0x12;
    #[allow(non_upper_case_globals, dead_code)]
    const _FORFIXING_0x12: u8 = 0;
    pub const INTRUCTION_AND: u8 = 0x13;
    #[allow(non_upper_case_globals, dead_code)]
    const _FORFIXING_0x13: u8 = 0;
    pub const INTRUCTION_OR: u8 = 0x14;
    #[allow(non_upper_case_globals, dead_code)]
    const _FORFIXING_0x14: u8 = 0;
    pub const INTRUCTION_XOR: u8 = 0x15;
    #[allow(non_upper_case_globals, dead_code)]
    const _FORFIXING_0x15: u8 = 0;
    pub const INTRUCTION_AND_MUT: u8 = 0x16;
    #[allow(non_upper_case_globals, dead_code)]
    const _FORFIXING_0x16: u8 = 0;
    pub const INTRUCTION_OR_MUT: u8 = 0x17;
    #[allow(non_upper_case_globals, dead_code)]
    const _FORFIXING_0x17: u8 = 0;
    pub const INTRUCTION_XOR_MUT: u8 = 0x18;
    #[allow(non_upper_case_globals, dead_code)]
    const _FORFIXING_0x18: u8 = 0;
    pub const INTRUCTION_CMP: u8 = 0x19;
    #[allow(non_upper_case_globals, dead_code)]
    const _FORFIXING_0x19: u8 = 0;
    pub const INTRUCTION_SHL: u8 = 0x1A;
    #[allow(non_upper_case_globals, dead_code)]
    const _FORFIXING_0x1A: u8 = 0;
    pub const INTRUCTION_SHR: u8 = 0x1B;
    #[allow(non_upper_case_globals, dead_code)]
    const _FORFIXING_0x1B: u8 = 0;
    pub const INTRUCTION_SHL_MUT: u8 = 0x1C;
    #[allow(non_upper_case_globals, dead_code)]
    const _FORFIXING_0x1C: u8 = 0;
    pub const INTRUCTION_SHR_MUT: u8 = 0x1D;
    #[allow(non_upper_case_globals, dead_code)]
    const _FORFIXING_0x1D: u8 = 0;
    pub const INTRUCTION_JMP: u8 = 0x1E;
    #[allow(non_upper_case_globals, dead_code)]
    const _FORFIXING_0x1E: u8 = 0;
    pub const INTRUCTION_JZ: u8 = 0x1F;
    #[allow(non_upper_case_globals, dead_code)]
    const _FORFIXING_0x1F: u8 = 0;
    pub const INTRUCTION_JNZ: u8 = 0x20;
    #[allow(non_upper_case_globals, dead_code)]
    const _FORFIXING_0x20: u8 = 0;
    pub const INTRUCTION_RET: u8 = 0x21;
    #[allow(non_upper_case_globals, dead_code)]
    const _FORFIXING_0x21: u8 = 0;
    pub const INTRUCTION_LIBCALL: u8 = 0x22;
    #[allow(non_upper_case_globals, dead_code)]
    const _FORFIXING_0x22: u8 = 0;
    pub const INTRUCTION_FORK: u8 = 0x23;
    #[allow(non_upper_case_globals, dead_code)]
    const _FORFIXING_0x23: u8 = 0;
    pub const INTRUCTION_JOIN: u8 = 0x24;
    #[allow(non_upper_case_globals, dead_code)]
    const _FORFIXING_0x24: u8 = 0;
    pub const INTRUCTION_YIELD: u8 = 0x25;
    #[allow(non_upper_case_globals, dead_code)]
    const _FORFIXING_0x25: u8 = 0;
    pub const INTRUCTION_AWAIT: u8 = 0x26;
    #[allow(non_upper_case_globals, dead_code)]
    const _FORFIXING_0x26: u8 = 0;
    pub fn parse_instrution(inst: &str) -> Option<u8> {
        match inst {
            "mov" => Some(0x01),
            "clr" => Some(0x02),
            "clrs" => Some(0x03),
            "alloc" => Some(0x04),
            "aralc" => Some(0x05),
            "load" => Some(0x06),
            "free" => Some(0x07),
            "own" => Some(0x08),
            "add" => Some(0x09),
            "sub" => Some(0x0A),
            "mul" => Some(0x0B),
            "div" => Some(0x0C),
            "rem" => Some(0x0D),
            "add_mut" => Some(0x0E),
            "sub_mut" => Some(0x0F),
            "mul_mut" => Some(0x10),
            "div_mut" => Some(0x11),
            "rem_mut" => Some(0x12),
            "and" => Some(0x13),
            "or" => Some(0x14),
            "xor" => Some(0x15),
            "and_mut" => Some(0x16),
            "or_mut" => Some(0x17),
            "xor_mut" => Some(0x18),
            "cmp" => Some(0x19),
            "shl" => Some(0x1A),
            "shr" => Some(0x1B),
            "shl_mut" => Some(0x1C),
            "shr_mut" => Some(0x1D),
            "jmp" => Some(0x1E),
            "jz" => Some(0x1F),
            "jnz" => Some(0x20),
            "ret" => Some(0x21),
            "libcall" => Some(0x22),
            "fork" => Some(0x23),
            "join" => Some(0x24),
            "yield" => Some(0x25),
            "await" => Some(0x26),
            _ => None,
        }
    }
}
pub mod futures {
    use crate::{
        FFISafe,
        boxed::{RTBox, RTSafeBoxWrapper},
    };
    use std::{
        marker::PhantomData,
        os::raw::c_void,
        ptr::{self, null_mut},
        task::{Poll, Waker},
    };
    pub mod implements {
        use crate::{
            FFISafe,
            boxed::{RTBox, RTSafeBoxWrapper},
            futures::{FFIWaker, FutureTask, WakerData},
        };
        use std::{ffi::c_void, marker::PhantomData, ptr::null_mut, sync::Arc, time::Duration};
        use tokio::{
            spawn,
            sync::{
                Mutex,
                mpsc::{Sender, channel},
            },
            task::JoinHandle,
            time,
        };
        struct FutState {
            output: Option<SafeContainer<*mut RTSafeBoxWrapper>>,
            finished: bool,
        }
        struct StateData {
            fut: Arc<Mutex<FutState>>,
            tx: Sender<FFIWaker>,
        }
        #[repr(transparent)]
        struct SafeContainer<T>(T);
        unsafe impl<T> Send for SafeContainer<T> {}
        unsafe impl<T> Sync for SafeContainer<T> {}
        extern "C" fn use_state(ptr: *mut c_void) -> *mut RTSafeBoxWrapper {
            let mut out = null_mut() as *mut _;
            unsafe {
                let data = Box::from_raw(ptr as *mut StateData);
                if let Ok(x) = data.fut.try_lock() {
                    if let Some(x) = x.output.as_ref() {
                        out = x.0;
                    }
                }
                _ = Box::into_raw(data);
            }
            out
        }
        extern "C" fn use_ready(ptr: *mut c_void) -> bool {
            let mut out = false;
            unsafe {
                let data = Box::from_raw(ptr as *mut StateData);
                if let Ok(x) = data.fut.try_lock() {
                    out = x.finished;
                }
                _ = Box::into_raw(data);
            }
            out
        }
        extern "C" fn waker(ptr: *mut c_void, waker: *mut WakerData) {
            unsafe {
                let data = Box::from_raw(ptr as *mut StateData);
                _ = data.tx.try_send(FFIWaker::use_waker(waker));
                _ = Box::into_raw(data);
            }
        }
        extern "C" fn clean_state(ptr: *mut c_void) {
            unsafe {
                drop(Box::from_raw(ptr as *mut StateData));
            }
        }
        pub fn create_future<T: FFISafe + 'static>(fut: JoinHandle<RTBox<T>>) -> FutureTask<T> {
            let local_state = Arc::new(Mutex::new(FutState {
                output: None,
                finished: false,
            }));
            let (tx, mut rx) = channel::<FFIWaker>(10);
            let state = local_state.clone();
            spawn(async move {
                let hwnd = fut;
                let mut waker = None;
                loop {
                    let mut lock = state.lock().await;
                    if let Ok(x) = rx.try_recv() {
                        waker = Some(x);
                    }
                    if hwnd.is_finished() {
                        lock.finished = true;
                        lock.output =
                            Some(SafeContainer(hwnd.await.expect("Unknown Error").into_raw()));
                        drop(lock);
                        break;
                    }
                    time::sleep(Duration::from_millis(10)).await;
                }
                if let Some(x) = waker {
                    x.call();
                }
                while let Some(x) = rx.recv().await {
                    x.call();
                }
            });
            let state = Box::into_raw(Box::new(StateData {
                fut: local_state,
                tx,
            })) as *mut c_void;
            FutureTask {
                _output: PhantomData,
                _state: state,
                _collect: use_state,
                _ready: use_ready,
                _waker: waker,
                _clean: clean_state,
            }
        }
    }
    #[repr(C)]
    pub struct FutureTask<T: FFISafe> {
        pub _state: *mut c_void,
        pub _collect: extern "C" fn(*mut c_void) -> *mut RTSafeBoxWrapper,
        pub _waker: extern "C" fn(*mut c_void, *mut WakerData) -> (),
        pub _ready: extern "C" fn(*mut c_void) -> bool,
        pub _clean: extern "C" fn(*mut c_void) -> (),
        pub _output: PhantomData<T>,
    }
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
    impl<T: FFISafe> Future for FutureTask<T> {
        type Output = RTBox<T>;
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
                    let alloc =
                        salloc::aligned_malloc(size_of::<WakerData>(), align_of::<WakerData>())
                            as *mut WakerData;
                    if !alloc.is_null() {
                        ptr::write(alloc, waker_struct);
                        (self._waker)(self._state, alloc);
                    }
                    return Poll::Pending;
                }
                (self._clean)(self._state);
                Poll::Ready(RTSafeBoxWrapper::construct(val))
            }
        }
    }
}
pub mod map {}
pub mod string {
    pub mod str {
        use std::{
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
                let _raw =
                    unsafe { salloc::aligned_malloc(length * size_of::<u8>(), align_of::<u8>()) }
                        as *mut u8;
                if _raw.is_null() {
                    {
                        ::std::io::_print(format_args!("ERR: Null\n"));
                    };
                }
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
    }
}
pub mod vector {
    use crate::FFISafe;
    use core::ffi::c_void;
    use std::{
        num::NonZeroUsize,
        ops::{Index, IndexMut},
        ptr,
    };
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
                {
                    ::core::panicking::panic_fmt(format_args!("Allocation Failed"));
                };
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
                    {
                        ::core::panicking::panic_fmt(format_args!("Allocation Failed"));
                    };
                }
                self.ptr = new_block as _;
                self.cap = new_cap;
            }
        }
        pub fn push(&mut self, value: T) {
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
                {
                    ::core::panicking::panic_fmt(format_args!(
                        "index out of bounds: the len is {0} but the index is {1}",
                        self.len, index,
                    ));
                };
            }
            unsafe { &*self.ptr.add(index) as &T }
        }
    }
    impl<T: FFISafe + Sized> IndexMut<usize> for Vector<T> {
        fn index_mut(&mut self, index: usize) -> &mut Self::Output {
            if index >= self.len {
                {
                    ::core::panicking::panic_fmt(format_args!(
                        "index out of bounds: the len is {0} but the index is {1}",
                        self.len, index,
                    ));
                };
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
}
pub unsafe trait FFISafe {}
unsafe impl FFISafe for u8 {}
unsafe impl FFISafe for u16 {}
unsafe impl FFISafe for u32 {}
unsafe impl FFISafe for u64 {}
unsafe impl FFISafe for i8 {}
unsafe impl FFISafe for i16 {}
unsafe impl FFISafe for i32 {}
unsafe impl FFISafe for i64 {}
unsafe impl FFISafe for usize {}
unsafe impl FFISafe for isize {}
unsafe impl FFISafe for c_void {}
unsafe impl<T> FFISafe for *const T {}
unsafe impl<T> FFISafe for *mut T {}
