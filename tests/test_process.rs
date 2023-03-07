use std::{
    io::{BufRead, BufReader},
    path::Path,
    process::Command,
};

use conpty::Process;

#[test]
pub fn envs() {
    // note: maybe making cmd /C being a default as it was is good?
    let batch = r#"cmd /C if "%TEST_ENV%"=="123456" (exit 0) else (exit 1)"#;

    // set correct value
    let mut cmd = Command::new(batch);
    cmd.env("TEST_ENV".to_string(), "123456".to_string());
    let proc = Process::spawn(cmd).unwrap();
    assert_eq!(proc.wait(None).unwrap(), 0);

    // set wrong value
    let mut cmd = Command::new(batch);
    cmd.env("TEST_ENV".to_string(), "NOT_CORRENT_VALUE".to_string());
    let proc = Process::spawn(cmd).unwrap();
    assert_eq!(proc.wait(None).unwrap(), 1);

    // not set at all
    let cmd = Command::new(batch);
    let proc = Process::spawn(cmd).unwrap();
    assert_eq!(proc.wait(None).unwrap(), 1);
}

#[test]
pub fn test_args_0() {
    // set correct value
    let mut cmd = Command::new("cmd /C echo");
    cmd.arg("Hello");
    cmd.arg("World");

    let mut proc = Process::spawn(cmd).unwrap();
    let output = proc.output().unwrap();

    let mut reader = BufReader::new(output);
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();

    assert!(line.contains("Hello World\r\n"), "{:?}", line);
}

#[test]
pub fn test_args_1() {
    // set correct value
    let mut cmd = Command::new("cmd /C echo");
    cmd.args(["Hello", "World", "!!!"]);

    let mut proc = Process::spawn(cmd).unwrap();
    let output = proc.output().unwrap();

    let mut reader = BufReader::new(output);
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();

    assert!(line.contains("Hello World !!!\r\n"), "{:?}", line);
}

#[test]
pub fn test_current_dir() {
    // set correct value
    let mut cmd = Command::new("cmd /C dir");
    cmd.current_dir("./tests");

    let mut proc = Process::spawn(cmd).unwrap();
    let output = proc.output().unwrap();
    let mut reader = BufReader::new(output);

    let this_file_name = Path::new(file!()).file_name().unwrap().to_str().unwrap();
    loop {
        let mut line = String::new();
        let i = reader.read_line(&mut line).unwrap();
        if line.contains(this_file_name) {
            return;
        }

        if i == 0 {
            assert!(false)
        }
    }
}
