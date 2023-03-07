use std::io::{Read, Result};

fn main() -> Result<()> {
    let mut proc = conpty::spawn("echo Hello World")?;
    let mut reader = proc.output()?;

    println!("Process has pid={}", proc.pid());

    let mut buf = [0; 1028];
    reader.read(&mut buf)?;

    assert!(String::from_utf8_lossy(&buf).contains("Hello World"));

    Ok(())
}
