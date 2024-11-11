use std::fmt::{Debug};
use log::{debug, trace};
use crate::apu::ApuType::RP2A03;
use crate::bus_device::{BusDevice, BusDeviceType};
use crate::bus_device::BusDeviceType::CONTROLLER;
use crate::controller::{Controller, ControllerType};
use crate::memory::{Memory, MemoryError};

const DEVICE_NAME: &str = "Standard Controller";
const CONTROLLER_ADDRESS_SPACE: (u16, u16) = (0x4016, 0x4016);
const CONTROLLER_MEMORY_SIZE: usize = 1;

#[derive(Debug)]
enum PollState {
    Waiting,
    Polling
}

#[derive(Debug)]
pub struct StandardController {
    poll_state: PollState,
}

impl Controller for StandardController {}

impl Memory for StandardController {
    fn initialize(&mut self) -> Result<usize, MemoryError> {
        debug!("initializing controller at 0x{:04X}", CONTROLLER_ADDRESS_SPACE.0);
        Ok(CONTROLLER_MEMORY_SIZE)
    }

    fn read_byte(&self, addr: u16) -> Result<u8, MemoryError> {
        trace!("controller read byte at 0x{:04X}: not implemented", addr);
        Ok(0)
    }

    fn trace_read_byte(&self, addr: u16) -> Result<u8, MemoryError> {
        Ok(0)
    }

    fn write_byte(&mut self, _: u16, value: u8) -> Result<(), MemoryError> {
        let state = value & 0x01;

        let result = match state {
            0x00 => self.change_poll_state(PollState::Waiting),
            0x01 => self.change_poll_state(PollState::Polling),
            _ => unreachable!(),
        };

        trace!("controller state: {:?}", self.poll_state);

        Ok(result)
    }

    fn read_word(&self, _: u16) -> Result<u16, MemoryError> {
        Ok(0)
    }

    fn write_word(&mut self, _: u16, _: u16) -> Result<(), MemoryError> {
        Ok(())
    }

    fn dump(&self) {
        todo!()
    }

    fn size(&self) -> usize {
        CONTROLLER_MEMORY_SIZE
    }
}

impl BusDevice for StandardController {
    fn get_name(&self) -> String {
        DEVICE_NAME.to_string()
    }

    fn get_device_type(&self) -> BusDeviceType {
        CONTROLLER(ControllerType::StandardController)
    }

    fn get_address_range(&self) -> (u16, u16) {
        CONTROLLER_ADDRESS_SPACE
    }

    fn is_addr_in_address_space(&self, addr: u16) -> bool {
        CONTROLLER_ADDRESS_SPACE.0 <= addr && addr <= CONTROLLER_ADDRESS_SPACE.1
    }
}

impl StandardController {

    pub fn new() -> StandardController {
        StandardController {
            poll_state: PollState::Waiting
        }
    }

    fn change_poll_state(&mut self, state: PollState) {
        self.poll_state = state;
    }
}