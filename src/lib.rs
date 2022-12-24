//! A library which provides an interface for [ConPTY].
//!
//! ```ignore
//! # // todo: determine why this test timeouts if runnin as a doc test but not as an example.
//! use std::io::prelude::*;
//!
//! let proc = conpty::spawn("echo Hello World").unwrap();
//! let mut reader = proc.output().unwrap();
//!
//! proc.wait(None).unwrap();
//!
//! let mut buf = [0; 1028];
//! let n = reader.read(&mut buf).unwrap();
//! assert!(String::from_utf8_lossy(&buf[..n]).contains("Hello World"));
//! ```
//!
//! [ConPTY]: https://devblogs.microsoft.com/commandline/windows-command-line-introducing-the-windows-pseudo-console-conpty/

use std::{
    ffi::{OsStr, OsString},
    process::Command,
};

use error::Error;

pub mod console;
pub mod error;
pub mod io;

mod process;
mod util;

pub use process::Process;

/// Spawns a command using `cmd.exe`.
pub fn spawn(command: impl AsRef<OsStr>) -> Result<Process, Error> {
    let mut cmd = OsString::new();
    cmd.push("cmd /C ");
    cmd.push(command);

    Process::spawn(Command::new(&cmd))
}
