use std::env;

// env::ARCH doesn't include full triple, and AFAIK there isn't a nicer way of getting the full triple
// (see lib.rs for the rest of this hack)
fn main() {
    println!("cargo:rustc-env=CARGO_DEB_DEFAULT_TARGET={}", env::var("TARGET").unwrap());
    println!("cargo:rerun-if-env-changed=TARGET");
    println!("cargo:rerun-if-changed=build.rs"); // optimization: avoid re-running this script
}
