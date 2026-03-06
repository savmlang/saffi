use benchmarks::{Instruction, RT_SIN};
use futures::{StreamExt, stream::FuturesUnordered};

fn main() {
  // Run registered benchmarks.
  divan::main();
}

// Register a `fibonacci` function and benchmark it over multiple cases.
#[divan::bench(args = [Instruction::None, Instruction::Sleep100ms])]
fn tokio(id: Instruction) {
  match id {
    Instruction::None => RT_SIN.block_on(benchmarks::none()),
    Instruction::Sleep100ms => RT_SIN.block_on(benchmarks::sleep100ms()),
  };
}

#[divan::bench]
fn throughput_flood_none() {
  RT_SIN.block_on(async {
    // We test 20k completions per sample to see the raw dispatch speed
    let mut tasks = FuturesUnordered::new();
    for _ in 0..20_000 {
      tasks.push(benchmarks::none());
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
      tasks.push(benchmarks::sleep100ms());
    }
    while let Some(_) = tasks.next().await {}
  });
}
