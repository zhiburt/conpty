use std::process::Command;

use conpty::Process;

// not sure if's desired behaiviour
// todo: timeout for wait/exit
#[test]
pub fn env_parameter() {
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
