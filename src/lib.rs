#![allow(non_snake_case)]

mod bindings {
    windows::include_bindings!();
}

use bindings::{
    Windows::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE},
    Windows::Win32::Foundation::{HANDLE, PWSTR},
    Windows::Win32::System::Console::{
        ClosePseudoConsole, CreatePseudoConsole, GetConsoleMode, GetConsoleScreenBufferInfo,
        GetStdHandle, ResizePseudoConsole, SetConsoleMode, CONSOLE_MODE,
        CONSOLE_SCREEN_BUFFER_INFO, COORD, ENABLE_VIRTUAL_TERMINAL_PROCESSING, HPCON,
        STD_OUTPUT_HANDLE,
    },
    Windows::Win32::System::Pipes::CreatePipe,
    Windows::Win32::System::Threading::{
        CreateProcessW, DeleteProcThreadAttributeList, InitializeProcThreadAttributeList,
        UpdateProcThreadAttribute, WaitForSingleObject, EXTENDED_STARTUPINFO_PRESENT,
        LPPROC_THREAD_ATTRIBUTE_LIST, PROCESS_INFORMATION, STARTUPINFOEXW,
        GetProcessId, GetExitCodeProcess, CREATE_UNICODE_ENVIRONMENT, WAIT_TIMEOUT,
    },
    Windows::Win32::System::WindowsProgramming::INFINITE,
    Windows::Win32::System::SystemServices::{
        GENERIC_READ
    },
    Windows::Win32::Storage::FileSystem::{
        FILE_SHARE_READ,FILE_SHARE_WRITE,OPEN_EXISTING,FILE_ATTRIBUTE_NORMAL, FILE_GENERIC_READ,  FILE_GENERIC_WRITE, CreateFileW
    },
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
    pty_in: File,
    pty_out: File,
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

        let f_reader = unsafe { File::from_raw_handle(pty_reader.0 as _) };
        let f_writer = unsafe { File::from_raw_handle(pty_writer.0 as _) };

        Ok(Self {
            pty_in: f_writer,
            pty_out: f_reader,
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


    // fn send_line(&self, b: impl AsRef<str>) -> windows::Result<usize> {
    //     let b = b.as_ref().as_bytes().as_mut_ptr();
    //     let buf_len = b.as_ref().as_bytes().len();
    //     let bytes_written = 0;
    //     WriteFile(self.pty_out, b as _, buf_len, &mut bytes_written as _, null_mut()).ok()?;

    //     Ok(bytes_written)
    // }
}

impl Drop for Proc {
    fn drop(&mut self) {
        unsafe {
            ClosePseudoConsole(self._console);

            CloseHandle(self._proc.hProcess);
            CloseHandle(self._proc.hThread);

            DeleteProcThreadAttributeList(self._proc_info.lpAttributeList);

            // Handles will be closes when File's will be dropped
            //
            // CloseHandle(hPipeOut);
            // CloseHandle(hPipeOut);
        }
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
        InitializeProcThreadAttributeList(LPPROC_THREAD_ATTRIBUTE_LIST(null_mut()), 1, 0, &mut size)
    };
    if res.as_bool() || size == 0 {
        return Err(windows::Error::new(HRESULT::from_thread(), ""));
    }

    let mut lpAttributeList = vec![0u8; size].into_boxed_slice();
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
    let mut cmd = command.as_ref().to_owned();
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

impl io::Write for Proc {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.pty_in.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.pty_in.flush()
    }

    fn write_vectored(&mut self, bufs: &[io::IoSlice<'_>]) -> io::Result<usize> {
        self.pty_in.write_vectored(bufs)
    }
}

impl io::Read for Proc {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.pty_out.read(buf)
    }
}

fn stdout_handle() -> windows::Result<HANDLE> {
    // we can't use `GetStdHandle(STD_OUTPUT_HANDLE)`
    // because it doesn't work when the IO is redirected
    //
    // https://stackoverflow.com/questions/33476316/win32-getconsolemode-error-code-6

    let hConsole = unsafe { CreateFileW(
        "CONOUT$",
        FILE_GENERIC_READ | FILE_GENERIC_WRITE,
        FILE_SHARE_READ|FILE_SHARE_WRITE, 
        std::ptr::null_mut(),
        OPEN_EXISTING,
        FILE_ATTRIBUTE_NORMAL,
        HANDLE::NULL,
    ) };

    if hConsole.is_null() || hConsole.is_invalid() {
        Err(HRESULT::from_thread().into())
    } else {
        Ok(hConsole)
    }
}