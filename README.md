# conpty

A library which provides an interface for [ConPTY](https://devblogs.microsoft.com/commandline/windows-command-line-introducing-the-windows-pseudo-console-conpty/).

It is originally developed to be a windows backend for [zhiburt/expectrl](https://github.com/zhiburt/expectrl).

## Get started

```rust
use std::io::prelude::*;

fn main() {
    let proc = conpty::spawn("echo Hello World").unwrap();
    let mut reader = proc.output().unwrap();

    println!("Process has pid={}", proc.pid());

    proc.wait(None).unwrap();

    let mut buf = [0; 1028];
    let n = reader.read(&mut buf).unwrap();
    assert!(String::from_utf8_lossy(&buf[..n]).contains("Hello World"));
}
```
