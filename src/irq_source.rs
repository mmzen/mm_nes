use std::fmt;
use std::fmt::{Display, Formatter};
use crate::cpu::{CpuError, Interruptible};

#[allow(dead_code)]
#[derive(Debug, PartialEq)]
pub enum IrqError {
    IrqAssertion(String),
    IrqDeassertion(String)
}

impl From<CpuError> for IrqError {
    fn from(error: CpuError) -> Self {
        IrqError::IrqAssertion(format!("unable to assert IRQ: {}", error))
    }
}

impl Display for IrqError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            IrqError::IrqAssertion(s) => write!(f, "{}", s),
            IrqError::IrqDeassertion(s) => write!(f, "{}", s),
        }
    }
}

pub trait IrqSource<T: Interruptible + ?Sized> {
    fn cpu(&self) -> *mut T;

    fn irq_mask(&self) -> u8;

    fn interrupt(&self) -> Result<(), IrqError> {
        /***
         * unsafe code is needed as clear_interrupt can be called
         * from a register write method, directly calling by the CPU,
         * and already having a mutable reference to itself.
         ***/
        let cpu = self.cpu();
        unsafe { &mut *cpu }.signal_irq(self.irq_mask())?;
        Ok(())
    }

    fn clear_interrupt(&self) -> Result<(), IrqError> {
        /***
         * unsafe code is needed as clear_interrupt can be called
         * from a register write method, directly calling by the CPU,
         * and already having a mutable reference to itself.
         ***/
        let cpu = self.cpu();
        unsafe { &mut *cpu }.clear_irq(self.irq_mask())?;
        Ok(())
    }

    fn is_asserted_irq(&self) -> Result<bool, IrqError> {
        let cpu = self.cpu();
        let irq_line_asserted = unsafe { &mut *cpu }.is_asserted_irq_by_source(self.irq_mask())?;
        Ok(irq_line_asserted)
    }
}