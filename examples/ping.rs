use std::io::prelude::*;

fn main() {
    let proc = conpty::spawn("ping").unwrap();
    let mut reader = proc.output().unwrap();

    println!("Process has pid={}", proc.pid());

    proc.wait(None).unwrap();

    let mut buf = [0; 1028 * 10 * 10];
    let n = reader.read(&mut buf).unwrap();
    println!("{}", String::from_utf8_lossy(&buf[..n]));
}
