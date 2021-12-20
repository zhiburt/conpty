use std::io::{Read, Write};

use conpty::spawn;

#[test]
pub fn close_one_pty_input_doesnt_close_others() {
    let proc = spawn("cmd").unwrap();
    let writer1 = proc.input().unwrap();
    let mut writer2 = proc.input().unwrap();

    assert!(writer2.write(b"").is_ok());

    drop(writer1);

    assert!(writer2.write(b"").is_ok());
}

#[test]
pub fn non_blocking_read() {
    let proc = spawn("cmd").unwrap();
    let mut reader = proc.output().unwrap();
    reader.set_non_blocking_mode().unwrap();

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
pub fn non_blocking_mode_affects_all_readers() {
    let proc = spawn("cmd").unwrap();
    let mut reader1 = proc.output().unwrap();
    let mut reader2 = proc.output().unwrap();
    reader2.set_non_blocking_mode().unwrap();

    assert_eq!(
        reader1.read(&mut [0; 128]).unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}

#[test]
pub fn dropping_one_reader_doesnt_affect_others() {
    let proc = spawn("cmd").unwrap();
    let mut reader1 = proc.output().unwrap();
    let reader2 = proc.output().unwrap();

    drop(reader2);

    reader1.set_non_blocking_mode().unwrap();
    assert_eq!(
        reader1.read(&mut [0; 128]).unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}
