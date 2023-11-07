extern crate shpool_pty;
extern crate libc;

use self::shpool_pty::prelude::*;

use std::io::prelude::*;
use std::string::String;
use std::process::Command;

fn read_line(master:&mut Master) -> String {
    let mut buf = [0];
    let mut res = String::new();
    while buf[0] as char != '\n' {
        master.read(&mut buf).expect("cannot read 1 byte");
        res.push(buf[0] as char)
    }
    res
}

#[test]
fn it_can_read_write() {
    let fork = Fork::from_ptmx().unwrap();

    if let Some(mut master) = fork.is_parent().ok() {
        let _ = master.write("echo readme!\n".to_string().as_bytes());

        read_line(&mut master); // this is the "echo readme!" we just sent
        read_line(&mut master); // this is the shell and "echo readme!" again
        assert_eq!(read_line(&mut master).trim(), "readme!");
        let _ = master.write("exit\n".to_string().as_bytes());
    } else {
        let _ = Command::new("bash").env_clear().status();
    }
}
