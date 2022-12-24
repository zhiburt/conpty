#![allow(non_snake_case)]

use std::{
    ffi::{c_void, OsStr, OsString},
    fmt,
    mem::size_of,
    os::windows::prelude::OsStrExt,
    process::Command,
    ptr::{null, null_mut},
    time::Duration,
};

use windows::{
    core::{self as win, HRESULT},
    Win32::{
        Foundation::{CloseHandle, HANDLE, PWSTR, WAIT_TIMEOUT},
        Storage::FileSystem::{
            CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_GENERIC_READ, FILE_GENERIC_WRITE,
            FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
        },
        System::{
            Console::{
                ClosePseudoConsole, CreatePseudoConsole, GetConsoleMode,
                GetConsoleScreenBufferInfo, ResizePseudoConsole, SetConsoleMode, CONSOLE_MODE,
                CONSOLE_SCREEN_BUFFER_INFO, COORD, ENABLE_ECHO_INPUT, ENABLE_LINE_INPUT,
                ENABLE_VIRTUAL_TERMINAL_PROCESSING, HPCON,
            },
            Pipes::CreatePipe,
            Threading::{
                CreateProcessW, DeleteProcThreadAttributeList, GetExitCodeProcess, GetProcessId,
                InitializeProcThreadAttributeList, TerminateProcess, UpdateProcThreadAttribute,
                WaitForSingleObject, CREATE_UNICODE_ENVIRONMENT, EXTENDED_STARTUPINFO_PRESENT,
                LPPROC_THREAD_ATTRIBUTE_LIST, PROCESS_INFORMATION, STARTUPINFOEXW,
            },
            WindowsProgramming::INFINITE,
        },
    },
};

use crate::{
    error::Error,
    io::{PipeReader, PipeWriter},
    util::clone_handle,
};

/// The structure is resposible for interations with spawned process.
/// It handles IO and other operations related to a spawned process.
pub struct Process {
    input: HANDLE,
    output: HANDLE,
    _proc: PROCESS_INFORMATION,
    _proc_info: STARTUPINFOEXW,
    _console: HPCON,
}

impl Process {
    /// Spawn a given command.
    ///
    /// ```ignore
    /// # // todo: determine why this test timeouts if runnin as a doc test but not as an example/test.
    /// use std::io::prelude::*;
    /// use std::process::Command;
    /// use conpty::Process;
    ///
    /// let mut cmd = Command::new("cmd");
    /// cmd.args(&["/C", "echo Hello World"]);
    ///
    /// let proc = Process::spawn(cmd).unwrap();
    /// let mut reader = proc.output().unwrap();
    ///
    /// let mut buf = [0; 1028];
    /// let n = reader.read(&mut buf).unwrap();
    /// assert!(String::from_utf8_lossy(&buf[..n]).contains("Hello World"));
    /// ```
    pub fn spawn(command: Command) -> Result<Self, Error> {
        spawn_command(command)
    }

    /// Returns a process's pid.
    pub fn pid(&self) -> u32 {
        get_process_pid(self._proc.hProcess)
    }

    /// Waits before process exists.
    pub fn wait(&self, timeout_millis: Option<u32>) -> Result<u32, Error> {
        wait_process(self._proc.hProcess, timeout_millis)
    }

    /// Is alive determines if a process is still running.
    ///
    /// IMPORTANT: Beware to use it in a way to stop reading when is_alive is false.
    //  Because at the point of calling method it may be alive but at the point of `read` call it may already not.
    pub fn is_alive(&self) -> bool {
        is_process_alive(self._proc.hProcess)
    }

    /// Resizes virtual terminal.
    pub fn resize(&mut self, x: i16, y: i16) -> Result<(), Error> {
        resize_console(self._console, x, y)
    }

    /// Termianates process with exit_code.
    pub fn exit(&mut self, code: u32) -> Result<(), Error> {
        kill_process(self._proc.hProcess, code)
    }

    /// Sets echo mode for a session.
    pub fn set_echo(&mut self, on: bool) -> Result<(), Error> {
        console_stdout_set_echo(on)
    }

    /// Returns a pipe writer to conPTY.
    pub fn input(&mut self) -> Result<PipeWriter, Error> {
        // see [Self::output]
        let handle = clone_handle(self.input)?;
        Ok(PipeWriter::new(handle))
    }

