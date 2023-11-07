extern crate shpool_pty;
extern crate libc;

use self::shpool_pty::prelude::*;

use std::io::prelude::*;
use std::process::{Command, Stdio};

fn main() {
    let fork = Fork::from_ptmx().unwrap();

    if let Some(mut master) = fork.is_parent().ok() {
        let mut string = String::new();

        master.read_to_string(&mut string).unwrap_or_else(|e| panic!("{}", e));

        let output = Command::new("tty")
            .stdin(Stdio::inherit())
            .output()
            .unwrap()
            .stdout;
        let output_str = String::from_utf8_lossy(&output);

        let parent_tty = output_str.trim();
        let child_tty = string.trim();

        println!("child_tty(\"{}\")[{}] != \"{}\" => {}", child_tty, child_tty.len(), "", child_tty != "");
        assert!(child_tty != "");
        assert!(child_tty != parent_tty);

        let mut parent_tty_dir: Vec<&str> = parent_tty.split("/").collect();
        let mut child_tty_dir: Vec<&str> = child_tty.split("/").collect();

        parent_tty_dir.pop();
        child_tty_dir.pop();

        assert_eq!(parent_tty_dir, child_tty_dir);
    } else {
        let _ = Command::new("tty").status();
    }
}
