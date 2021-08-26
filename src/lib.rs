#![allow(non_snake_case)]

mod bindings {
    windows::include_bindings!();
}

use bindings::{
    Windows::Win32::Foundation::CloseHandle,
    Windows::Win32::Foundation::{HANDLE, PWSTR},
    Windows::Win32::Storage::FileSystem::{
        CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_GENERIC_READ, FILE_GENERIC_WRITE, FILE_SHARE_READ,
        FILE_SHARE_WRITE, OPEN_EXISTING,
    },
    Windows::Win32::System::Console::{
        ClosePseudoConsole, CreatePseudoConsole, GetConsoleMode, GetConsoleScreenBufferInfo,
        ResizePseudoConsole, SetConsoleMode, CONSOLE_MODE, CONSOLE_SCREEN_BUFFER_INFO, COORD,
        ENABLE_VIRTUAL_TERMINAL_PROCESSING, HPCON,
    },
    Windows::Win32::System::Pipes::CreatePipe,
    Windows::Win32::System::Threading::{
        CreateProcessW, DeleteProcThreadAttributeList, GetExitCodeProcess, GetProcessId,
        InitializeProcThreadAttributeList, TerminateProcess, UpdateProcThreadAttribute,
        WaitForSingleObject, CREATE_UNICODE_ENVIRONMENT, EXTENDED_STARTUPINFO_PRESENT,
        LPPROC_THREAD_ATTRIBUTE_LIST, PROCESS_INFORMATION, STARTUPINFOEXW, WAIT_TIMEOUT,
    },
    Windows::Win32::System::WindowsProgramming::INFINITE,
};
use std::collections::HashMap;
use std::fs::File;
use std::io;
use std::os::windows::io::FromRawHandle;
use std::{mem::size_of, ptr::null_mut};
use windows::HRESULT;

pub fn spawn(cmd: impl Into<String>) -> windows::Result<Proc> {
    Proc::spawn(ProcAttr::cmd(cmd.into()))
}

pub struct Proc {
    pty_input: HANDLE,
    pty_output: HANDLE,
    _proc: PROCESS_INFORMATION,
    _proc_info: STARTUPINFOEXW,
    _console: HPCON,
}

impl Proc {
    fn spawn(attr: ProcAttr) -> windows::Result<Self> {
        enableVirtualTerminalSequenceProcessing()?;
        let (mut console, pty_reader, pty_writer) = createPseudoConsole()?;
        let startup_info = initializeStartupInfoAttachedToConPTY(&mut console)?;
        let proc = execProc(startup_info, attr)?;

        Ok(Self {
            pty_input: pty_writer,
            pty_output: pty_reader,
            _console: console,
            _proc: proc,
            _proc_info: startup_info,
        })
    }

    pub fn resize(&self, x: i16, y: i16) -> windows::Result<()> {
        unsafe { ResizePseudoConsole(self._console, COORD { X: x, Y: y }) }
    }

    pub fn pid(&self) -> u32 {
        unsafe { GetProcessId(self._proc.hProcess) }
    }

    pub fn exit(&self, code: u32) -> windows::Result<()> {
        unsafe { TerminateProcess(self._proc.hProcess, code).ok() }
    }

    pub fn wait(&self) -> windows::Result<u32> {
        unsafe {
            WaitForSingleObject(self._proc.hProcess, INFINITE);

            let mut code = 0;
            GetExitCodeProcess(self._proc.hProcess, &mut code).ok()?;

            Ok(code)
        }
    }

    pub fn is_alive(&self) -> bool {
        // https://stackoverflow.com/questions/1591342/c-how-to-determine-if-a-windows-process-is-running/5303889
        unsafe {
            let ret = WaitForSingleObject(self._proc.hProcess, 0);
            ret == WAIT_TIMEOUT
        }
    }

    pub fn pty_input(&self) -> io::Result<File> {
        let pty_input = unsafe { File::from_raw_handle(self.pty_input.0 as _) };
        Ok(pty_input)
    }

