//! This module contains [crate::Process]'s `Input` and `Output` pipes.
//!
//! Input - PipeWriter
//! Output - PipeReader

use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Storage::FileSystem::{FlushFileBuffers, ReadFile, WriteFile};
use windows::Win32::System::Pipes::PeekNamedPipe;

use crate::error::Error;
use crate::util::{clone_handle, win_error_to_io};
use std::ffi::c_void;
use std::io::{Read, Write};
use std::mem::MaybeUninit;
use std::ptr::{self, null_mut};
use std::{fmt, io};

/// PipeReader wraps a win32 pipe to provide a [std::io::Read] interface.
/// It also provides a non_blocking mode settings.
pub struct PipeReader {
    handle: HANDLE,
    blocking: bool,
}

impl PipeReader {
    /// Returns a new instance of PipeReader.
    pub fn new(handle: HANDLE) -> Self {
        Self {
            handle,
            blocking: true,
        }
    }

    /// Sets a pipe to a non blocking mode.
    ///
    /// It doesn't changes DUPed handles.
    ///
    /// Mainly developed to not pile down libraries to include any windows API crate.
    pub fn blocking(&mut self, on: bool) {
        self.blocking = on;
    }

    /// Tries to clone a instance to a new one.
    /// All cloned instances share the same underlaying data so
    /// Reading from one cloned pipe will affect an original pipe.
    pub fn try_clone(&self) -> Result<Self, Error> {
        clone_handle(self.handle).map_err(Into::into).map(Self::new)
    }
}

impl Read for PipeReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if !self.blocking {
            // So we could use SetNamedPipeHandleState but seems like it doesn't work sometimes?
            // Plus it changes all DUPed handles

            let available = unsafe {
                let mut bytes = MaybeUninit::<u32>::uninit();
                let bytes_ptr: *mut u32 = ptr::addr_of_mut!(*bytes.as_mut_ptr());

                PeekNamedPipe(
                    self.handle,
                    null_mut(),
                    0,
                    null_mut(),
                    bytes_ptr,
                    null_mut(),
                )
                .ok()
                .map_err(win_error_to_io)?;

                bytes.assume_init()
            };

            if available == 0 {
                return Err(io::Error::new(io::ErrorKind::WouldBlock, ""));
            }
        }

        let mut n = 0;
        let size = buf.len() as u32;
        let buf = buf.as_mut_ptr() as _;

        unsafe {
            ReadFile(self.handle, buf, size, &mut n, null_mut())
                .ok()
                .map_err(win_error_to_io)?;
        }

        Ok(n as usize)
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

impl fmt::Debug for PipeReader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PipeReader")
            .field("handle", &(self.handle.0))
            .field("handle(ptr)", &(self.handle.0 as *const c_void))
            .finish()
    }
}

/// PipeWriter implements [std::io::Write] interface for win32 pipe.
pub struct PipeWriter {
    handle: HANDLE,
}

impl PipeWriter {
    /// Creates a new instance of PipeWriter.
    ///
    /// It owns a HANDLE.
    pub fn new(handle: HANDLE) -> Self {
        Self { handle }
    }

    /// Tries to make a clone of PipeWriter.
    pub fn try_clone(&self) -> Result<Self, Error> {
        clone_handle(self.handle).map_err(Into::into).map(Self::new)
    }
}

impl Write for PipeWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut n = 0;
        let buf_size = buf.len() as u32;

        unsafe {
            WriteFile(self.handle, buf.as_ptr() as _, buf_size, &mut n, null_mut())
                .ok()
                .map_err(win_error_to_io)?;
        }

        Ok(n as usize)
    }

    fn flush(&mut self) -> io::Result<()> {
        unsafe {
            FlushFileBuffers(self.handle)
                .ok()
                .map_err(win_error_to_io)?;
        }
        Ok(())
    }
}

impl Drop for PipeWriter {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.handle).ok().unwrap();
        }
    }
}

impl Into<std::fs::File> for PipeWriter {
    fn into(self) -> std::fs::File {
        use std::os::windows::io::FromRawHandle;
        unsafe { std::fs::File::from_raw_handle(self.handle.0 as _) }
    }
}

impl fmt::Debug for PipeWriter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PipeReader")
            .field("handle", &(self.handle.0))
            .field("handle(ptr)", &(self.handle.0 as *const c_void))
            .finish()
    }
}
