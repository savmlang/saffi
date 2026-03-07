use std::{path::PathBuf, process::Command};

fn main() {
  let mut project_root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());

  let mut asyncs = project_root.clone();
  asyncs.push("asyncs");

  project_root.push("../..");

  let mut rt = project_root.clone();
  rt.push("cache");
  let common_targets = rt.to_str().expect("Unable to build deps");

  // Build SaAlloc at project root
  if !Command::new("cargo")
    .arg("build")
    .arg("--release")
    .arg("-p")
    .arg("salloc")
    .current_dir(&project_root)
    .env("CARGO_TARGET_DIR", common_targets)
    .spawn()
    .expect("Unable to launch cargo build for SaAlloc")
    .wait()
    .expect("Unable to compile SaAlloc")
    .success()
  {
    panic!("Building SaAlloc failed")
  }

  // Build asyncs
  if !Command::new("cargo")
    .arg("build")
    .arg("--release")
    .current_dir(&asyncs)
    .env("CARGO_TARGET_DIR", common_targets)
    .spawn()
    .expect("Unable to launch cargo build for Asyncs")
    .wait()
    .expect("Unable to compile Asyncs")
    .success()
  {
    panic!("Building Asyncs failed");
  }

  // Point to the workspace target directory where the .so files are generated
  println!(
    "cargo:rustc-link-search=native={}/target/release",
    common_targets
  );
  println!(
    "cargo:rustc-link-search=native={}/target/debug",
    common_targets
  );

  // println!("cargo:rustc-link-lib=cdylib=asyncfn");
  // println!("cargo:rustc-link-lib=cdylib=salloc");
}
