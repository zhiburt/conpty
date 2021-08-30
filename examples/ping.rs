use std::io::prelude::*;

fn main() {
    let proc = conpty::spawn("ping").unwrap();
    let mut reader = proc.output().unwrap();
    let mut writer = proc.input().unwrap();

    println!("Process has pid={}", proc.pid());

    writer.write(b"ping").unwrap();

    let mut buf = [0; 300];
    while proc.is_alive() {
        let n = reader.read(&mut buf).unwrap();
        println!("{}", String::from_utf8_lossy(&buf[..n]));
    }
}
