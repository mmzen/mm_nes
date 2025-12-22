// Authorship: Human 30% | Claude 70%
//! PPU DMA register ($4014) - signals OAM DMA start to the scheduler.
//!
//! Writing to $4014 initiates an OAM DMA transfer. In cycle-accurate mode,
//! this module just signals the DMA start via a shared cell, and the actual
//! byte-by-byte transfer is handled by the DmaController in the scheduler.
//!
//! # Timing Contract
//!
//! Writing $4014 sets a latch (`dma_start_page`). The scheduler samples this
//! latch at the **start of the next master cycle**, not the current one.
//! This means if the CPU writes $4014 on cycle N, the first DMA halt attempt
//! begins on cycle N+1.
//!
//! # Double-Write Behavior
//!
//! If the CPU writes $4014 multiple times before the scheduler samples the
//! latch, the last write wins. This matches hardware behavior where rapid
//! writes would simply update the source page register.

use std::cell::Cell;
use std::rc::Rc;
use log::debug;
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
    /// Shared cell to signal DMA start (contains source page, None if no DMA pending)
    dma_start_page: Rc<Cell<Option<u8>>>,
    /// Shared data bus for open bus reads
    data_bus: Rc<Cell<u8>>,
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
        debug!("initializing PPU DMA (cycle-accurate mode)");
        Ok(PPU_DMA_SIZE)
    }

    fn read_byte(&self, _addr: u16) -> Result<u8, MemoryError> {
        // $4014 is write-only on real hardware.
        // Reading returns open bus (current data bus value), consistent with APU behavior.
        Ok(self.data_bus.get())
    }

    fn peek_byte(&self, _addr: u16) -> Result<u8, MemoryError> {
        // For peeking, also return open bus (don't expose internal state)
        Ok(self.data_bus.get())
    }

    fn write_byte(&mut self, _: u16, value: u8) -> Result<(), MemoryError> {
        // Signal DMA start to the scheduler via the shared cell.
        // The value is the high byte of the source address (page).
        //
        // Timing: This sets the latch. The scheduler will sample it at the
        // start of the NEXT master cycle, initiating DMA on cycle N+1.
        self.dma_start_page.set(Some(value));

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
    /// Create PpuDma with shared cells for DMA signaling and open bus.
    ///
    /// # Arguments
    /// * `dma_start_page` - Shared cell to signal DMA start (set to Some(page) on write)
    /// * `data_bus` - Shared data bus for open bus reads
    ///
    /// When write_byte is called, it sets `dma_start_page` to Some(page).
    /// The scheduler samples this at the start of the next master cycle.
    pub fn new_with_dma_signal(dma_start_page: Rc<Cell<Option<u8>>>, data_bus: Rc<Cell<u8>>) -> Self {
        PpuDma {
            dma_start_page,
            data_bus,
        }
    }
}
