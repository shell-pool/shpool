extern crate shpool_pty;
extern crate libc;
extern crate errno;

use shpool_pty::fork::*;
use std::io::Read;
use std::process::{Command};

fn main() {
    let fork = Fork::from_ptmx().unwrap();

    if let Some(mut master) = fork.is_parent().ok() {
        // Read output via PTY master
        let mut output = String::new();

        match master.read_to_string(&mut output) {
            Ok(_nread) => println!("child tty is: {}", output.trim()),
            Err(e) => panic!("read error: {}", e),
        }
    } else {
        // Child process just exec `tty`
        Command::new("tty").status().expect("could not execute tty");
    }
}
