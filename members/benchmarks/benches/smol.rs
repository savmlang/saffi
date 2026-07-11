use benchmarks::Instruction;
use futures::{StreamExt, stream::FuturesUnordered};
use smol::block_on;

fn main() {
  // Run registered benchmarks.
  divan::main();
}

#[divan::bench(args = [Instruction::None, Instruction::Sleep100ms], sample_size = 1)]
fn single(id: Instruction) {
  match id {
    Instruction::None => block_on(benchmarks::none()),
    Instruction::Sleep100ms => block_on(benchmarks::sleep100ms_smol()),
  };
}

#[divan::bench(args = [Instruction::None, Instruction::Sleep100ms], sample_size = 1)]
fn flood(id: Instruction) {
  block_on(async {
    match id {
      Instruction::None => {
        let mut tasks = FuturesUnordered::new();
        for _ in 0..50_000 {
          tasks.push(benchmarks::none());
        }

        while let Some(_) = tasks.next().await {}
      }
      Instruction::Sleep100ms => {
        let mut tasks = FuturesUnordered::new();
        for _ in 0..50_000 {
          tasks.push(benchmarks::sleep100ms_smol());
        }

        while let Some(_) = tasks.next().await {}
      }
    };
  });
}
