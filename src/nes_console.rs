use std::fmt::{Display, Formatter};
use crate::cpu::CPU;
use crate::memory::Memory;

struct NESConsole {
    bus: Box<dyn Memory>,
    wram: Box<dyn Memory>,
    vram: Box<dyn Memory>,
    cpu: Box<dyn CPU>
}

impl NESConsole {
    fn power_on(&mut self) -> Result<(), NESConsoleError> {
        Ok(())
    }
}

enum NESConsoleError {
    BuilderError(String)
}

impl Display for NESConsoleError {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            NESConsoleError::BuilderError(s) => { write!(f, "builder error: {}", s) }
        }
    }
}

struct NESConsoleBuilder {
    bus: Option<Box<dyn Memory>>,
    wram: Option<Box<dyn Memory>>,
    vram: Option<Box<dyn Memory>>,
    cpu: Option<Box<dyn CPU>>
}

impl NESConsoleBuilder {
    fn new() -> Self {
        NESConsoleBuilder {
            bus: None,
            wram: None,
            vram: None,
            cpu: None
        }
    }

    fn with_bus(mut self, bus: Box<dyn Memory>) -> Self {
        self.bus = Some(bus);
        self
    }

    fn with_wram(mut self, wram: Box<dyn Memory>) -> Self {
        self.wram = Some(wram);
        self
    }

    fn with_vram(mut self, vram: Box<dyn Memory>) -> Self {
        self.vram = Some(vram);
        self
    }

    fn with_cpu(mut self, cpu: Box<dyn CPU>) -> Self {
        self.cpu = Some(cpu);
        self
    }

    fn build(self) -> Result<NESConsole, NESConsoleError> {
        if let (Some(bus), Some(wram), Some(vram), Some(cpu)) = (self.bus, self.wram, self.vram, self.cpu) {
            Ok(NESConsole { bus, wram, vram, cpu })
        } else {
            Err(NESConsoleError::BuilderError("missing required components".to_string()))
        }
    }
}