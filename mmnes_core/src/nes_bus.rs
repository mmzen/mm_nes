// Authorship: Human 85% | Claude 15%
use std::cell::{Cell, RefCell};
use std::fmt::Debug;
use std::rc::Rc;
use log::{debug, trace};
use crate::bus::{Bus, BusError};
use crate::bus_device::{BusDevice, BusDeviceType};
use crate::memory::{Memory, MemoryError};

pub const BUS_ADDRESSABLE_SIZE: usize = 64 * 1024;

#[derive(Debug)]
pub struct NESBus {
    devices: Vec<Rc<RefCell<dyn BusDevice>>>,
    num_devices: usize,
    data_bus: Rc<Cell<u8>>,  // Shared data bus value for open bus behavior
}

impl Memory for NESBus {
    fn initialize(&mut self) -> Result<usize, MemoryError> {
        Ok(BUS_ADDRESSABLE_SIZE)
    }

    fn read_byte(&self, addr: u16) -> Result<u8, MemoryError> {
        let (memory, effective_addr) = self.lookup_address(addr)?;
        let value = memory.borrow().read_byte(effective_addr)?;

        // $4015 (APU status) is special - reading it should NOT update the data bus
        if addr != 0x4015 {
            self.data_bus.set(value);  // Update data bus with read value
        }
        Ok(value)
    }

    /// Peek at memory without updating data bus (for debugging/disassembly only).
    fn peek_byte(&self, addr: u16) -> Result<u8, MemoryError> {
        let (memory, effective_addr) = self.lookup_address(addr)?;
        let value = memory.borrow().peek_byte(effective_addr)?;

        Ok(value)
    }

    fn write_byte(&mut self, addr: u16, value: u8) -> Result<(), MemoryError> {
        self.data_bus.set(value);  // Update data bus with written value

        let (memory, effective_addr) = self.lookup_address(addr)?;
        memory.borrow_mut().write_byte(effective_addr, value)?;

        Ok(())
    }

    fn read_word(&self, addr: u16) -> Result<u16, MemoryError> {
        // Read as two bytes to properly update data bus (high byte will be last on bus)
        let low = self.read_byte(addr)?;
        let high = self.read_byte(addr.wrapping_add(1))?;
        Ok(u16::from_le_bytes([low, high]))
    }

    fn write_word(&mut self, addr: u16, value: u16) -> Result<(), MemoryError> {
        // Write as two bytes to properly update data bus
        let bytes = value.to_le_bytes();
        self.write_byte(addr, bytes[0])?;
        self.write_byte(addr.wrapping_add(1), bytes[1])?;
        Ok(())
    }

    fn dump(&self) {
        todo!()
    }

    fn size(&self) -> usize {
        BUS_ADDRESSABLE_SIZE
    }
}

impl Bus for NESBus {


    fn add_device(&mut self, device: Rc<RefCell<dyn BusDevice>>) -> Result<(), BusError> {
        let size = device.borrow().size();
        let address_space = device.borrow().get_virtual_address_range();

        debug!("BUS: adding device {} - size: {} bytes, address range: 0x{:04X} - 0x{:04X}",
        device.borrow().get_name(), size, address_space.0, address_space.1);

        for addr in address_space.0..=address_space.1 {
            if self.devices[addr as usize].borrow().get_device_type() != BusDeviceType::OPENBUS {
                debug!("BUS: address 0x{:04X} already mapped by device {}, overwriting by {} ...",
                         addr, self.devices[addr as usize].borrow().get_name(), device.borrow().get_name());
            }

            self.devices[addr as usize] = device.clone();
        }

        let count = self.count_addresses_in_bus();
        self.num_devices += 1;

        debug!("BUS: {} addresses mapped in bus, {} devices", count, self.num_devices);

        Ok(())
    }
}

impl NESBus {

    pub fn new() -> Self {
        let data_bus = Rc::new(Cell::new(0u8));
        let open_bus = Rc::new(RefCell::new(OpenBus::new(data_bus.clone())));

        NESBus {
            devices: vec![open_bus.clone(); 65536],
            num_devices: 0,
            data_bus,
        }
    }

    /// Get a reference to the shared data bus for open bus behavior
    pub fn get_data_bus(&self) -> Rc<Cell<u8>> {
        self.data_bus.clone()
    }

    #[allow(dead_code)]
    fn is_addr_in_boundary(&self, addr: u16) -> bool {
        addr < BUS_ADDRESSABLE_SIZE as u16
    }

    fn lookup_address(&self, addr: u16) -> Result<(Rc<RefCell<dyn BusDevice>>, u16), BusError> {
        let device = self.devices[addr as usize].clone();
        let size = device.borrow().size();

        // INVARIANT: Device size must be a power of two for correct address masking.
        // A non-power-of-two size will produce incorrect effective addresses via the
        // `addr & (size - 1)` mask. If this fires, the device's size() is misconfigured.
        debug_assert!(
            size.is_power_of_two(),
            "BUS: device {} has non-power-of-two size {} - address masking will be incorrect!",
            device.borrow().get_name(),
            size
        );

        let effective_addr = addr & (size - 1) as u16;

        trace!("BUS: translated address 0x{:04X} to device {} ({}, 0x{:04X} - 0x{:04X}), effective address 0x{:04X}",
                    addr, device.borrow().get_name(), device.borrow().get_device_type(),
                    device.borrow().get_virtual_address_range().0, device.borrow().get_virtual_address_range().1,
                    effective_addr);

        Ok((device, effective_addr))
    }

    fn count_addresses_in_bus(&self) -> usize {
        self.devices
            .iter()
            .filter(|d| {
                d.borrow().get_device_type() != BusDeviceType::OPENBUS
            })
            .count()
    }
}

const OPEN_BUS_DEVICE_NAME: &str = "Open Bus";

struct OpenBus {
    data_bus: Rc<Cell<u8>>,  // Shared reference to system data bus
}

impl std::fmt::Debug for OpenBus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenBus")
            .field("data_bus", &self.data_bus.get())
            .finish()
    }
}

impl OpenBus {
    fn new(data_bus: Rc<Cell<u8>>) -> Self {
        OpenBus { data_bus }
    }
}

impl BusDevice for OpenBus {
    fn get_name(&self) -> String {
        OPEN_BUS_DEVICE_NAME.to_string()
    }

    fn get_device_type(&self) -> BusDeviceType {
        BusDeviceType::OPENBUS
    }

    #[allow(arithmetic_overflow)]
    fn get_virtual_address_range(&self) -> (u16, u16) {
        (0x000, 0x0000 + (BUS_ADDRESSABLE_SIZE - 1) as u16)
    }
}

impl Memory for OpenBus {
    fn initialize(&mut self) -> Result<usize, MemoryError> {
        Ok(0)
    }

    fn read_byte(&self, _: u16) -> Result<u8, MemoryError> {
        Ok(self.data_bus.get())  // Return current data bus value
    }

    fn peek_byte(&self, _: u16) -> Result<u8, MemoryError> {
        Ok(self.data_bus.get())
    }

    fn write_byte(&mut self, _: u16, _: u8) -> Result<(), MemoryError> {
        Ok(())
    }

    fn read_word(&self, _: u16) -> Result<u16, MemoryError> {
        Ok(0x0000)
    }

    fn write_word(&mut self, _: u16, _: u16) -> Result<(), MemoryError> {
        Ok(())
    }

    fn dump(&self) {
    }

    fn size(&self) -> usize {
        1
    }
}
