// Authorship: Human 0% | Claude 100%
//! Tests for StandardController
//!
//! Validates controller state machine, button reading, and open bus behavior.

use std::cell::Cell;
use std::rc::Rc;
use crate::input::Input;
use crate::key_event::KeyEvents;
use crate::memory::Memory;
use crate::standard_controller::StandardController;
use crate::tests::init;

/// Mock Input that returns configurable button states
#[derive(Debug)]
struct MockInput {
    button_states: [u8; 8],
}

impl MockInput {
    fn new() -> Self {
        MockInput {
            button_states: [0; 8],
        }
    }

    fn set_button(&mut self, button: usize, pressed: bool) {
        if button < 8 {
            self.button_states[button] = if pressed { 1 } else { 0 };
        }
    }

    fn set_all_buttons(&mut self, states: [u8; 8]) {
        self.button_states = states;
    }
}

impl Input for MockInput {
    fn get_input_state(&mut self, control_states: &mut [u8; 8]) {
        *control_states = self.button_states;
    }

    fn set_input_state(&mut self, _key_events: KeyEvents) {
        // Not needed for tests
    }
}

fn create_controller() -> StandardController<MockInput> {
    let data_bus = Rc::new(Cell::new(0u8));
    StandardController::new(MockInput::new(), data_bus)
}

fn create_controller_with_data_bus(data_bus: Rc<Cell<u8>>) -> StandardController<MockInput> {
    StandardController::new(MockInput::new(), data_bus)
}

// ============================================================================
// Initialization tests
// ============================================================================

#[test]
fn controller_initializes_successfully() {
    init();
    let mut controller = create_controller();
    let result = controller.initialize();
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 1); // CONTROLLER_MEMORY_SIZE
}

#[test]
fn controller_size_is_one_byte() {
    init();
    let controller = create_controller();
    assert_eq!(controller.size(), 1);
}

// ============================================================================
// State machine tests
// ============================================================================

#[test]
fn reading_without_strobe_returns_default_state() {
    init();
    let controller = create_controller();

    // Without strobe, should return 0x01 (DEFAULT_STATE)
    let result = controller.read_byte(0x4016).unwrap();
    assert_eq!(result & 0x1F, 0x01);
}

#[test]
fn strobe_high_then_low_enables_button_reading() {
    init();
    let mut controller = create_controller();

    // Set button states: A pressed, B not pressed
    controller.input.set_button(0, true);  // A
    controller.input.set_button(1, false); // B

    // Strobe high (0x01) - enter Polling state
    controller.write_byte(0x4016, 0x01).unwrap();

    // Strobe low (0x00) - latch buttons, enter StateReady
    controller.write_byte(0x4016, 0x00).unwrap();

    // First read should return A button state (pressed = 1)
    let result = controller.read_byte(0x4016).unwrap();
    assert_eq!(result & 0x01, 1);

    // Second read should return B button state (not pressed = 0)
    let result = controller.read_byte(0x4016).unwrap();
    assert_eq!(result & 0x01, 0);
}

#[test]
fn all_eight_buttons_can_be_read_in_sequence() {
    init();
    let mut controller = create_controller();

    // Set all button states: A=1, B=0, Select=1, Start=0, Up=1, Down=0, Left=1, Right=0
    controller.input.set_all_buttons([1, 0, 1, 0, 1, 0, 1, 0]);

    // Strobe sequence
    controller.write_byte(0x4016, 0x01).unwrap();
    controller.write_byte(0x4016, 0x00).unwrap();

    // Read all 8 buttons
    let expected = [1, 0, 1, 0, 1, 0, 1, 0];
    for (i, &expected_state) in expected.iter().enumerate() {
        let result = controller.read_byte(0x4016).unwrap();
        assert_eq!(result & 0x01, expected_state, "Button {} mismatch", i);
    }
}

#[test]
fn after_reading_all_buttons_returns_default_state() {
    init();
    let mut controller = create_controller();

    // Strobe
    controller.write_byte(0x4016, 0x01).unwrap();
    controller.write_byte(0x4016, 0x00).unwrap();

    // Read all 8 buttons
    for _ in 0..8 {
        let _ = controller.read_byte(0x4016).unwrap();
    }

    // After reading all buttons, state should return to Idle
    // Reading now should return DEFAULT_STATE (0x01)
    let result = controller.read_byte(0x4016).unwrap();
    assert_eq!(result & 0x1F, 0x01);
}

