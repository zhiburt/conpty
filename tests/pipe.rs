use std::io::{Read, Write};

use conpty::spawn;

#[test]
pub fn close_one_pty_input_doesnt_close_others() {
    let mut proc = spawn("cmd").unwrap();
    let writer1 = proc.input().unwrap();
    let mut writer2 = proc.input().unwrap();

    assert!(writer2.write(b"").is_ok());

    drop(writer1);

    assert!(writer2.write(b"").is_ok());
}

#[test]
pub fn non_blocking_read() {
    let mut proc = spawn("cmd").unwrap();
    let mut reader = proc.output().unwrap();
    reader.blocking(false);

    let mut buf = [0; 1028];
    loop {
        match reader.read(&mut buf) {
            Ok(_) => break,
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(err) => Err(err).unwrap(),
        }
    }
}

#[test]
pub fn non_blocking_mode_does_not_affect_all_readers() {
    let mut proc = spawn("cmd").unwrap();
    let mut reader1 = proc.output().unwrap();
    let mut reader2 = proc.output().unwrap();
    reader2.blocking(false);

    assert!(reader1.read(&mut [0; 128]).is_ok());
}

#[test]
pub fn dropping_one_reader_doesnt_affect_others() {
    let mut proc = spawn("cmd").unwrap();
    let mut reader1 = proc.output().unwrap();
    let reader2 = proc.output().unwrap();

    drop(reader2);

    reader1.blocking(false);
    assert_eq!(
        reader1.read(&mut [0; 128]).unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}
