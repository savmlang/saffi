use std::{env, path::PathBuf, process::Command};

fn main() {
  let mut project_root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());

  project_root.push("../../..");

  let mut rt = project_root.clone();
  rt.push("cache");
  rt.push("asyncs");
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

  // Point to the workspace target directory where the dylib files are generated
  println!(
    "cargo:rustc-link-search=native={}/target/release",
    common_targets
  );
  println!(
    "cargo:rustc-link-search=native={}/target/debug",
    common_targets
  );
}
