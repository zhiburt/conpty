use crate::bindings::{
    Windows::Win32::Foundation::CloseHandle,
    Windows::Win32::Foundation::{HANDLE, PWSTR, DuplicateHandle, DUPLICATE_SAME_ACCESS},
    Windows::Win32::Storage::FileSystem::{
        CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_GENERIC_READ, FILE_GENERIC_WRITE, FILE_SHARE_READ,
        FILE_SHARE_WRITE, OPEN_EXISTING, ReadFile, WriteFile, FlushFileBuffers,
    },
    Windows::Win32::System::Console::{
        ClosePseudoConsole, CreatePseudoConsole, GetConsoleMode, GetConsoleScreenBufferInfo,
        ResizePseudoConsole, SetConsoleMode, CONSOLE_MODE, CONSOLE_SCREEN_BUFFER_INFO, COORD,
        ENABLE_VIRTUAL_TERMINAL_PROCESSING, HPCON,
    },
    Windows::Win32::System::Pipes::{CreatePipe, SetNamedPipeHandleState, PIPE_NOWAIT},
    Windows::Win32::System::Threading::{
        CreateProcessW, DeleteProcThreadAttributeList, GetExitCodeProcess, GetProcessId,
        InitializeProcThreadAttributeList, TerminateProcess, UpdateProcThreadAttribute,
        WaitForSingleObject, CREATE_UNICODE_ENVIRONMENT, EXTENDED_STARTUPINFO_PRESENT,
        LPPROC_THREAD_ATTRIBUTE_LIST, PROCESS_INFORMATION, STARTUPINFOEXW, WAIT_TIMEOUT, GetCurrentProcess,
    },
    Windows::Win32::System::WindowsProgramming::{INFINITE, PIPE_WAIT},
};

use std::io::{Write, Read};
use std::ptr::null_mut;
use windows::HRESULT;

pub struct PipeReader {
    handle: HANDLE,
}

impl PipeReader {
    pub fn new(handle: HANDLE) -> Self {
        Self {
            handle,
        }
    }

    // Affects all dupped descriptors.
    //
    // Mainly developed to not pile down libraries to include any windows API crate.
    pub fn set_non_blocking_mode(&mut self) -> std::io::Result<()> {
        let mut nowait = PIPE_NOWAIT;
        unsafe { SetNamedPipeHandleState(self.handle, &mut nowait.0, null_mut(), null_mut()).ok().unwrap(); }
        Ok(())
    }

    // Affects all dupped descriptors.
    //
    // Mainly developed to not pile down libraries to include any windows API crate.
    pub fn set_blocking_mode(&mut self) -> std::io::Result<()> {
        let mut nowait = PIPE_WAIT;
        unsafe { SetNamedPipeHandleState(self.handle, &mut nowait, null_mut(), null_mut()).ok().unwrap(); }
        Ok(())
    }

    pub fn try_clone(&self) -> std::io::Result<Self> {
        clone_handle(self.handle).map(Self::new)
    }
}

impl Read for PipeReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut n = 0;
        let buf_size = buf.len() as u32;

        match unsafe { ReadFile(self.handle, buf.as_mut_ptr() as _, buf_size, &mut n, null_mut()).ok() } {
            Ok(()) => Ok(n as usize),
            // https://stackoverflow.com/questions/34504970/non-blocking-read-on-os-pipe-on-windows
            Err(err) if err.code() == HRESULT::from_win32(232) => Err(std::io::Error::new(std::io::ErrorKind::WouldBlock, err)),
            Err(err) => Err(err).unwrap(),
        }
    }
}

impl Into<std::fs::File> for PipeReader {
    fn into(self) -> std::fs::File {
        use std::os::windows::io::FromRawHandle;
        unsafe { std::fs::File::from_raw_handle(self.handle.0 as _) }
    }
}

pub struct PipeWriter {
    handle: HANDLE,
}

impl PipeWriter {
    pub fn new(handle: HANDLE) -> Self {
        Self {
            handle,
        }
    }

    pub fn try_clone(&self) -> std::io::Result<Self> {
        clone_handle(self.handle).map(Self::new)
    }
}

impl Write for PipeWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut n = 0;
        let buf_size = buf.len() as u32;

        unsafe { WriteFile(self.handle, buf.as_ptr() as _, buf_size, &mut n, null_mut()).ok().unwrap(); }

        Ok(n as usize)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        unsafe { FlushFileBuffers(self.handle).ok().unwrap() }
        Ok(()) 
    }
}

impl Into<std::fs::File> for PipeWriter {
    fn into(self) -> std::fs::File {
        use std::os::windows::io::FromRawHandle;
        unsafe { std::fs::File::from_raw_handle(self.handle.0 as _) }
    }
}

fn clone_handle(handle: HANDLE) -> std::io::Result<HANDLE> {
    let mut cloned_handle = HANDLE::default();
    unsafe {
        DuplicateHandle (unsafe { GetCurrentProcess() }, handle, unsafe { GetCurrentProcess() }, &mut cloned_handle, 0, false, DUPLICATE_SAME_ACCESS).ok().unwrap();
    }

    Ok(cloned_handle)
}
