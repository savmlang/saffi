fn main() {
  // Disable statistics and secure mode to shrink binary size
  // These are C-level flags for the mimalloc build
  println!("cargo:rustc-env=CFLAGS=-DMI_STAT=0 -DMI_SECURE=0 -DMI_DEBUG=0");
}
