use std::fmt::{Debug, Formatter};
use sdl2::event::Event;
use sdl2::{EventPump, Sdl};
use crate::input::{Input, InputError};

const A: usize = 0x00;
const B: usize = 0x01;
const SELECT: usize = 0x02;
const START: usize = 0x03;
const UP: usize = 0x04;
const DOWN: usize = 0x05;
const LEFT: usize = 0x06;
const RIGHT: usize = 0x07;

const RELEASED: u8 = 0x00;
const PRESSED: u8 = 0x01;

pub struct InputSDL2 {
    event_pump: EventPump,
}

impl Input for InputSDL2 {
    fn get_input_state(&mut self, control_states: &mut [u8; 8]) {
        for event in self.event_pump.poll_iter() {
            match event {
                Event::KeyDown { keycode: Some(keycode), .. } => {
                    match keycode {
                        sdl2::keyboard::Keycode::A => control_states[A] = 1,
                        sdl2::keyboard::Keycode::Z => control_states[B] = 1,
                        sdl2::keyboard::Keycode::RETURN => control_states[START] = 1,
                        sdl2::keyboard::Keycode::ESCAPE => control_states[SELECT] = 1,
                        sdl2::keyboard::Keycode::UP => control_states[UP] = 1,
                        sdl2::keyboard::Keycode::DOWN => control_states[DOWN] = 1,
                        sdl2::keyboard::Keycode::LEFT => control_states[LEFT] = 1,
                        sdl2::keyboard::Keycode::RIGHT => control_states[RIGHT] = 1,
                        _ => {}
                    }
                },

                Event::KeyUp { keycode: Some(keycode), .. } => {
                    match keycode {
                        sdl2::keyboard::Keycode::A => control_states[A] = 0,
                        sdl2::keyboard::Keycode::Z => control_states[B] = 0,
                        sdl2::keyboard::Keycode::RETURN => control_states[START] = 0,
                        sdl2::keyboard::Keycode::ESCAPE => control_states[SELECT] = 0,
                        sdl2::keyboard::Keycode::UP => control_states[UP] = 0,
                        sdl2::keyboard::Keycode::DOWN => control_states[DOWN] = 0,
                        sdl2::keyboard::Keycode::LEFT => control_states[LEFT] = 0,
                        sdl2::keyboard::Keycode::RIGHT => control_states[RIGHT] = 0,
                        _ => {}
                    }
                },
                _ => {}
            }
        }
    }
}

impl Debug for InputSDL2 {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

impl InputSDL2 {
    pub fn new(sdl_context: &Sdl) -> Result<Self, InputError> {
        let event_pump = sdl_context.event_pump()?;

        let input = InputSDL2 {
            event_pump,
        };

        Ok(input)
    }
}