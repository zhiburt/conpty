use std::io::prelude::*;

fn main() {
    let proc = conpty::spawn("ping").unwrap();
    let mut reader = proc.output().unwrap();

    println!("Process has pid={}", proc.pid());

    while proc.is_alive() {}

    // currently I didn't figure out  way how to savely && easily read from pipe until EOF
    let mut buf = [0; 1028 * 10 * 10];
    let n = reader.read(&mut buf).unwrap();
    println!("{:?}", &buf[..n]);
    println!("{:?}", String::from_utf8_lossy(&buf[..n]));
}
