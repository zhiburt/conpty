use std::{
    io::{self, BufRead, BufReader, LineWriter, Read, Write},
    thread,
    time::Duration,
};

use conpty::spawn;
use strip_ansi_escapes::strip;

#[test]
fn write_and_read() {
    let mut proc = spawn(r"python .\tests\util\cat.py").unwrap();
    let mut writer = LineWriter::new(proc.input().unwrap());
    let mut reader = BufReader::new(proc.output().unwrap());

    writer.write_all(b"hello cat\r\n").unwrap();

    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    assert_eq!(strip(line.as_bytes()).unwrap(), b"hello cat\n");

    drop(writer);
    drop(reader);

    proc.exit(1).unwrap();

    thread::sleep(Duration::from_millis(300));

    assert!(proc.is_alive() == false);
}

#[test]
fn write_ctrl_c() {
    let mut proc = spawn(r"python .\tests\util\cat.py").unwrap();
    let mut writer = proc.input().unwrap();

    thread::sleep(Duration::from_millis(600));

    // send ^C
    writer.write_all(&[3]).unwrap();
    drop(writer);

    thread::sleep(Duration::from_millis(600));

    assert!(proc.is_alive() == false);
}

#[test]
fn write_ctrl_z() {
    let mut proc = spawn(r"python .\tests\util\cat.py").unwrap();
    let mut writer = proc.input().unwrap();

    // send ^Z
    writer.write_all(&[0x1A]).unwrap();
    writer.write_all(b"\r\n").unwrap();
    drop(writer);

    thread::sleep(Duration::from_millis(600));

    assert!(proc.is_alive() == false);
}

#[test]
fn read_until() {
    let mut proc = spawn(r"python .\tests\util\cat.py").unwrap();
    let mut writer = proc.input().unwrap();
    let mut reader = BufReader::new(proc.output().unwrap());

    writeln!(writer, "Hello World").unwrap();

    let mut buf = Vec::new();
    reader.read_until(b' ', &mut buf).unwrap();

    assert_eq!(strip(&buf).unwrap(), b"Hello ");

    let mut buf = vec![0; 128];
    let n = reader.read(&mut buf).unwrap();
    assert_eq!(strip(&buf[..n]).unwrap(), b"World");
}

#[test]
fn read_blocks_after_process_exit() {
    let mut proc = spawn(r"python .\tests\util\cat.py").unwrap();
    let mut writer = proc.input().unwrap();
    let mut reader = proc.output().unwrap();

    thread::sleep(Duration::from_millis(300));

    writeln!(writer, "Hello World").unwrap();

    let mut buf = [0; 128];
    reader.read(&mut buf).unwrap();
    assert_eq!(strip(buf).unwrap(), b"Hello World");

    proc.exit(1).unwrap();

    thread::sleep(Duration::from_millis(600));

    assert!(proc.is_alive() == false);

    let mut reader = proc.output().unwrap();
    try_pipe_read(
        move || reader.read(&mut [0; 128]),
        Duration::from_millis(600),
    );
}

#[test]
fn read_blocks_after_process_exit_with_no_output() {
    let mut proc = spawn(r"python .\tests\util\cat.py").unwrap();
    let mut writer = proc.input().unwrap();
    let mut reader = proc.output().unwrap();

    thread::sleep(Duration::from_millis(300));

    writeln!(writer, "Hello World").unwrap();

    let mut buf = [0; 128];
    reader.read(&mut buf).unwrap();
    assert_eq!(strip(buf).unwrap(), b"Hello World");

    proc.exit(1).unwrap();

    thread::sleep(Duration::from_millis(600));

    assert!(proc.is_alive() == false);

    try_pipe_read(
        move || reader.read(&mut [0; 128]),
        Duration::from_millis(600),
    );
}

#[test]
fn read_to_end_blocks_after_process_exit() {
    let mut proc = spawn(r"echo 'Hello World'").unwrap();

    thread::sleep(Duration::from_millis(600));

    assert!(proc.is_alive() == false);

    let mut reader = proc.output().unwrap();
    try_pipe_read(
        move || reader.read_to_end(&mut Vec::new()),
        Duration::from_millis(600),
    );
}

fn try_pipe_read<R: FnOnce() -> io::Result<usize> + Send + 'static>(reader: R, timeout: Duration) {
    let handle = thread::spawn(move || {
        // Because reader will be dropped when the the reading is still active
        // we might get a error that pipe has been closed.
        match reader() {
            Err(err) if err.to_string() == "The pipe has been ended. (os error -2147024787)" => {}
            result => {
                // the error will be propagated in case of panic
                panic!(
                    "it's unnexpected that read operation will be ended {:?}",
                    result.unwrap_err().to_string()
                )
            }
        }
    });

    // give some time to read
    thread::sleep(timeout);

    drop(handle);
}
