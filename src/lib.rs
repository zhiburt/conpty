#![allow(non_snake_case)]

mod bindings {
    windows::include_bindings!();
}

use bindings::{
    Windows::Win32::Foundation::CloseHandle,
    Windows::Win32::Foundation::{HANDLE, PWSTR},
    Windows::Win32::Storage::FileSystem::{
        CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_GENERIC_READ, FILE_GENERIC_WRITE, FILE_SHARE_READ,
        FILE_SHARE_WRITE, OPEN_EXISTING, ReadFile
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
use std::fs::File;
use std::io;
use std::os::windows::io::FromRawHandle;
use std::{mem::size_of, ptr::null_mut};
use windows::HRESULT;
use std::collections::HashMap;

pub fn spawn() -> windows::Result<()> {
    enableVirtualTerminalSequenceProcessing();
    let (mut console, pty_reader, pty_writer) = createPseudoConsole().unwrap();
    let mut startup_info = initializeStartupInfoAttachedToConPTY(&mut console).unwrap();

    execProc(startup_info);
    
    let mut read = 0;
    let mut buf = [0; 300]; 
    unsafe { ReadFile(pty_reader, buf.as_mut_ptr() as _, buf.len() as u32, &mut read, null_mut()); }

    Ok(())
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

    let size = COORD { X: 24, Y: 80 };

    let console = unsafe { CreatePseudoConsole(size, pty_in, pty_out, 0)? };


    unsafe {
        CloseHandle(pty_in);
    }
    unsafe {
        CloseHandle(pty_out);
    }

    Ok((console, con_reader, con_writer))
}

const PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE: usize = 0x00020016;  // 22 | 0x0002_0000

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

fn execProc(mut startup_info: STARTUPINFOEXW) -> PROCESS_INFORMATION {
    let cmd = "ping";
    
    let mut proc_info = PROCESS_INFORMATION::default();
    unsafe {
        CreateProcessW(
            PWSTR::NULL, // is it save to use *mut u8 as *mut u16
            cmd,
            null_mut(),
            null_mut(),
            false,
            EXTENDED_STARTUPINFO_PRESENT,
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