    pub fn pty_output(&self) -> io::Result<File> {
        let pty_output = unsafe { File::from_raw_handle(self.pty_output.0 as _) };
        Ok(pty_output)
    }
}

impl Drop for Proc {
    fn drop(&mut self) {
        unsafe {
            ClosePseudoConsole(self._console);

            CloseHandle(self._proc.hProcess);
            CloseHandle(self._proc.hThread);

            DeleteProcThreadAttributeList(self._proc_info.lpAttributeList);
            let _ = Box::from_raw(self._proc_info.lpAttributeList.0 as _);

            CloseHandle(self.pty_input);
            CloseHandle(self.pty_output);
        }
    }
}

// ProcAttr represents parameters for process to be spawned.
//
// Generally to run a common process you can set commandline to a path to binary.
// But if you're trying to spawn just a command in shell if must provide your shell first, like cmd.exe.
// One more time, cmd.exe is not needed if you're spawning an .exe file - it is necessary if you're trying
// to spawn a anything else like .bat file.
#[derive(Default, Debug)]
pub struct ProcAttr {
    application: Option<String>,
    commandline: Option<String>,
    current_dir: Option<String>,
    args: Vec<String>,
    env: Option<HashMap<String, String>>,
}

impl ProcAttr {
    pub fn batch(file: String) -> Self {
        // To run a batch file, you must start the command interpreter; set lpApplicationName to cmd.exe and
        // set lpCommandLine to the following arguments: /c plus the name of the batch file.
        //
        // https://docs.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-createprocessw
        let inter = std::env::var("COMSPEC").unwrap();
        let args = format!("/C {:?}", file);

        Self::default().application(inter).commandline(args)
    }

    pub fn cmd(commandline: String) -> Self {
        // To run a batch file, you must start the command interpreter; set lpApplicationName to cmd.exe and
        // set lpCommandLine to the following arguments: /c plus the name of the batch file.
        //
        // https://docs.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-createprocessw
        let inter = std::env::var("COMSPEC").unwrap();
        let args = format!("{} /C {}", inter, commandline);

        Self::default().commandline(args)
    }

    pub fn commandline(mut self, cmd: String) -> Self {
        self.commandline = Some(cmd);
        self
    }

    pub fn application(mut self, application: String) -> Self {
        self.application = Some(application);
        self
    }

    pub fn current_dir(mut self, dir: String) -> Self {
        self.current_dir = Some(dir);
        self
    }

    pub fn args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }

    pub fn arg(mut self, arg: String) -> Self {
        self.args.push(arg);
        self
    }

    pub fn envs(mut self, env: HashMap<String, String>) -> Self {
        self.env = Some(env);
        self
    }

    pub fn env(mut self, key: String, value: String) -> Self {
        match &mut self.env {
            Some(env) => {
                env.insert(key, value);
                self
            }
            None => self.envs(HashMap::new()).env(key, value),
        }
    }

    pub fn spawn(self) -> windows::Result<Proc> {
        Proc::spawn(self)
    }
}

fn enableVirtualTerminalSequenceProcessing() -> windows::Result<()> {
    let stdout_h = stdout_handle()?;
    unsafe {
        let mut mode = CONSOLE_MODE::default();
        GetConsoleMode(stdout_h, &mut mode).ok()?;
        mode |= ENABLE_VIRTUAL_TERMINAL_PROCESSING; // DISABLE_NEWLINE_AUTO_RETURN
        SetConsoleMode(stdout_h, mode).ok()?;

        CloseHandle(stdout_h);
    }

    Ok(())
}

