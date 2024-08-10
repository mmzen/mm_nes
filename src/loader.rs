use std::fmt::{Debug, Display, Formatter};
use std::io::Error;

pub trait Loader: Debug  {
    fn load_rom(&mut self, path: &str) -> Result<(), LoaderError>;
}

#[derive(Debug)]
pub enum LoaderError {
    IoError(Error),
    InvalidRomFormat,
}

impl From<Error> for LoaderError {
    fn from(error: Error) -> Self {
        LoaderError::IoError(error)
    }
}

impl Display for LoaderError {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            LoaderError::IoError(e) => { write!(f, "i/o error {}", e) },
            LoaderError::InvalidRomFormat => { write!(f, "invalid ROM format")  }
        }
    }
}