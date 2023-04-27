//! Lists symbols in an archive.
//!
//! To list all symbols in an archive, run:
//!
//! ```shell
//! cargo run --example symbols <path/to/archive.a>
//! ```

extern crate ar;

use std::env;
use std::fs::File;
use std::path::Path;

fn main() {
    let num_args = env::args().count();
    if num_args != 2 {
        println!("Usage: symbols <path/to/archive.a>");
        return;
    }

    let input_path = env::args().nth(1).unwrap();
    let input_path = Path::new(&input_path);
    let input_file =
        File::open(input_path).expect("failed to open input file");
    let mut archive = ar::Archive::new(input_file);

    for symbol in archive.symbols().expect("failed to parse symbols") {
        println!("{}", String::from_utf8_lossy(symbol));
    }
}
