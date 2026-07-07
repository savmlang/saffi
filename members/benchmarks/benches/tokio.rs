use benchmarks::{Instruction, RT_MUL};
use futures::{StreamExt, stream::FuturesUnordered};

fn main() {
  // Run registered benchmarks.
  divan::main();
}

#[divan::bench(args = [Instruction::None, Instruction::Sleep100ms], sample_size = 1)]
fn single(id: Instruction) {
  match id {
    Instruction::None => RT_MUL.block_on(benchmarks::none()),
    Instruction::Sleep100ms => RT_MUL.block_on(benchmarks::sleep100ms()),
  };
}

#[divan::bench(args = [Instruction::None, Instruction::Sleep100ms], sample_size = 1)]
fn multi(id: Instruction) {
  RT_MUL.block_on(async {
    match id {
      Instruction::None => {
        let mut tasks = FuturesUnordered::new();
        for _ in 0..5_000 {
          tasks.push(benchmarks::none());
        }

        while let Some(_) = tasks.next().await {}
      }
      Instruction::Sleep100ms => {
        let mut tasks = FuturesUnordered::new();
        for _ in 0..5_000 {
          tasks.push(benchmarks::sleep100ms());
        }

        while let Some(_) = tasks.next().await {}
      }
    };
  });
}
