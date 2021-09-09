use crate::bindings::{
    Windows::Win32::Foundation::{DuplicateHandle, DUPLICATE_SAME_ACCESS, HANDLE},
    Windows::Win32::System::Console::{
        GetStdHandle, STD_INPUT_HANDLE,
        CONSOLE_MODE,
        ENABLE_ECHO_INPUT, ENABLE_LINE_INPUT, ENABLE_MOUSE_INPUT,
        ENABLE_WINDOW_INPUT, ENABLE_PROCESSED_INPUT, ENABLE_EXTENDED_FLAGS,
        ENABLE_INSERT_MODE, ENABLE_QUICK_EDIT_MODE, ENABLE_VIRTUAL_TERMINAL_INPUT,
        SetConsoleMode, GetConsoleMode,
    },
    Windows::Win32::System::Threading::GetCurrentProcess,
    Windows::Win32::System::Threading::{WaitForSingleObject, WAIT_OBJECT_0},
};

use std::io;

// todo: create a Console type which could store old modes to be able to restore it after making it raw.
pub fn set_raw() -> windows::Result<()> {
    // fixme:
    // it's not guaranted that stdin is connected to console, as
    // it may be redirected.
    // see https://github.com/containerd/console/blob/05dadd92d21fc51f0bf56eadcb4201955cfc98d8/console.go#L65-L78
    let stdin = unsafe { GetStdHandle(STD_INPUT_HANDLE) };
    _set_raw(stdin)
}

pub fn _set_raw(stdin: HANDLE) -> windows::Result<()> {
    let mut mode = CONSOLE_MODE::default();
    unsafe { GetConsoleMode(stdin, &mut mode).ok()?; }

    mode &= CONSOLE_MODE(!ENABLE_ECHO_INPUT.0);
	mode &= CONSOLE_MODE(!ENABLE_LINE_INPUT.0);
	mode &= CONSOLE_MODE(!ENABLE_MOUSE_INPUT.0);
	mode &= CONSOLE_MODE(!ENABLE_WINDOW_INPUT.0);
	mode &= CONSOLE_MODE(!ENABLE_PROCESSED_INPUT.0);

	mode |= CONSOLE_MODE(ENABLE_EXTENDED_FLAGS);
	mode |= CONSOLE_MODE(ENABLE_INSERT_MODE.0);
	mode |= CONSOLE_MODE(ENABLE_QUICK_EDIT_MODE.0);

    let vtInputSupported = true;
	if vtInputSupported {
		mode |= ENABLE_VIRTUAL_TERMINAL_INPUT;
	}

    unsafe { SetConsoleMode(stdin, mode).ok()?; }

    Ok(())
}

/// clone_handle can be used to clone a general HANDLE.
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

/// is_handle_ready can be used with FILE HANDLEs to determine if there's
/// something to read.
pub fn is_stdin_not_empty() -> std::io::Result<bool> {
    // https://stackoverflow.com/questions/23164492/how-can-i-detect-if-there-is-input-waiting-on-stdin-on-windows
    Ok(unsafe { WaitForSingleObject(GetStdHandle(STD_INPUT_HANDLE), 0) == WAIT_OBJECT_0 })
}

/// is_handle_ready can be used with FILE HANDLEs to determine if there's
/// something to read.
pub fn is_handle_ready(handle: HANDLE) -> std::io::Result<bool> {
    Ok(unsafe { WaitForSingleObject(handle, 0) == WAIT_OBJECT_0 })
}

pub(crate) fn win_error_to_io(err: windows::Error) -> io::Error {
    let code = err.code();
    io::Error::from_raw_os_error(code.0 as i32)
}
