use benchmarks::{Instruction, RT_MUL};
use futures::{StreamExt, stream::FuturesUnordered};
use saffi::savmasync;

fn main() {
  savmasync::init();

  // Run registered benchmarks.
  divan::main();
}

#[divan::bench(args = [Instruction::None, Instruction::Sleep100ms])]
fn single(id: Instruction) {
  match id {
    Instruction::None => RT_MUL.block_on(benchmarks::none()),
    Instruction::Sleep100ms => RT_MUL.block_on(benchmarks::sleep100ms()),
  };
}

#[divan::bench(args = [Instruction::None, Instruction::Sleep100ms])]
fn multi(id: Instruction) {
  RT_MUL.block_on(async {
    match id {
      Instruction::None => {
        let mut tasks = FuturesUnordered::new();
        for _ in 0..20_000 {
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
