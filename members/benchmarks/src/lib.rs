use std::{sync::LazyLock, time::Duration};

use tokio::runtime::{Builder, Runtime};

pub mod asyncfn {
  use saffi::futures::FutureTask;

  #[cfg_attr(windows, link(name = "asyncfn", kind = "raw-dylib"))]
  #[cfg_attr(not(windows), link(name = "asyncfn", kind = "dylib"))]
  unsafe extern "C" {
    pub safe fn none() -> FutureTask<u8>;
    pub safe fn sleep100ms() -> FutureTask<u8>;
  }
}

pub async fn none() -> u8 {
  0
}

pub async fn sleep100ms() -> u8 {
  tokio::time::sleep(Duration::from_millis(100)).await;

  0
}

#[derive(Debug, Clone, Copy)]
pub enum Instruction {
  None,
  Sleep100ms,
}

pub static RT_MUL: LazyLock<Runtime> =
  LazyLock::new(|| Builder::new_multi_thread().enable_all().build().unwrap());

pub static RT_SIN: LazyLock<Runtime> =
  LazyLock::new(|| Builder::new_current_thread().enable_all().build().unwrap());