fn createPseudoConsole() -> windows::Result<(HPCON, HANDLE, HANDLE)> {
    let (pty_in, con_writer) = pipe()?;
    let (con_reader, pty_out) = pipe()?;

    let size = inhirentConsoleSize()?;

    let console = unsafe { CreatePseudoConsole(size, pty_in, pty_out, 0)? };

    // Note: We can close the handles to the PTY-end of the pipes here
    // because the handles are dup'ed into the ConHost and will be released
    // when the ConPTY is destroyed.
    unsafe {
        CloseHandle(pty_in);
    }
    unsafe {
        CloseHandle(pty_out);
    }

    Ok((console, con_reader, con_writer))
}

fn inhirentConsoleSize() -> windows::Result<COORD> {
    let stdout_h = stdout_handle()?;
    let mut info = CONSOLE_SCREEN_BUFFER_INFO::default();
    unsafe {
        GetConsoleScreenBufferInfo(stdout_h, &mut info).ok()?;
        CloseHandle(stdout_h);
    };

    let mut size = COORD { X: 24, Y: 80 };
    size.X = info.srWindow.Right - info.srWindow.Left + 1;
    size.Y = info.srWindow.Bottom - info.srWindow.Top + 1;

    Ok(size)
}

// const PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE: usize = 22 | 0x0002_0000;
const PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE: usize = 0x00020016;

fn initializeStartupInfoAttachedToConPTY(hPC: &mut HPCON) -> windows::Result<STARTUPINFOEXW> {
    let mut siEx = STARTUPINFOEXW::default();
    siEx.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;

    let mut size: usize = 0;
    let res = unsafe {
        InitializeProcThreadAttributeList(LPPROC_THREAD_ATTRIBUTE_LIST::default(), 1, 0, &mut size)
    };
    if res.as_bool() || size == 0 {
        return Err(windows::Error::new(HRESULT::from_thread(), ""));
    }

    // SAFETY
    // we leak the memory intentionally,
    // it will be freed on DROP.
    let lpAttributeList = vec![0u8; size].into_boxed_slice();
    let lpAttributeList = Box::leak(lpAttributeList);

    siEx.lpAttributeList = LPPROC_THREAD_ATTRIBUTE_LIST(lpAttributeList.as_mut_ptr().cast());

    unsafe {
        InitializeProcThreadAttributeList(siEx.lpAttributeList, 1, 0, &mut size).ok()?;
        UpdateProcThreadAttribute(
            siEx.lpAttributeList,
            0,
            PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE,
            hPC.0 as _,
            size_of::<HPCON>(),
            null_mut(),
            null_mut(),
        )
        .ok()?;
    }

    Ok(siEx)
}

fn execProc(
    mut startup_info: STARTUPINFOEXW,
    attr: ProcAttr,
) -> windows::Result<PROCESS_INFORMATION> {
    if attr.commandline.is_none() && attr.application.is_none() {
        panic!("")
    }

    println!("attr {:?}", attr);

    let mut commandline = pwstr_param(attr.commandline);
    let mut application = pwstr_param(attr.application);
    let mut current_dir = pwstr_param(attr.current_dir);
    let env = match attr.env {
        Some(env) => Box::<[u16]>::into_raw(environment_block_unicode(env).into_boxed_slice()) as _,
        None => null_mut(),
    };

    let mut proc_info = PROCESS_INFORMATION::default();
    let result = unsafe {
        CreateProcessW(
            application.abi(),
            commandline.abi(),
            null_mut(),
            null_mut(),
            false,
            EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT, // CREATE_UNICODE_ENVIRONMENT | CREATE_NEW_CONSOLE
            env,
            current_dir.abi(),
            &mut startup_info.StartupInfo,
            &mut proc_info,
        )
        .ok()
    };

    if !env.is_null() {
        unsafe {
            ::std::boxed::Box::from_raw(env);
        }
    }

    result?;

    Ok(proc_info)
}

fn pipe() -> windows::Result<(HANDLE, HANDLE)> {
    let mut p_in = HANDLE::default();
    let mut p_out = HANDLE::default();
    unsafe { CreatePipe(&mut p_in, &mut p_out, std::ptr::null_mut(), 0).ok()? };

    Ok((p_in, p_out))
}

