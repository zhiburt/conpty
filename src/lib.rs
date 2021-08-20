#![allow(non_snake_case)]

mod bindings {
    windows::include_bindings!();
}

use bindings::{
    Windows::Win32::Foundation::CloseHandle,
    Windows::Win32::Foundation::{HANDLE, PWSTR},
    Windows::Win32::Storage::FileSystem::{
        CreateFileW, ReadFile, FILE_ATTRIBUTE_NORMAL, FILE_GENERIC_READ, FILE_GENERIC_WRITE,
        FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING, FlushFileBuffers, WriteFile,
    },
    Windows::Win32::System::Console::{
        ClosePseudoConsole, CreatePseudoConsole, GetConsoleMode, GetConsoleScreenBufferInfo,
        ResizePseudoConsole, SetConsoleMode, CONSOLE_MODE, CONSOLE_SCREEN_BUFFER_INFO, COORD,
        ENABLE_VIRTUAL_TERMINAL_PROCESSING, HPCON,
    },
    Windows::Win32::System::Pipes::CreatePipe,
    Windows::Win32::System::Threading::{
        CreateProcessW, DeleteProcThreadAttributeList, GetExitCodeProcess, GetProcessId,
        InitializeProcThreadAttributeList, UpdateProcThreadAttribute, WaitForSingleObject,
        CREATE_UNICODE_ENVIRONMENT, EXTENDED_STARTUPINFO_PRESENT, LPPROC_THREAD_ATTRIBUTE_LIST,
        PROCESS_INFORMATION, STARTUPINFOEXW, WAIT_TIMEOUT,
    },
    Windows::Win32::System::WindowsProgramming::INFINITE,
};
use std::fs::File;
use std::io;
use std::os::windows::io::FromRawHandle;
use std::{mem::size_of, ptr::null_mut};
use windows::HRESULT;

pub fn spawn(cmd: impl AsRef<str>) -> windows::Result<Proc> {
    Proc::spawn(cmd)
}

pub struct Proc {
    pty_input: HANDLE,
    pty_output: HANDLE,
    _proc: PROCESS_INFORMATION,
    _proc_info: STARTUPINFOEXW,
    _console: HPCON,
}

impl Proc {
    pub fn spawn(cmd: impl AsRef<str>) -> windows::Result<Self> {
        enableVirtualTerminalSequenceProcessing().unwrap();
        let (mut console, pty_reader, pty_writer) = createPseudoConsole().unwrap();
        let startup_info = initializeStartupInfoAttachedToConPTY(&mut console).unwrap();
        let proc = execProc(startup_info, cmd);

        Ok(Self {
            pty_input: pty_writer,
            pty_output: pty_reader,
            _console: console,
            _proc: proc,
            _proc_info: startup_info,
        })
    }

    pub fn resize(&self, x: i16, y: i16) -> windows::Result<()> {
        unsafe { ResizePseudoConsole(self._console.clone(), COORD { X: x, Y: y })? };
        Ok(())
    }

    pub fn pid(&self) -> u32 {
        unsafe { GetProcessId(self._proc.hProcess) }
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

    pub fn pty_input(&self) -> File {
        unsafe { File::from_raw_handle(self.pty_input.0 as _) }
    }

    pub fn pty_output(&self) -> File {
        unsafe { File::from_raw_handle(self.pty_output.0 as _) }
    }
}

impl Drop for Proc {
    fn drop(&mut self) {
        unsafe {
            ClosePseudoConsole(self._console);

            CloseHandle(self._proc.hProcess);
            CloseHandle(self._proc.hThread);

            DeleteProcThreadAttributeList(self._proc_info.lpAttributeList);
            unsafe { let _ = Box::from_raw(self._proc_info.lpAttributeList.0 as _); }

            CloseHandle(self.pty_input);
            CloseHandle(self.pty_output);
        }
    }
}

impl io::Write for Proc {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // fixme: downcasting usize to u32 on x64 may cause issue
        let mut written = 0;
        unsafe { WriteFile(self.pty_input, buf.as_ptr() as _, buf.len() as u32, &mut written, null_mut()).ok().unwrap() };
        Ok(written as usize)
    }

    fn flush(&mut self) -> io::Result<()> {
        unsafe { FlushFileBuffers(self.pty_input).ok().unwrap() };
        Ok(())
    }
}

impl io::Read for Proc {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut read = 0;
        unsafe { ReadFile(self.pty_output, buf.as_mut_ptr() as _, buf.len() as u32, &mut read, null_mut()).ok().unwrap(); }
        Ok(read as usize)
    }
}

fn enableVirtualTerminalSequenceProcessing() -> windows::Result<()> {
    let stdout_h = stdout_handle()?;
    unsafe {
        let mut mode = CONSOLE_MODE::default();
        GetConsoleMode(stdout_h, &mut mode).ok().unwrap();
        mode |= ENABLE_VIRTUAL_TERMINAL_PROCESSING; // DISABLE_NEWLINE_AUTO_RETURN
        SetConsoleMode(stdout_h, mode).ok().unwrap();

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

fn execProc(mut startup_info: STARTUPINFOEXW, command: impl AsRef<str>) -> PROCESS_INFORMATION {
    let inter = std::env::var("COMSPEC").unwrap();
    // The Unicode version of this function, CreateProcessW, can modify the contents of this string.
    // Therefore, this parameter cannot be a pointer to read-only memory (such as a const variable or a literal string).
    // If this parameter is a constant string, the function may cause an access violation.
    let cmd = command.as_ref().to_owned();
    let cmd = format!("{} /C {:?}", inter, cmd);

    println!("cmd {:?}", cmd);

    let mut proc_info = PROCESS_INFORMATION::default();
    unsafe {
        CreateProcessW(
            PWSTR::NULL,
            cmd,
            null_mut(),
            null_mut(),
            false,
            EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT, // CREATE_UNICODE_ENVIRONMENT | CREATE_NEW_CONSOLE
            null_mut(),
            PWSTR::NULL,
            &mut startup_info.StartupInfo,
            &mut proc_info,
        )
        .ok()
        .unwrap()
    };

    proc_info
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
