// Authorship: Human 90% | Claude 10%
use std::cell::{Cell, RefCell};
use std::cmp::PartialEq;
use std::fmt::{Debug};
use std::rc::Rc;
use log::debug;
use crate::bus_device::{BusDevice, BusDeviceType};
use crate::bus_device::BusDeviceType::CONTROLLER;
use crate::controller::{Controller, ControllerError, ControllerType};
use crate::input::Input;
use crate::key_event::KeyEvents;
use crate::memory::{Memory, MemoryError};

const DEVICE_NAME: &str = "Standard Controller";
const CONTROLLER_ADDRESS_SPACE: (u16, u16) = (0x4016, 0x4016);
const CONTROLLER_MEMORY_SIZE: usize = 1;
const CONTROLLER_NUM_BUTTONS: usize = 8;
const DEFAULT_STATE: u8 = 0x01;

#[derive(Debug, PartialEq)]
enum State {
    Idle,
    Polling,
    StateReady
}

#[derive(Debug)]
pub struct StandardController<T: Input> {
    input: T,
    state: RefCell<State>,
    control_states: [u8; CONTROLLER_NUM_BUTTONS],
    control_index: RefCell<usize>,
    /// Shared data bus for open bus behavior (upper 3 bits of read)
    data_bus: Rc<Cell<u8>>,
}

impl<T: Input> Controller for StandardController<T> {
    fn set_input(&mut self, input: KeyEvents) -> Result<(), ControllerError> {
        self.input.set_input_state(input);
        Ok(())
    }
}

impl<T: Input> Memory for StandardController<T> {
    fn initialize(&mut self) -> Result<usize, MemoryError> {
        debug!("initializing controller at 0x{:04X}", CONTROLLER_ADDRESS_SPACE.0);
        Ok(CONTROLLER_MEMORY_SIZE)
    }

    fn read_byte(&self, _: u16) -> Result<u8, MemoryError> {
        // NES controller read returns:
        // - Bit 0: Controller serial data (button state)
        // - Bits 1-4: Usually 0 for standard controller
        // - Bits 5-7: Open bus (from data bus)

        let control_state = if *self.state.borrow() == State::StateReady {
            let index = *self.control_index.borrow();

            if index == CONTROLLER_NUM_BUTTONS - 1 {
                *self.state.borrow_mut() = State::Idle;
            } else {
                *self.control_index.borrow_mut() = index + 1;
            }

            self.control_states[index]
        } else {
            DEFAULT_STATE
        };

        // Combine controller bit 0 with open bus bits 5-7
        let open_bus_bits = self.data_bus.get() & 0xE0;  // Upper 3 bits from open bus
        let result = (control_state & 0x1F) | open_bus_bits;

        Ok(result)
    }

    fn trace_read_byte(&self, _: u16) -> Result<u8, MemoryError> {
        Ok(0)
    }

    fn write_byte(&mut self, _: u16, value: u8) -> Result<(), MemoryError> {
        let state = value & 0x01;

        let result = match state {
            0x00 => {
                if *self.state.borrow() == State::Polling {
                    self.input.get_input_state(&mut self.control_states);
                    *self.control_index.borrow_mut() = 0;

                    *self.state.borrow_mut() = State::StateReady;
                    //trace!("controller input: {:?}", self.control_states);
                }
            },

            0x01 => *self.state.borrow_mut() = State::Polling,
            _ => unreachable!(),
        };

        //trace!("controller state: {:?}", self.state);
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

impl<T: Input> BusDevice for StandardController<T> {
    fn get_name(&self) -> String {
        DEVICE_NAME.to_string()
    }

    fn get_device_type(&self) -> BusDeviceType {
        CONTROLLER(ControllerType::StandardController)
    }

    fn get_virtual_address_range(&self) -> (u16, u16) {
        CONTROLLER_ADDRESS_SPACE
    }
}

impl<T: Input> StandardController<T> {

    pub fn new(input: T, data_bus: Rc<Cell<u8>>) -> StandardController<T> {
        StandardController {
            input,
            state: RefCell::new(State::Idle),
            control_states: [0; CONTROLLER_NUM_BUTTONS],
            control_index: RefCell::new(0),
            data_bus,
        }
    }
}