#[test]
fn strobe_can_be_repeated() {
    init();
    let mut controller = create_controller();

    // First strobe sequence
    controller.input.set_button(0, true);
    controller.write_byte(0x4016, 0x01).unwrap();
    controller.write_byte(0x4016, 0x00).unwrap();

    let result = controller.read_byte(0x4016).unwrap();
    assert_eq!(result & 0x01, 1);

    // Second strobe sequence - change button state
    controller.input.set_button(0, false);
    controller.write_byte(0x4016, 0x01).unwrap();
    controller.write_byte(0x4016, 0x00).unwrap();

    let result = controller.read_byte(0x4016).unwrap();
    assert_eq!(result & 0x01, 0);
}

// ============================================================================
// Open bus tests
// ============================================================================

#[test]
fn open_bus_bits_5_7_are_combined_with_button_data() {
    init();
    let data_bus = Rc::new(Cell::new(0xE0u8)); // Upper 3 bits set
    let mut controller = create_controller_with_data_bus(data_bus.clone());

    // Strobe
    controller.write_byte(0x4016, 0x01).unwrap();
    controller.write_byte(0x4016, 0x00).unwrap();

    // Read should combine button state (lower 5 bits) with open bus (upper 3 bits)
    let result = controller.read_byte(0x4016).unwrap();

    // Upper 3 bits should be from data_bus (0xE0)
    assert_eq!(result & 0xE0, 0xE0);
}

#[test]
fn open_bus_changes_are_reflected_in_reads() {
    init();
    let data_bus = Rc::new(Cell::new(0x00u8));
    let mut controller = create_controller_with_data_bus(data_bus.clone());

    // Strobe
    controller.write_byte(0x4016, 0x01).unwrap();
    controller.write_byte(0x4016, 0x00).unwrap();

    // First read with data_bus = 0x00
    let result1 = controller.read_byte(0x4016).unwrap();
    assert_eq!(result1 & 0xE0, 0x00);

    // Change data bus
    data_bus.set(0xA0);

    // Second read should reflect new data_bus value
    let result2 = controller.read_byte(0x4016).unwrap();
    assert_eq!(result2 & 0xE0, 0xA0);
}

// ============================================================================
// Edge cases
// ============================================================================

#[test]
fn write_with_only_bit_0_matters() {
    init();
    let mut controller = create_controller();

    controller.input.set_button(0, true);

    // Write 0xFF - only bit 0 should be used (strobe high)
    controller.write_byte(0x4016, 0xFF).unwrap();

    // Write 0xFE - only bit 0 should be used (strobe low)
    controller.write_byte(0x4016, 0xFE).unwrap();

    // Should have latched button state
    let result = controller.read_byte(0x4016).unwrap();
    assert_eq!(result & 0x01, 1);
}

#[test]
fn multiple_strobe_highs_dont_affect_state() {
    init();
    let mut controller = create_controller();

    controller.input.set_button(0, true);

    // Multiple strobe highs
    controller.write_byte(0x4016, 0x01).unwrap();
    controller.write_byte(0x4016, 0x01).unwrap();
    controller.write_byte(0x4016, 0x01).unwrap();

    // Single strobe low
    controller.write_byte(0x4016, 0x00).unwrap();

    // Should still work normally
    let result = controller.read_byte(0x4016).unwrap();
    assert_eq!(result & 0x01, 1);
}

#[test]
fn strobe_low_without_high_does_nothing() {
    init();
    let mut controller = create_controller();

    controller.input.set_button(0, true);

    // Strobe low without prior high - should not latch
    controller.write_byte(0x4016, 0x00).unwrap();

    // Should still return default state
    let result = controller.read_byte(0x4016).unwrap();
    assert_eq!(result & 0x1F, 0x01);
}

// ============================================================================
// read_word and write_word (not typically used but should work)
// ============================================================================

#[test]
fn read_word_returns_zero() {
    init();
    let controller = create_controller();
    let result = controller.read_word(0x4016).unwrap();
    assert_eq!(result, 0);
}

#[test]
fn write_word_succeeds() {
    init();
    let mut controller = create_controller();
    let result = controller.write_word(0x4016, 0x0000);
    assert!(result.is_ok());
}