fn stdout_handle() -> windows::Result<HANDLE> {
    // we can't use `GetStdHandle(STD_OUTPUT_HANDLE)`
    // because it doesn't work when the IO is redirected
    //
    // https://stackoverflow.com/questions/33476316/win32-getconsolemode-error-code-6

    let hConsole = unsafe {
        CreateFileW(
            "CONOUT$",
            FILE_GENERIC_READ | FILE_GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            std::ptr::null_mut(),
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            HANDLE::NULL,
        )
    };

    if hConsole.is_null() || hConsole.is_invalid() {
        Err(HRESULT::from_thread().into())
    } else {
        Ok(hConsole)
    }
}

fn environment_block_unicode(env: HashMap<String, String>) -> Vec<u16> {
    if env.is_empty() {
        // two '\0' in UTF-16/UCS-2
        // four '\0' in UTF-8
        return vec![0, 0];
    }

    let mut b = Vec::new();
    for (key, value) in env {
        let part = format!("{}={}\0", key, value);
        b.extend(part.encode_utf16());
    }

    b.push(0);

    b
}

// if given string is empty there will be produced a "\0" string in UTF-16
fn pwstr_param(s: Option<String>) -> windows::Param<'static, PWSTR> {
    use windows::IntoParam;
    match s {
        Some(s) => {
            // https://github.com/microsoft/windows-rs/blob/ba61866b51bafac94844a242f971739583ffa70e/crates/gen/src/pwstr.rs
            s.into_param()
        }
        None => {
            // the memory will be zeroed
            // https://github.com/microsoft/windows-rs/blob/e1ab47c00b10b220d1372e4cdbe9a689d6365001/src/runtime/param.rs
            windows::Param::None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::prelude::*;
    use std::iter::FromIterator;

    // not sure if's desired behaiviour
    #[test]
    pub fn close_one_pty_input_close_others() {
        let mut proc = spawn("cmd").unwrap();
        let writer1 = proc.pty_input().unwrap();
        let mut writer2 = proc.pty_input().unwrap();

        assert!(writer2.write(b"").is_ok());

        drop(writer1);

        assert!(writer2.write(b"").is_err());
    }

    // not sure if's desired behaiviour
    // todo: timeout for wait/exit
    #[test]
    pub fn env_parameter() {
        let batch = r#"if "%TEST_ENV%"=="123456" (exit 0) else (exit 1)"#;
        let mut proc = ProcAttr::cmd(batch.to_string())
            .env("TEST_ENV".to_string(), "123456".to_string())
            .spawn()
            .unwrap();
        assert_eq!(proc.wait().unwrap(), 0);

        let mut proc = ProcAttr::cmd(batch.to_string())
            .env("TEST_ENV".to_string(), "NOT_CORRENT_VALUE".to_string())
            .spawn()
            .unwrap();
        assert_eq!(proc.wait().unwrap(), 1);

        // not set
        let mut proc = ProcAttr::cmd(batch.to_string()).spawn().unwrap();
        assert_eq!(proc.wait().unwrap(), 1);
    }

    #[test]
    fn env_block_test() {
        assert_eq!(
            environment_block_unicode(HashMap::from_iter([("asd".to_string(), "qwe".to_string())])),
            str_to_utf16("asd=qwe\0\0")
        );
        assert!(matches!(environment_block_unicode(HashMap::from_iter([
                ("asd".to_string(), "qwe".to_string()),
                ("zxc".to_string(), "123".to_string())
            ])), s if s == str_to_utf16("asd=qwe\0zxc=123\0\0") || s == str_to_utf16("zxc=123\0asd=qwe\0\0")));
        assert_eq!(
            environment_block_unicode(HashMap::from_iter([])),
            str_to_utf16("\0\0")
        );
    }

    fn str_to_utf16(s: impl AsRef<str>) -> Vec<u16> {
        s.as_ref().encode_utf16().collect()
    }
}
