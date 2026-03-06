use benchmarks::{Instruction, RT_SIN};
use futures::{StreamExt, stream::FuturesUnordered};
use saffi::futures::FFIFuture;

fn main() {
  // Run registered benchmarks.
  divan::main();
}

// Register a `fibonacci` function and benchmark it over multiple cases.
#[divan::bench(args = [Instruction::None, Instruction::Sleep100ms])]
fn tokio(id: Instruction) {
  match id {
    Instruction::None => RT_SIN.block_on(FFIFuture::new(benchmarks::asyncfn::none())),
    Instruction::Sleep100ms => RT_SIN.block_on(FFIFuture::new(benchmarks::asyncfn::sleep100ms())),
  };
}

#[divan::bench]
fn throughput_flood_none() {
  RT_SIN.block_on(async {
    // We test 20k completions per sample to see the raw dispatch speed
    let mut tasks = FuturesUnordered::new();
    for _ in 0..20_000 {
      tasks.push(FFIFuture::new(benchmarks::asyncfn::none()));
    }
    while let Some(_) = tasks.next().await {}
  });
}

#[divan::bench]
fn throughput_timer_storm() {
  RT_SIN.block_on(async {
    // We test 5k concurrent smol-timers crossing into Tokio
    let mut tasks = FuturesUnordered::new();
    for _ in 0..5_000 {
      tasks.push(FFIFuture::new(benchmarks::asyncfn::sleep100ms()));
    }
    while let Some(_) = tasks.next().await {}
  });
}
