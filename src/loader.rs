use std::cell::RefCell;
use std::fmt::{Debug, Display, Formatter};
use std::io::Error;
use std::path::PathBuf;
use std::rc::Rc;
use crate::cartridge::Cartridge;
use crate::memory::MemoryError;

#[derive(Default, Debug, Clone)]
pub enum LoaderType {
    #[default]
    INESV1
}

pub trait Loader: Debug  {
    fn load(&mut self, path: &PathBuf) -> Result<Rc<RefCell<dyn Cartridge>>, LoaderError>;
}

#[derive(Debug)]
pub enum LoaderError {
    IoError(Error),
    InvalidRomFormat,
    MemoryError(MemoryError)
}

impl From<Error> for LoaderError {
    fn from(error: Error) -> Self {
        LoaderError::IoError(error)
    }
}

impl From<MemoryError> for LoaderError {
    fn from(error: MemoryError) -> Self {
        LoaderError::MemoryError(error)
    }
}


impl Display for LoaderError {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            LoaderError::IoError(e) => { write!(f, "i/o error {}", e) },
            LoaderError::InvalidRomFormat => { write!(f, "invalid ROM format") },
            LoaderError::MemoryError(e) => { write!(f, "memory error: {}", e) }
        }
    }
}