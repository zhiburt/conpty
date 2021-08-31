use crate::bindings::{
    Windows::Win32::Foundation::{CloseHandle, DuplicateHandle, DUPLICATE_SAME_ACCESS, HANDLE},
    Windows::Win32::Storage::FileSystem::{FlushFileBuffers, ReadFile, WriteFile},
    Windows::Win32::System::Pipes::{SetNamedPipeHandleState, PIPE_NOWAIT},
    Windows::Win32::System::Threading::GetCurrentProcess,
    Windows::Win32::System::WindowsProgramming::PIPE_WAIT,
};

use std::io::{self, Read, Write};
use std::ptr::null_mut;
use windows::HRESULT;

#[derive(Debug)]
pub struct PipeReader {
    handle: HANDLE,
}

impl PipeReader {
    pub fn new(handle: HANDLE) -> Self {
        Self { handle }
    }

    // Affects all dupped descriptors.
    //
    // Mainly developed to not pile down libraries to include any windows API crate.
    pub fn set_non_blocking_mode(&mut self) -> io::Result<()> {
        let mut nowait = PIPE_NOWAIT;
        unsafe {
            SetNamedPipeHandleState(self.handle, &mut nowait.0, null_mut(), null_mut())
                .ok()
                .map_err(win_error_to_io)?;
        }
        Ok(())
    }

    // Affects all dupped descriptors.
    //
    // Mainly developed to not pile down libraries to include any windows API crate.
    pub fn set_blocking_mode(&mut self) -> io::Result<()> {
        let mut nowait = PIPE_WAIT;
        unsafe {
            SetNamedPipeHandleState(self.handle, &mut nowait, null_mut(), null_mut())
                .ok()
                .map_err(win_error_to_io)?;
        }
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

        match unsafe {
            ReadFile(
                self.handle,
                buf.as_mut_ptr() as _,
                buf_size,
                &mut n,
                null_mut(),
            )
            .ok()
        } {
            Ok(()) => Ok(n as usize),
            // https://stackoverflow.com/questions/34504970/non-blocking-read-on-os-pipe-on-windows
            Err(err) if err.code() == HRESULT::from_win32(232) => {
                Err(std::io::Error::new(std::io::ErrorKind::WouldBlock, err))
            }
            Err(err) => Err(win_error_to_io(err)),
        }
    }
}

impl Drop for PipeReader {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.handle).ok().unwrap();
        }
    }
}

impl Into<std::fs::File> for PipeReader {
    fn into(self) -> std::fs::File {
        use std::os::windows::io::FromRawHandle;
        unsafe { std::fs::File::from_raw_handle(self.handle.0 as _) }
    }
}

#[derive(Debug)]
pub struct PipeWriter {
    handle: HANDLE,
}

impl PipeWriter {
    pub fn new(handle: HANDLE) -> Self {
        Self { handle }
    }

    pub fn try_clone(&self) -> std::io::Result<Self> {
        clone_handle(self.handle).map(Self::new)
    }
}

impl Write for PipeWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut n = 0;
        let buf_size = buf.len() as u32;

        unsafe {
            WriteFile(self.handle, buf.as_ptr() as _, buf_size, &mut n, null_mut())
                .ok()
                .map_err(win_error_to_io)?;
        }

        Ok(n as usize)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        unsafe {
            FlushFileBuffers(self.handle)
                .ok()
                .map_err(win_error_to_io)?;
        }
        Ok(())
    }
}

impl Into<std::fs::File> for PipeWriter {
    fn into(self) -> std::fs::File {
        use std::os::windows::io::FromRawHandle;
        unsafe { std::fs::File::from_raw_handle(self.handle.0 as _) }
    }
}

pub fn clone_handle(handle: HANDLE) -> std::io::Result<HANDLE> {
    let mut cloned_handle = HANDLE::default();
    unsafe {
        DuplicateHandle(
            GetCurrentProcess(),
            handle,
            GetCurrentProcess(),
            &mut cloned_handle,
            0,
            false,
            DUPLICATE_SAME_ACCESS,
        )
        .ok()
        .map_err(win_error_to_io)?;
    }

    Ok(cloned_handle)
}

fn win_error_to_io(err: windows::Error) -> io::Error {
    let code = err.code();
    io::Error::from_raw_os_error(code.0 as i32)
}
