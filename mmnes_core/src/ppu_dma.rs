// Authorship: Human 40% | Claude 60%
//! PPU DMA register ($4014) - signals OAM DMA start to the scheduler.
//!
//! Writing to $4014 initiates an OAM DMA transfer. In cycle-accurate mode,
//! this module just signals the DMA start via a shared cell, and the actual
//! byte-by-byte transfer is handled by the DmaController in the scheduler.

use std::cell::Cell;
use std::rc::Rc;
use log::info;
use crate::bus_device::{BusDevice, BusDeviceType};
use crate::dma::DmaType;
use crate::dma::PpuDmaType::NESPPUDMA;
use crate::memory::{Memory, MemoryError};

const PPU_DMA_NAME: &str = "PPU DMA";
const PPU_DMA_ADDRESS_SPACE: (u16, u16) = (0x4014, 0x4014);
const PPU_DMA_SIZE: usize = 1;

/// PPU DMA register - signals OAM DMA transfers.
///
/// In cycle-accurate mode, this just sets a shared cell with the source page,
/// and the DmaController handles the actual transfer cycle-by-cycle.
#[derive(Debug)]
pub struct PpuDma {
    /// Last written value (source page address)
    last_transfer_addr: u8,
    /// Shared cell to signal DMA start (contains source page, None if no DMA pending)
    dma_start_page: Rc<Cell<Option<u8>>>,
}

impl BusDevice for PpuDma {
    fn get_name(&self) -> String {
        PPU_DMA_NAME.to_string()
    }

    fn get_device_type(&self) -> BusDeviceType {
        BusDeviceType::DMA(DmaType::PpuDma(NESPPUDMA))
    }

    fn get_virtual_address_range(&self) -> (u16, u16) {
        PPU_DMA_ADDRESS_SPACE
    }
}

impl Memory for PpuDma {
    fn initialize(&mut self) -> Result<usize, MemoryError> {
        info!("initializing PPU DMA (cycle-accurate mode)");
        Ok(PPU_DMA_SIZE)
    }

    fn read_byte(&self, addr: u16) -> Result<u8, MemoryError> {
        // $4014 is write-only, but reading returns the last written value
        // (open bus behavior is handled at the bus level)
        let value = match addr {
            0x00 => self.last_transfer_addr,
            _ => unreachable!()
        };

        Ok(value)
    }

    fn trace_read_byte(&self, addr: u16) -> Result<u8, MemoryError> {
        self.read_byte(addr)
    }

    fn write_byte(&mut self, _: u16, value: u8) -> Result<(), MemoryError> {
        // Signal DMA start to the scheduler via the shared cell
        // The value is the high byte of the source address (page)
        self.dma_start_page.set(Some(value));
        self.last_transfer_addr = value;

        Ok(())
    }

    fn read_word(&self, _: u16) -> Result<u16, MemoryError> {
        unimplemented!()
    }

    fn write_word(&mut self, _: u16, _: u16) -> Result<(), MemoryError> {
        unimplemented!()
    }

    fn dump(&self) {
        unimplemented!()
    }

    fn size(&self) -> usize {
        PPU_DMA_SIZE
    }
}

impl PpuDma {
    /// Create PpuDma with a shared cell for signaling DMA start.
    ///
    /// When write_byte is called, it sets the shared cell to Some(page),
    /// which the scheduler's step_master_cycle() checks to start DMA.
    pub fn new_with_dma_signal(dma_start_page: Rc<Cell<Option<u8>>>) -> Self {
        PpuDma {
            last_transfer_addr: 0,
            dma_start_page,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dma_signal_on_write() {
        let dma_signal = Rc::new(Cell::new(None));
        let mut ppu_dma = PpuDma::new_with_dma_signal(dma_signal.clone());

        ppu_dma.initialize().unwrap();

        // Initially no DMA signaled
        assert_eq!(dma_signal.get(), None);

        // Write to $4014 signals DMA with source page
        ppu_dma.write_byte(0x00, 0x02).unwrap();
        assert_eq!(dma_signal.get(), Some(0x02));

        // Reading returns last written value
        assert_eq!(ppu_dma.read_byte(0x00).unwrap(), 0x02);
    }
}
