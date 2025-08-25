use crate::input_external::InputExternal;
use crate::input::Input;
use crate::key_event::{KeyEvent, KeyEvents};
use crate::tests::init;


fn create_input_external() -> InputExternal {
    InputExternal::new()
}

#[test]
fn should_update_control_state_correctly_when_key_is_pressed() {
    init();

    let mut input_external = create_input_external();
    let mut control_states = [0u8; 8];

    let mut key_events = KeyEvents::new();
    key_events.push_back(KeyEvent { key: 2, pressed: true });
    key_events.push_back(KeyEvent { key: 5, pressed: true });

    input_external.set_input_state(key_events);
    input_external.get_input_state(&mut control_states);

    assert_eq!(control_states[2], 1);
    assert_eq!(control_states[5], 1);
    assert_eq!(control_states[0], 0);
    assert_eq!(control_states[1], 0);
    assert_eq!(control_states[3], 0);
    assert_eq!(control_states[4], 0);
    assert_eq!(control_states[6], 0);
    assert_eq!(control_states[7], 0);
}

#[test]
fn should_update_control_state_correctly_when_key_is_released() {
    init();

    let mut input_external = create_input_external();
    let mut control_states = [1u8; 8];

    let mut key_events = KeyEvents::new();
    key_events.push_back(KeyEvent { key: 3, pressed: false });
    key_events.push_back(KeyEvent { key: 6, pressed: false });

    input_external.set_input_state(key_events);
    input_external.get_input_state(&mut control_states);

    assert_eq!(control_states[3], 0);
    assert_eq!(control_states[6], 0);
    assert_eq!(control_states[0], 1);
    assert_eq!(control_states[1], 1);
    assert_eq!(control_states[2], 1);
    assert_eq!(control_states[4], 1);
    assert_eq!(control_states[5], 1);
    assert_eq!(control_states[7], 1);
}

#[test]
fn should_handle_multiple_key_events_in_sequence_during_get_input_state() {
    init();

    let mut input_external = create_input_external();
    let mut control_states = [0u8; 8];

    let mut key_events = KeyEvents::new();
    key_events.push_back(KeyEvent { key: 0, pressed: true });
    key_events.push_back(KeyEvent { key: 1, pressed: true });
    key_events.push_back(KeyEvent { key: 2, pressed: true });
    key_events.push_back(KeyEvent { key: 1, pressed: false });
    key_events.push_back(KeyEvent { key: 0, pressed: false });

    input_external.set_input_state(key_events);
    input_external.get_input_state(&mut control_states);

    assert_eq!(control_states[0], 0);
    assert_eq!(control_states[1], 0);
    assert_eq!(control_states[2], 1);
    assert_eq!(control_states[3], 0);
    assert_eq!(control_states[4], 0);
    assert_eq!(control_states[5], 0);
    assert_eq!(control_states[6], 0);
    assert_eq!(control_states[7], 0);
}

#[test]
fn should_process_all_available_key_events_until_none_remain() {
    init();

    let mut input_external = create_input_external();
    let mut control_states = [0u8; 8];

    let mut key_events = KeyEvents::new();
    key_events.push_back(KeyEvent { key: 0, pressed: true });
    key_events.push_back(KeyEvent { key: 1, pressed: true });
    key_events.push_back(KeyEvent { key: 2, pressed: true });
    key_events.push_back(KeyEvent { key: 3, pressed: true });
    key_events.push_back(KeyEvent { key: 4, pressed: true });

    input_external.set_input_state(key_events);
    input_external.get_input_state(&mut control_states);

    assert_eq!(control_states[0], 1);
    assert_eq!(control_states[1], 1);
    assert_eq!(control_states[2], 1);
    assert_eq!(control_states[3], 1);
    assert_eq!(control_states[4], 1);

    let mut control_states_second = [0u8; 8];
    input_external.get_input_state(&mut control_states_second);

    assert_eq!(control_states_second[0], 0);
    assert_eq!(control_states_second[1], 0);
    assert_eq!(control_states_second[2], 0);
    assert_eq!(control_states_second[3], 0);
    assert_eq!(control_states_second[4], 0);
    assert_eq!(control_states_second[5], 0);
    assert_eq!(control_states_second[6], 0);
    assert_eq!(control_states_second[7], 0);
}