    /// Returns a pipe reader from conPTY.
    pub fn output(&mut self) -> Result<PipeReader, Error> {
        // It's crusial to clone first and not affect original HANDLE
        // as closing it closes all other's handles even though it's kindof unxpected.
        //
        // "
        // Closing a handle does not close the object.  It merely reduces the
        // "reference count".  When the reference count goes to zero, the object
        // itself is closed.  So, if you have a file handle, and you duplicate that
        // handle, the file now has two "references".  If you close one handle, the
        // file still has one reference, so the FILE cannot be closed.
        // "
        //
        // https://social.msdn.microsoft.com/Forums/windowsdesktop/en-US/1754715c-45b7-4d8c-ba56-a501ccaec12c/closehandle-amp-duplicatehandle?forum=windowsgeneraldevelopmentissues
        let handle = clone_handle(self.output)?;
        Ok(PipeReader::new(handle))
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        unsafe {
            ClosePseudoConsole(self._console);

            let _ = CloseHandle(self._proc.hProcess);
            let _ = CloseHandle(self._proc.hThread);

            DeleteProcThreadAttributeList(self._proc_info.lpAttributeList);
            let _ = Box::from_raw(self._proc_info.lpAttributeList as _);

            let _ = CloseHandle(self.input);
            let _ = CloseHandle(self.output);
        }
    }
}

impl fmt::Debug for Process {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PipeReader")
            .field("pty_output", &(self.output.0))
            .field("pty_output(ptr)", &(self.output.0 as *const c_void))
            .field("pty_input", &(self.input.0))
            .field("pty_input(ptr)", &(self.input.0 as *const c_void))
            .finish_non_exhaustive()
    }
}

fn enableVirtualTerminalSequenceProcessing() -> win::Result<()> {
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

fn createPseudoConsole() -> win::Result<(HPCON, HANDLE, HANDLE)> {
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

fn inhirentConsoleSize() -> win::Result<COORD> {
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

fn initializeStartupInfoAttachedToConPTY(hPC: &mut HPCON) -> win::Result<STARTUPINFOEXW> {
    let mut siEx = STARTUPINFOEXW::default();
    siEx.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;

    let mut size: usize = 0;
    let res = unsafe { InitializeProcThreadAttributeList(null_mut() as _, 1, 0, &mut size) };
    if res.as_bool() || size == 0 {
        return Err(win::Error::new(
            HRESULT::default(),
            "failed initialize proc attribute list".into(),
        ));
    }

    // SAFETY
    // we leak the memory intentionally,
    // it will be freed on DROP.
    let lpAttributeList = vec![0u8; size].into_boxed_slice();
    let lpAttributeList = Box::leak(lpAttributeList);

    siEx.lpAttributeList = lpAttributeList.as_mut_ptr().cast() as LPPROC_THREAD_ATTRIBUTE_LIST;

    unsafe {
        InitializeProcThreadAttributeList(siEx.lpAttributeList, 1, 0, &mut size).ok()?;
        UpdateProcThreadAttribute(
            siEx.lpAttributeList,
            0,
            PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE,
            *hPC as _,
            size_of::<HPCON>(),
            null_mut(),
            null_mut(),
        )
        .ok()?;
    }

    Ok(siEx)
}

fn execProc(command: Command, startup_info: STARTUPINFOEXW) -> win::Result<PROCESS_INFORMATION> {
    let commandline = build_commandline(&command);
    let mut commandline = convert_osstr_to_utf16(&commandline);
    let commandline = PWSTR(commandline.as_mut_ptr());

    let current_dir = command.get_current_dir();
    let mut current_dir = current_dir.map(|p| convert_osstr_to_utf16(p.as_os_str()));
    let current_dir = current_dir
        .as_mut()
        .map_or(null_mut(), |dir| dir.as_mut_ptr());
    let current_dir = PWSTR(current_dir);

    let envs_list = || {
        command
            .get_envs()
            .filter_map(|(key, value)| value.map(|value| (key, value)))
    };
    let envs = environment_block_unicode(envs_list());
    let envs = if envs_list().next().is_some() {
        envs.as_ptr() as _
    } else {
        null()
    };

    let appname = PWSTR(null_mut());
    let dwflags = EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT; // CREATE_UNICODE_ENVIRONMENT | CREATE_NEW_CONSOLE

    let mut proc_info = PROCESS_INFORMATION::default();
    unsafe {
        CreateProcessW(
            appname,
            commandline,
            null_mut(),
            null_mut(),
            false,
            dwflags,
            envs,
            current_dir,
            &startup_info.StartupInfo,
            &mut proc_info,
        )
        .ok()?
    };

    Ok(proc_info)
}

fn build_commandline(command: &Command) -> OsString {
    let mut buf = OsString::new();
    buf.push(command.get_program());

    for arg in command.get_args() {
        buf.push(" ");
        buf.push(arg);
    }

    buf
}

fn pipe() -> win::Result<(HANDLE, HANDLE)> {
    let mut p_in = HANDLE::default();
    let mut p_out = HANDLE::default();
    unsafe { CreatePipe(&mut p_in, &mut p_out, std::ptr::null_mut(), 0).ok()? };

    Ok((p_in, p_out))
}

fn stdout_handle() -> win::Result<HANDLE> {
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
            HANDLE::default(),
        )
        .ok()?
    };

    Ok(hConsole)
}

fn environment_block_unicode<'a>(
    env: impl IntoIterator<Item = (&'a OsStr, &'a OsStr)>,
) -> Vec<u16> {
    let mut b = Vec::new();
    for (key, value) in env {
        b.extend(key.encode_wide());
        b.extend("=".encode_utf16());
        b.extend(value.encode_wide());
        b.push(0);
    }

    if b.is_empty() {
        // two '\0' in UTF-16/UCS-2
        // four '\0' in UTF-8
        return vec![0, 0];
    }

    b.push(0);

    b
}

