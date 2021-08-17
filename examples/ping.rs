use std::io::prelude::*;

fn main() {
    let mut proc = conpty::spawn("ping").unwrap();

    let mut buf = [0; 300];
    let n = proc.read(&mut buf).unwrap();
    println!("{}", String::from_utf8(buf[..n].to_vec()).unwrap());
}
