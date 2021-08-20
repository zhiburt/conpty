use std::io::prelude::*;

fn main() {
    let mut proc = conpty::spawn("ping").unwrap();

    println!("Process has pid={}", proc.pid());

    proc.write(b"ping").unwrap();

    let mut buf = [0; 300];
    while proc.is_alive() {
        let n = proc.read(&mut buf).unwrap();
        println!("{}", String::from_utf8_lossy(&buf[..n]));
    }
}
