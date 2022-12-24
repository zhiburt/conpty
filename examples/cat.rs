use std::{
    io::prelude::*,
    time::{Duration, Instant},
};

use conpty::io::PipeReader;

fn main() {
    let mut p = conpty::spawn(r#"cmd"#).unwrap();

    println!("Process PID={:?}", p.pid());

    let mut input = p.input().unwrap();
    let mut output = p.output().unwrap();
    output.blocking(false);

    println!("{:?}", wait_for(&mut output, "All rights reserved"));

    input
        .write_all("echo \"This is a test string 😁\"\r\n".as_bytes())
        .unwrap();

    println!("{:?}", wait_for(&mut output, "😁"));

    input.write_all(b"powershell\r\n").unwrap();

    println!("{:?}", wait_for(&mut output, "https://aka.ms/PSWindows"));

    input.write_all(b"cat examples/cat.rs\r\n").unwrap();

    println!("{:?}", wait_for(&mut output, "main"));
}

fn wait_for(output: &mut PipeReader, s: &str) -> String {
    let treashhold = Duration::from_secs(2);
    let now = Instant::now();

    let mut out = vec![0; 1000];
    let mut buf = String::new();
    loop {
        try_read(output, &mut out, &mut buf);
        if buf.contains(s) {
            return buf;
        }

        if now.elapsed() > treashhold {
            panic!("TIMEOUT REACHED")
        }
    }
}

fn try_read(o: &mut PipeReader, out: &mut Vec<u8>, buf: &mut String) {
    match o.read(out) {
        Ok(n) => {
            let s = String::from_utf8_lossy(&out[..n]);
            buf.push_str(&s);
        }
        Err(err) => {
            if err.kind() == std::io::ErrorKind::WouldBlock {
                return;
            }

            panic!("{:?}", err);
        }
    }
}
