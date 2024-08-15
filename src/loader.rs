use std::cell::RefCell;
use std::fmt::{Debug, Display, Formatter};
use std::io::Error;
use std::rc::Rc;
use crate::memory::Memory;

#[derive(Default, Debug)]
pub enum LoaderType {
    #[default]
    INESV1
}

pub trait Loader: Debug  {
    fn load_rom(&mut self, path: &str) -> Result<(), LoaderError>;
    fn set_target_memory(&mut self, memory: Rc<RefCell<dyn Memory>>) {}
}

#[derive(Debug)]
pub enum LoaderError {
    IoError(Error),
    InvalidRomFormat,
    NotConfigured(String)
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
            LoaderError::InvalidRomFormat => { write!(f, "invalid ROM format") }
            LoaderError::NotConfigured(_) => { write!(f, "missing target memory") }
        }
    }
}