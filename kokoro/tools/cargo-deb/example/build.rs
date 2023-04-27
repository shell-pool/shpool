use std::env;
use std::path::PathBuf;
use std::fs::{self, File};
use std::io::Write;

fn main() {
  let out_str = env::var("OUT_DIR").unwrap();
  let out_path = PathBuf::from(&out_str);
  let mut out_path = out_path
    .ancestors()  // .../target/<debug|release>/build/example-<SHA>/out
    .skip(3)      // .../target/<debug|release>
    .next().unwrap().to_owned();
  out_path.push("assets");

  if !out_path.exists() { fs::create_dir(&out_path).expect("Could not create assets dir"); }
  File::create(out_path.join("5.txt")).and_then(|mut f| f.write_all(b"Hello generated asset 1")).expect("Could not write asset file");
  File::create(out_path.join("6.txt")).and_then(|mut f| f.write_all(b"Hello generated asset 2")).expect("Could not write asset file");

  if cfg!(feature = "example_non_debian_build") {
    panic!("Detected this example isn't built via cargo-deb, because example_non_debian_build feature is on. Build with --no-default-features");
  }
  if !cfg!(feature = "example_debian_build") {
    panic!("Detected this example isn't built via cargo-deb, because example_debian_build feature is off. Build with --features=example_debian_build");
  }
}