// if given string is empty there will be produced a "\0" string in UTF-16
fn convert_osstr_to_utf16(s: &OsStr) -> Vec<u16> {
    let mut bytes: Vec<_> = s.encode_wide().collect();
    bytes.push(0);
    bytes
}

fn console_stdout_set_echo(on: bool) -> Result<(), Error> {
    // todo: determine if this function is usefull and it works?
    let stdout_h = stdout_handle()?;

    let mut mode = CONSOLE_MODE::default();
    unsafe { GetConsoleMode(stdout_h, &mut mode).ok()? };

    match on {
        true => mode |= ENABLE_ECHO_INPUT | ENABLE_LINE_INPUT,
        false => mode &= !ENABLE_ECHO_INPUT,
    };

    unsafe {
        SetConsoleMode(stdout_h, mode).ok()?;
        CloseHandle(stdout_h).ok()?;
    }

    Ok(())
}

fn spawn_command(command: Command) -> Result<Process, Error> {
    enableVirtualTerminalSequenceProcessing()?;
    let (mut console, output, input) = createPseudoConsole()?;
    let startup_info = initializeStartupInfoAttachedToConPTY(&mut console)?;
    let proc = execProc(command, startup_info)?;
    Ok(Process {
        input,
        output,
        _console: console,
        _proc: proc,
        _proc_info: startup_info,
    })
}

fn resize_console(console: HPCON, x: i16, y: i16) -> Result<(), Error> {
    unsafe { ResizePseudoConsole(console, COORD { X: x, Y: y }) }?;
    Ok(())
}

fn get_process_pid(proc: HANDLE) -> u32 {
    unsafe { GetProcessId(proc) }
}

fn kill_process(proc: HANDLE, code: u32) -> Result<(), Error> {
    unsafe { TerminateProcess(proc, code).ok()? };
    Ok(())
}

fn is_process_alive(proc: HANDLE) -> bool {
    // https://stackoverflow.com/questions/1591342/c-how-to-determine-if-a-windows-process-is-running/5303889
    unsafe { WaitForSingleObject(proc, 0) == WAIT_TIMEOUT }
}

fn wait_process(proc: HANDLE, timeout_millis: Option<u32>) -> Result<u32, Error> {
    match timeout_millis {
        Some(timeout) => {
            let result = unsafe { WaitForSingleObject(proc, timeout) };
            if result == WAIT_TIMEOUT {
                return Err(Error::Timeout(Duration::from_millis(timeout as u64)));
            }
        }
        None => {
            unsafe { WaitForSingleObject(proc, INFINITE) };
        }
    }

    let mut code = 0;
    unsafe {
        GetExitCodeProcess(proc, &mut code).ok()?;
    }

    Ok(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_block_test() {
        let tests = [
            (vec![], "\0\0"),
            (vec![(OsStr::new("asd"), OsStr::new("qwe"))], "asd=qwe\0\0"),
            (
                vec![
                    (OsStr::new("asd"), OsStr::new("qwe")),
                    (OsStr::new("zxc"), OsStr::new("123")),
                ],
                "asd=qwe\0zxc=123\0\0",
            ),
        ];

        for (m, expected) in tests {
            let env = environment_block_unicode(m);
            let expected = str_to_utf16(expected);

            assert_eq!(env, expected,);
        }
    }

    fn str_to_utf16(s: impl AsRef<str>) -> Vec<u16> {
        s.as_ref().encode_utf16().collect()
    }
}
