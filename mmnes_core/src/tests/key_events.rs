use crate::key_event::{KeyEvent, KeyEvents, NES_CONTROLLER_KEY_A, NES_CONTROLLER_KEY_B, NES_CONTROLLER_KEY_START};

#[test]
fn new_creates_empty_key_events() {
    let key_events = KeyEvents::new();
    assert!(key_events.is_empty());
}

#[test]
fn push_single_key_event_and_retrieve() {
    let mut key_events = KeyEvents::new();
    let key_event = KeyEvent {
        key: NES_CONTROLLER_KEY_A,
        pressed: true
    };

    key_events.push_back(key_event);

    let retrieved_event = key_events.pop_front();
    assert_eq!(retrieved_event.is_some(), true);

    let unwrapped_event = retrieved_event.unwrap();
    assert_eq!(unwrapped_event.key, NES_CONTROLLER_KEY_A);
    assert_eq!(unwrapped_event.pressed, true);
}

#[test]
fn handle_multiple_key_events_in_fifo_order() {
    let mut key_events = KeyEvents::new();

    let event1 = KeyEvent {
        key: NES_CONTROLLER_KEY_A,
        pressed: true
    };
    let event2 = KeyEvent {
        key: NES_CONTROLLER_KEY_B,
        pressed: false
    };
    let event3 = KeyEvent {
        key: NES_CONTROLLER_KEY_START,
        pressed: true
    };

    key_events.push_back(event1);
    key_events.push_back(event2);
    key_events.push_back(event3);

    let retrieved_event1 = key_events.pop_front().unwrap();
    assert_eq!(retrieved_event1.key, NES_CONTROLLER_KEY_A);
    assert_eq!(retrieved_event1.pressed, true);

    let retrieved_event2 = key_events.pop_front().unwrap();
    assert_eq!(retrieved_event2.key, NES_CONTROLLER_KEY_B);
    assert_eq!(retrieved_event2.pressed, false);

    let retrieved_event3 = key_events.pop_front().unwrap();
    assert_eq!(retrieved_event3.key, NES_CONTROLLER_KEY_START);
    assert_eq!(retrieved_event3.pressed, true);

    assert!(key_events.is_empty());
}

#[test]
fn pop_front_returns_none_when_empty() {
    let mut key_events = KeyEvents::new();
    let result = key_events.pop_front();
    assert_eq!(result, None);
}

#[test]
fn is_empty_returns_false_after_adding_events_and_true_after_clearing() {
    let mut key_events = KeyEvents::new();
    assert!(key_events.is_empty());

    let key_event = KeyEvent {
        key: NES_CONTROLLER_KEY_A,
        pressed: true
    };

    key_events.push_back(key_event);
    assert_eq!(key_events.is_empty(), false);

    key_events.clear();
    assert!(key_events.is_empty());
}

#[test]
fn iterator_implementation_returns_all_events_in_correct_order() {
    let mut key_events = KeyEvents::new();

    let event1 = KeyEvent {
        key: NES_CONTROLLER_KEY_A,
        pressed: true
    };
    let event2 = KeyEvent {
        key: NES_CONTROLLER_KEY_B,
        pressed: false
    };
    let event3 = KeyEvent {
        key: NES_CONTROLLER_KEY_START,
        pressed: true
    };

    key_events.push_back(event1);
    key_events.push_back(event2);
    key_events.push_back(event3);

    let collected_events: Vec<KeyEvent> = key_events.collect();

    assert_eq!(collected_events.len(), 3);
    assert_eq!(collected_events[0].key, NES_CONTROLLER_KEY_A);
    assert_eq!(collected_events[0].pressed, true);
    assert_eq!(collected_events[1].key, NES_CONTROLLER_KEY_B);
    assert_eq!(collected_events[1].pressed, false);
    assert_eq!(collected_events[2].key, NES_CONTROLLER_KEY_START);
    assert_eq!(collected_events[2].pressed, true);
}

#[test]
fn iterator_implementation_on_empty_key_events_returns_none() {
    let mut key_events = KeyEvents::new();
    let result = key_events.next();
    assert_eq!(result, None);
}

