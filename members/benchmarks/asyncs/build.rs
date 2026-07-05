use std::{env, path::PathBuf};

fn main() {
  let mut project_root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());

  project_root.push("../../..");

  let rt = project_root.clone();
  let common_targets = rt.to_str().expect("Unable to build deps");

  // Point to the workspace target directory where the dylib files are generated
  println!(
    "cargo:rustc-link-search=native={}/libsalloc/target/release",
    common_targets
  );
  println!(
    "cargo:rustc-link-search=native={}/libsavmasync/target/release",
    common_targets
  );
}
