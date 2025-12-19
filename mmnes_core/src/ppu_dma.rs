// Authorship: Human 80% | Claude 20%
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use log::info;
use crate::bus::Bus;
use crate::bus_device::{BusDevice, BusDeviceType};
use crate::dma::{Dma, DmaType};
use crate::dma::PpuDmaType::NESPPUDMA;
use crate::dma_device::DmaDevice;
use crate::memory::{Memory, MemoryError};

const PPU_DMA_NAME: &str = "PPU DMA";
const PPU_DMA_ADDRESS_SPACE: (u16, u16) = (0x4014, 0x4014);
const PPU_DMA_SIZE: usize = 1;
/// OAM DMA takes 513 cycles on even CPU cycle start, 514 on odd
const OAM_DMA_CYCLES_EVEN: u32 = 513;
const OAM_DMA_CYCLES_ODD: u32 = 514;

#[derive(Debug)]
pub struct PpuDma {
    device: Rc<RefCell<dyn DmaDevice>>,
    last_transfer_addr: u8,
    bus: Rc<RefCell<dyn Bus>>,
    /// Shared cell to report DMA cycles to halt CPU
    dma_halt_cycles: Rc<Cell<u32>>,
    /// Shared cell tracking CPU odd/even cycle (for DMA alignment)
    cpu_cycle_odd: Rc<Cell<bool>>,
}

impl Dma for PpuDma {
    fn transfer_memory(&mut self, value: u8) -> Result<u16, MemoryError> {
        let source = (value as u16) << 8;
        let last_value = source | 0x00FF;

        //debug!("DMA: transferring 256 bytes of memory from 0x{:04X} to PPU", source);

        // Signal DMA halt cycles based on odd/even CPU cycle
        let halt_cycles = if self.cpu_cycle_odd.get() {
            OAM_DMA_CYCLES_ODD
        } else {
            OAM_DMA_CYCLES_EVEN
        };
        self.dma_halt_cycles.set(halt_cycles);

        let mut index = 0;
        let bus = self.bus.as_ptr();

        /***
         * the unsafe call is necessary because in the current design, this
         * code is called as the CPU already holds a mutable reference to the bus.
         */
        for addr in source..=last_value {
            let data = unsafe { &*bus }.read_byte(addr)?;
            self.device.borrow_mut().dma_write(index as u8, data)?;
            index += 1;
        }

        Ok(index)
    }
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
        info!("initializing PPU DMA");
        Ok(PPU_DMA_SIZE)
    }

    fn read_byte(&self, addr: u16) -> Result<u8, MemoryError> {
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
        self.transfer_memory(value)?;
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

    pub fn new(device: Rc<RefCell<dyn DmaDevice>>, bus: Rc<RefCell<dyn Bus>>) -> Self {
        PpuDma {
            device,
            last_transfer_addr: 0,
            bus,
            dma_halt_cycles: Rc::new(Cell::new(0)),
            cpu_cycle_odd: Rc::new(Cell::new(false)),
        }
    }

    /// Create PpuDma with shared cells for cycle-accurate DMA timing.
    /// - `dma_halt_cycles`: Shared cell where DMA writes the number of cycles to halt CPU
    /// - `cpu_cycle_odd`: Shared cell indicating current CPU cycle parity (for alignment)
    pub fn new_with_cycle_tracking(
        device: Rc<RefCell<dyn DmaDevice>>,
        bus: Rc<RefCell<dyn Bus>>,
        dma_halt_cycles: Rc<Cell<u32>>,
        cpu_cycle_odd: Rc<Cell<bool>>,
    ) -> Self {
        PpuDma {
            device,
            last_transfer_addr: 0,
            bus,
            dma_halt_cycles,
            cpu_cycle_odd,
        }
    }

    /// Get the shared cell for DMA halt cycles (for external monitoring)
    pub fn get_dma_halt_cycles_cell(&self) -> Rc<Cell<u32>> {
        self.dma_halt_cycles.clone()
    }

    /// Get the shared cell for CPU cycle parity (for external updating)
    pub fn get_cpu_cycle_odd_cell(&self) -> Rc<Cell<bool>> {
        self.cpu_cycle_odd.clone()
    }
}

