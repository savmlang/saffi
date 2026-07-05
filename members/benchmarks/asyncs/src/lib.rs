use std::time::Duration;

use saffi::futures::{FutureTask, implements::create_future};
use smol::Timer;

use async_io::driverloop;
use saffi::savmasync::generate;

generate! {
  SMOL_RT => (0, driverloop)
}

#[unsafe(no_mangle)]
pub extern "C" fn none() -> FutureTask<u8> {
  create_future(async { 0 })
}

#[unsafe(no_mangle)]
pub extern "C" fn sleep100ms() -> FutureTask<u8> {
  create_future(async {
    Timer::after(Duration::from_millis(100)).await;

    0
  })
}
