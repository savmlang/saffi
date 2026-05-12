use std::{env, path::PathBuf, process::Command};

fn main() {
  let mut project_root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());

  project_root.push("../../..");

  let mut rt = project_root.clone();
  rt.push("libsalloc");
  let common_targets = rt.to_str().expect("Unable to build deps");

  // Point to the workspace target directory where the dylib files are generated
  println!(
    "cargo:rustc-link-search=native={}/target/release",
    common_targets
  );
}
