use std::{ffi::c_void, sync::mpsc::Sender};

use crate::{FFISafe, boxed::drop_rtbox};

#[repr(C)]
pub struct SenderStructure;

#[repr(C)]
pub struct ThreadSpawnContext {
  // implicitly has some data
  pub sender: *mut SenderStructure,
  pub send: extern "C" fn(sender: *mut SenderStructure, data: *mut c_void),
}

unsafe impl FFISafe for ThreadSpawnContext {}

unsafe impl FFISafe for SenderStructure {}

pub extern "C" fn send(sender: *mut SenderStructure, data: *mut c_void) {
  unsafe {
    debug_assert!(!sender.is_null());

    let sender = &*(sender as *mut Sender<*mut c_void>);

    _ = sender.send(data);
  }
}

impl Drop for ThreadSpawnContext {
  fn drop(&mut self) {
    unsafe { drop_rtbox(self.sender) };
  }
}
