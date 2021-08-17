use std::io::prelude::*;

fn main() {
    let mut proc = conpty::spawn("ping localhost").unwrap();

    println!("Process has pid={}", proc.pid());

    let mut buf = [0; 300];

    proc.wait().unwrap();

    let n = proc.read(&mut buf).unwrap();
    println!("{}", String::from_utf8_lossy(&buf[..n]));
}
