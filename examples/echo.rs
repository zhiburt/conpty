use std::io::prelude::*;

fn main() {
    let mut proc = conpty::spawn("echo Hello World").unwrap();
    let mut reader = proc.output().unwrap();

    println!("Process has pid={}", proc.pid());

    let mut buf = [0; 1028];
    let n = reader.read(&mut buf).unwrap();
    assert!(String::from_utf8_lossy(&buf[..n]).contains("Hello World"));
}
