use std::sync::mpsc::Sender;

use crate::{
  FFISafe,
  boxed::{RTSafeBoxWrapper, drop_rtbox},
};

#[repr(C)]
pub struct ThreadSpawnContext {
  // implicitly has some data
  pub sender: *mut RTSafeBoxWrapper,
  pub send: extern "C" fn(pt: *mut RTSafeBoxWrapper, data: *mut RTSafeBoxWrapper),
}

unsafe impl FFISafe for ThreadSpawnContext {}

#[repr(C)]
pub struct SendWrapper(*mut RTSafeBoxWrapper);

unsafe impl Send for SendWrapper {}
unsafe impl Sync for SendWrapper {}
unsafe impl FFISafe for SendWrapper {}

unsafe impl<T: FFISafe> FFISafe for Sender<T> {}

pub extern "C" fn send(pt: *mut RTSafeBoxWrapper, data: *mut RTSafeBoxWrapper) {
  unsafe {
    let pt = RTSafeBoxWrapper::construct::<Sender<SendWrapper>>(pt);

    _ = pt.send(SendWrapper(data));

    pt.into_raw();
  }
}

impl Drop for ThreadSpawnContext {
  fn drop(&mut self) {
    unsafe { drop_rtbox(self.sender) };
  }
}
