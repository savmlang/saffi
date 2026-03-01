use std::time::Duration;

use tokio::time::sleep;

use crate::futures::{FFIFuture, implements::create_future};

#[tokio::test]
async fn test_rt() {
  let fut = create_future(hello());

  let out = FFIFuture::new(fut).await;

  assert!(out == 64);
}

async fn hello() -> u64 {
  sleep(Duration::from_secs(5)).await;

  64
}
