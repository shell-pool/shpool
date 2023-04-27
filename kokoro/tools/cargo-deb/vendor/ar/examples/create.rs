//! Creates an archive from one or more input files.
//!
//! To create a new archive, run:
//!
//! ```shell
//! cargo run --example create <path/to/output.a> <path/to/input1> <input2..>
//! ```
//!
//! Assuming the output file doesn't already exist, this is roughly equivalent
//! to running:
//!
//! ```shell
//! ar -cr <path/to/output.a> <path/to/input1> <input2..>
//! ```

extern crate ar;

use std::env;
use std::fs::File;
use std::path::Path;

fn main() {
    let num_args = env::args().count();
    if num_args < 3 {
        println!("Usage: create <outpath> <inpath> [<inpath>...]");
        return;
    }

    let output_path = env::args().nth(1).unwrap();
    let output_path = Path::new(&output_path);
    let output_file =
        File::create(output_path).expect("failed to open output file");
    let mut builder = ar::Builder::new(output_file);

    for index in 2..num_args {
        let input_path = env::args().nth(index).unwrap();
        let input_path = Path::new(&input_path);
        builder
            .append_path(input_path)
            .expect(&format!("failed to add {:?} to archive", input_path));
    }
}
