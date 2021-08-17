use std::io::prelude::*;

fn main() {
    let mut proc = conpty::spawn("ping localhost").unwrap();

    let mut buf = [0; 300];

    let n = proc.read(&mut buf).unwrap();
    println!("{:?}", &buf[..n]);
    println!("{:?}", String::from_utf8_lossy(&buf[..n]));

    let n = proc.read(&mut buf).unwrap();
    println!("{:?}", &buf[..n]);
    println!("{:?}", String::from_utf8_lossy(&buf[..n]));
}
