use std::{fmt, time::Duration};

use windows::core as win;

#[derive(Debug)]
pub enum Error {
    Win(win::Error),
    Timeout(Duration),
}

impl std::error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Win(err) => writeln!(f, "Windows error: {}", err),
            Self::Timeout(limit) => writeln!(f, "A timeout {:?} was reached", limit),
        }
    }
}

impl From<win::Error> for Error {
    fn from(err: win::Error) -> Self {
        Error::Win(err)
    }
}
