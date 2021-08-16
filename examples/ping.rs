use std::io::prelude::*;

fn main() {
    let mut proc = conpty::spawn("ping").unwrap();
    
    let mut buf = [0; 300];
    loop {
        let n = proc.read(&mut buf).unwrap();
        // println!("111111111111 {}", n);
        if n == 0 {
            break;
        }
        println!("{}", String::from_utf8(buf[..n].to_vec()).unwrap());
    }
}