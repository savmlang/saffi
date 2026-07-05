use benchmarks::{Instruction, RT_MUL};
use futures::{StreamExt, stream::FuturesUnordered};
use saffi::{futures::FFIFuture, savmasync};

fn main() {
  savmasync::init();

  // Run registered benchmarks.
  divan::main();
}

#[divan::bench(args = [Instruction::None, Instruction::Sleep100ms], sample_size = 100)]
fn single(id: Instruction) {
  match id {
    Instruction::None => RT_MUL.block_on(FFIFuture::new(benchmarks::asyncfn::none())),
    Instruction::Sleep100ms => RT_MUL.block_on(FFIFuture::new(benchmarks::asyncfn::sleep100ms())),
  };
}

#[divan::bench(args = [Instruction::None, Instruction::Sleep100ms], sample_size = 100)]
fn flood(id: Instruction) {
  RT_MUL.block_on(async {
    let mut tasks = FuturesUnordered::new();

    match id {
      Instruction::None => {
        for _ in 0..5_000 {
          tasks.push(FFIFuture::new(benchmarks::asyncfn::none()));
        }
      }
      Instruction::Sleep100ms => {
        for _ in 0..5_000 {
          {
            tasks.push(FFIFuture::new(benchmarks::asyncfn::sleep100ms()));
          }
        }
      }
    };

    while let Some(_) = tasks.next().await {}
  });
}
