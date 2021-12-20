use conpty::ProcAttr;

// not sure if's desired behaiviour
// todo: timeout for wait/exit
#[test]
pub fn env_parameter() {
    let batch = r#"if "%TEST_ENV%"=="123456" (exit 0) else (exit 1)"#;
    let proc = ProcAttr::cmd(batch.to_string())
        .env("TEST_ENV".to_string(), "123456".to_string())
        .spawn()
        .unwrap();
    assert_eq!(proc.wait(None).unwrap(), 0);

    let proc = ProcAttr::cmd(batch.to_string())
        .env("TEST_ENV".to_string(), "NOT_CORRENT_VALUE".to_string())
        .spawn()
        .unwrap();
    assert_eq!(proc.wait(None).unwrap(), 1);

    // not set
    let proc = ProcAttr::cmd(batch.to_string()).spawn().unwrap();
    assert_eq!(proc.wait(None).unwrap(), 1);
}
