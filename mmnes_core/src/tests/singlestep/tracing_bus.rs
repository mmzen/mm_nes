// Authorship: Human 0% | Claude 100%
//! TracingBus - A bus implementation that records all bus activity for testing
//!
//! This bus provides a simple 64KB memory space and records every read/write
//! operation for cycle-accurate validation against SingleStepTests.

use std::cell::RefCell;
use std::rc::Rc;
use crate::bus::{Bus, BusError};
use crate::bus_device::BusDevice;
use crate::memory::{Memory, MemoryError};
use super::{BusCycle, BusOperation};

/// A bus that traces all memory operations for test validation
#[derive(Debug)]
pub struct TracingBus {
    /// Full 64KB address space
    memory: Box<[u8; 65536]>,
    /// Recorded bus operations
    trace: RefCell<Vec<BusCycle>>,
}

impl TracingBus {
    /// Create a new TracingBus with zeroed memory
    pub fn new() -> Self {
        TracingBus {
            memory: Box::new([0u8; 65536]),
            trace: RefCell::new(Vec::new()),
        }
    }

    /// Initialize memory from a list of (address, value) pairs
    pub fn load_memory(&mut self, data: &[(u16, u8)]) {
        for &(addr, value) in data {
            self.memory[addr as usize] = value;
        }
    }

    /// Clear the trace buffer
    pub fn clear_trace(&self) {
        self.trace.borrow_mut().clear();
    }

    /// Get a copy of the current trace
    pub fn get_trace(&self) -> Vec<BusCycle> {
        self.trace.borrow().clone()
    }

    /// Read a byte without tracing (for test assertions)
    pub fn peek(&self, addr: u16) -> u8 {
        self.memory[addr as usize]
    }
}

impl Default for TracingBus {
    fn default() -> Self {
        Self::new()
    }
}

impl Memory for TracingBus {
    fn initialize(&mut self) -> Result<usize, MemoryError> {
        Ok(65536)
    }

    fn read_byte(&self, addr: u16) -> Result<u8, MemoryError> {
        let value = self.memory[addr as usize];
        self.trace.borrow_mut().push(BusCycle {
            address: addr,
            value,
            operation: BusOperation::Read,
        });
        Ok(value)
    }

    fn trace_read_byte(&self, addr: u16) -> Result<u8, MemoryError> {
        // Non-tracing read for debugging purposes
        Ok(self.memory[addr as usize])
    }

    fn write_byte(&mut self, addr: u16, value: u8) -> Result<(), MemoryError> {
        self.trace.borrow_mut().push(BusCycle {
            address: addr,
            value,
            operation: BusOperation::Write,
        });
        self.memory[addr as usize] = value;
        Ok(())
    }

    fn read_word(&self, addr: u16) -> Result<u16, MemoryError> {
        // Read low byte then high byte (6502 is little-endian)
        let lo = self.read_byte(addr)?;
        let hi = self.read_byte(addr.wrapping_add(1))?;
        Ok(u16::from_le_bytes([lo, hi]))
    }

    fn write_word(&mut self, addr: u16, value: u16) -> Result<(), MemoryError> {
        let bytes = value.to_le_bytes();
        self.write_byte(addr, bytes[0])?;
        self.write_byte(addr.wrapping_add(1), bytes[1])?;
        Ok(())
    }

    fn dump(&self) {
        // Not implemented for tests
    }

    fn size(&self) -> usize {
        65536
    }
}

impl Bus for TracingBus {
    fn add_device(&mut self, _device: Rc<RefCell<dyn BusDevice>>) -> Result<(), BusError> {
        // TracingBus doesn't support devices - it's a simple flat memory
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracing_bus_records_reads() {
        let bus = TracingBus::new();

        let _ = bus.read_byte(0x1234);
        let _ = bus.read_byte(0x5678);

        let trace = bus.get_trace();
        assert_eq!(trace.len(), 2);
        assert_eq!(trace[0].address, 0x1234);
        assert_eq!(trace[0].operation, BusOperation::Read);
        assert_eq!(trace[1].address, 0x5678);
    }

    #[test]
    fn tracing_bus_records_writes() {
        let mut bus = TracingBus::new();

        let _ = bus.write_byte(0x1234, 0xAB);
        let _ = bus.write_byte(0x5678, 0xCD);

        let trace = bus.get_trace();
        assert_eq!(trace.len(), 2);
        assert_eq!(trace[0].address, 0x1234);
        assert_eq!(trace[0].value, 0xAB);
        assert_eq!(trace[0].operation, BusOperation::Write);
        assert_eq!(trace[1].address, 0x5678);
        assert_eq!(trace[1].value, 0xCD);
    }

    #[test]
    fn tracing_bus_load_memory() {
        let mut bus = TracingBus::new();
        bus.load_memory(&[(0x100, 0x42), (0x200, 0x55)]);

        assert_eq!(bus.peek(0x100), 0x42);
        assert_eq!(bus.peek(0x200), 0x55);
    }

    #[test]
    fn tracing_bus_clear_trace() {
        let bus = TracingBus::new();

        let _ = bus.read_byte(0x1234);
        assert_eq!(bus.get_trace().len(), 1);

        bus.clear_trace();
        assert_eq!(bus.get_trace().len(), 0);
    }

    #[test]
    fn peek_does_not_trace() {
        let mut bus = TracingBus::new();
        bus.load_memory(&[(0x100, 0x42)]);

        let value = bus.peek(0x100);
        assert_eq!(value, 0x42);
        assert_eq!(bus.get_trace().len(), 0);
    }
}
