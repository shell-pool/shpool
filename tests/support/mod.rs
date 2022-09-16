use std::path::{PathBuf, Path};
use std::env;

pub fn testdata_file<P: AsRef<Path>>(file: P) -> PathBuf {
    let mut dir = cargo_dir();
    dir.pop();
    dir.pop();
    dir.join("tests").join("data").join(file)
}

pub fn shpool_bin() -> PathBuf {
    cargo_dir().join("shpool")
}

pub fn cargo_dir() -> PathBuf {
    env::var_os("CARGO_BIN_PATH").map(PathBuf::from).or_else(|| {
        env::current_exe().ok().map(|mut path| {
            path.pop();
            if path.ends_with("deps") {
                path.pop();
            }
            path
        })
    }).unwrap_or_else(|| {
        panic!("CARGO_BIN_PATH wasn't set. Cannot continue running test")
    })
}
