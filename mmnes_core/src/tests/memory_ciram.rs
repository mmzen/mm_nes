use std::cell::RefCell;
use std::rc::Rc;
use crate::bus::Bus;
use crate::memory::Memory;
use crate::memory_ciram::{CiramMemory, PpuNameTableMirroring};
use crate::nes_bus::NESBus;
use crate::tests::init;

const PPU_CIRAM_SIZE: usize = 4 * 1024;

#[test]
fn create_ciram_memory_with_vertical_mirroring_and_correct_size() {
    init();

    let ciram = CiramMemory::new(PpuNameTableMirroring::Vertical);

    assert_eq!(ciram.size(), PPU_CIRAM_SIZE);
    assert_eq!(ciram.mirroring(), PpuNameTableMirroring::Vertical);
}

#[test]
fn create_ciram_memory_with_horizontal_mirroring_and_correct_size() {
    init();

    let ciram = CiramMemory::new(PpuNameTableMirroring::Horizontal);

    assert_eq!(ciram.size(), PPU_CIRAM_SIZE);
    assert_eq!(ciram.mirroring(), PpuNameTableMirroring::Horizontal);
}

#[test]
fn create_ciram_memory_with_single_screen_lower_mirroring_and_correct_size() {
    init();

    let ciram = CiramMemory::new(PpuNameTableMirroring::SingleScreenLower);

    assert_eq!(ciram.size(), PPU_CIRAM_SIZE);
    assert_eq!(ciram.mirroring(), PpuNameTableMirroring::SingleScreenLower);
}

#[test]
fn create_ciram_memory_with_single_screen_upper_mirroring_and_correct_size() {
    init();

    let ciram = CiramMemory::new(PpuNameTableMirroring::SingleScreenUpper);

    assert_eq!(ciram.size(), PPU_CIRAM_SIZE);
    assert_eq!(ciram.mirroring(), PpuNameTableMirroring::SingleScreenUpper);
}

#[test]
fn vertical_mirroring_maps_addresses_correctly() {
    init();

    let mut ciram = CiramMemory::new(PpuNameTableMirroring::Vertical);
    let test_value = 0xAB;

    // Write to nametable 0 (0x0000-0x23FF)
    ciram.write_byte(0x0000, test_value).unwrap();

    // Vertical mirroring: nametable 0 mirrors to nametable 2
    assert_eq!(ciram.read_byte(0x0800).unwrap(), test_value);

    // Write to nametable 1 (0x0400-0x27FF)
    ciram.write_byte(0x0400, test_value + 1).unwrap();

    // Vertical mirroring: nametable 1 mirrors to nametable 3
    assert_eq!(ciram.read_byte(0x0C00).unwrap(), test_value + 1);

    // Verify original addresses still contain the values
    assert_eq!(ciram.read_byte(0x0000).unwrap(), test_value);
    assert_eq!(ciram.read_byte(0x0400).unwrap(), test_value + 1);

    // Verify that nametable 0 and 2 share the same memory
    ciram.write_byte(0x0800, test_value + 2).unwrap();
    assert_eq!(ciram.read_byte(0x0000).unwrap(), test_value + 2);

    // Verify that nametable 1 and 3 share the same memory
    ciram.write_byte(0x0C00, test_value + 3).unwrap();
    assert_eq!(ciram.read_byte(0x0400).unwrap(), test_value + 3);
}

#[test]
fn horizontal_mirroring_maps_addresses_correctly() {
    init();

    let mut ciram = CiramMemory::new(PpuNameTableMirroring::Horizontal);
    let test_value = 0xAB;

    // Write to nametable 0 (0x0000-0x23FF)
    ciram.write_byte(0x0000, test_value).unwrap();

    // Horizontal mirroring: nametable 0 mirrors to nametable 1
    assert_eq!(ciram.read_byte(0x0400).unwrap(), test_value);

    // Write to nametable 2 (0x0800-0x2BFF)
    ciram.write_byte(0x0800, test_value + 1).unwrap();

    // Horizontal mirroring: nametable 2 mirrors to nametable 3
    assert_eq!(ciram.read_byte(0x0C00).unwrap(), test_value + 1);

    // Verify original addresses still contain the values
    assert_eq!(ciram.read_byte(0x0000).unwrap(), test_value);
    assert_eq!(ciram.read_byte(0x0800).unwrap(), test_value + 1);

    // Verify that nametable 0 and 1 share the same memory
    ciram.write_byte(0x0400, test_value + 2).unwrap();
    assert_eq!(ciram.read_byte(0x0000).unwrap(), test_value + 2);

    // Verify that nametable 2 and 3 share the same memory
    ciram.write_byte(0x0C00, test_value + 3).unwrap();
    assert_eq!(ciram.read_byte(0x0800).unwrap(), test_value + 3);
}

#[test]
fn single_screen_lower_mirroring_maps_addresses_correctly() {
    init();

    let mut ciram = CiramMemory::new(PpuNameTableMirroring::SingleScreenLower);
    let test_value = 0xAB;

    // Write to nametable 0 (0x0000-0x23FF)
    ciram.write_byte(0x0000, test_value).unwrap();

    // Single screen lower: all nametables map to the same memory
    assert_eq!(ciram.read_byte(0x0400).unwrap(), test_value); // nametable 1
    assert_eq!(ciram.read_byte(0x0800).unwrap(), test_value); // nametable 2
    assert_eq!(ciram.read_byte(0x0C00).unwrap(), test_value); // nametable 3

    // Write to nametable 1 and verify all nametables see the change
    ciram.write_byte(0x0400, test_value + 1).unwrap();
    assert_eq!(ciram.read_byte(0x0000).unwrap(), test_value + 1);
    assert_eq!(ciram.read_byte(0x0800).unwrap(), test_value + 1);
    assert_eq!(ciram.read_byte(0x0C00).unwrap(), test_value + 1);

    // Write to nametable 2 and verify all nametables see the change
    ciram.write_byte(0x0800, test_value + 2).unwrap();
    assert_eq!(ciram.read_byte(0x0000).unwrap(), test_value + 2);
    assert_eq!(ciram.read_byte(0x0400).unwrap(), test_value + 2);
    assert_eq!(ciram.read_byte(0x0C00).unwrap(), test_value + 2);

    // Write to nametable 3 and verify all nametables see the change
    ciram.write_byte(0x0C00, test_value + 3).unwrap();
    assert_eq!(ciram.read_byte(0x0000).unwrap(), test_value + 3);
    assert_eq!(ciram.read_byte(0x0400).unwrap(), test_value + 3);
    assert_eq!(ciram.read_byte(0x0800).unwrap(), test_value + 3);
}

#[test]
fn single_screen_upper_mirroring_maps_addresses_correctly() {
    init();

    let mut ciram = CiramMemory::new(PpuNameTableMirroring::SingleScreenUpper);
    let test_value = 0xAB;

    // Write to nametable 0 (0x0000-0x23FF)
    ciram.write_byte(0x0000, test_value).unwrap();

    // Single screen upper: all nametables map to the same memory
    assert_eq!(ciram.read_byte(0x0400).unwrap(), test_value); // nametable 1
    assert_eq!(ciram.read_byte(0x0800).unwrap(), test_value); // nametable 2
    assert_eq!(ciram.read_byte(0x0C00).unwrap(), test_value); // nametable 3

    // Write to nametable 1 and verify all nametables see the change
    ciram.write_byte(0x0400, test_value + 1).unwrap();
    assert_eq!(ciram.read_byte(0x0000).unwrap(), test_value + 1);
    assert_eq!(ciram.read_byte(0x0800).unwrap(), test_value + 1);
    assert_eq!(ciram.read_byte(0x0C00).unwrap(), test_value + 1);

    // Write to nametable 2 and verify all nametables see the change
    ciram.write_byte(0x0800, test_value + 2).unwrap();
    assert_eq!(ciram.read_byte(0x0000).unwrap(), test_value + 2);
    assert_eq!(ciram.read_byte(0x0400).unwrap(), test_value + 2);
    assert_eq!(ciram.read_byte(0x0C00).unwrap(), test_value + 2);

    // Write to nametable 3 and verify all nametables see the change
    ciram.write_byte(0x0C00, test_value + 3).unwrap();
    assert_eq!(ciram.read_byte(0x0000).unwrap(), test_value + 3);
    assert_eq!(ciram.read_byte(0x0400).unwrap(), test_value + 3);
    assert_eq!(ciram.read_byte(0x0800).unwrap(), test_value + 3);
}

#[test]
fn vertical_mirroring_maps_word_addresses_correctly() {
    init();

    let mut ciram = CiramMemory::new(PpuNameTableMirroring::Vertical);
    let test_value = 0xABCD;

    // Write to nametable 0 (0x0000-0x23FF)
    ciram.write_word(0x0000, test_value).unwrap();

    // Vertical mirroring: nametable 0 mirrors to nametable 2
    assert_eq!(ciram.read_word(0x0800).unwrap(), test_value);

    // Write to nametable 1 (0x0400-0x27FF)
    ciram.write_word(0x0400, test_value + 1).unwrap();

    // Vertical mirroring: nametable 1 mirrors to nametable 3
    assert_eq!(ciram.read_word(0x0C00).unwrap(), test_value + 1);

    // Verify original addresses still contain the values
    assert_eq!(ciram.read_word(0x0000).unwrap(), test_value);
    assert_eq!(ciram.read_word(0x0400).unwrap(), test_value + 1);

    // Verify that nametable 0 and 2 share the same memory
    ciram.write_word(0x0800, test_value + 2).unwrap();
    assert_eq!(ciram.read_word(0x0000).unwrap(), test_value + 2);

    // Verify that nametable 1 and 3 share the same memory
    ciram.write_word(0x0C00, test_value + 3).unwrap();
    assert_eq!(ciram.read_word(0x0400).unwrap(), test_value + 3);
}

#[test]
fn horizontal_mirroring_maps_word_addresses_correctly() {
    init();

    let mut ciram = CiramMemory::new(PpuNameTableMirroring::Horizontal);
    let test_value = 0xABCD;

    // Write to nametable 0 (0x0000-0x23FF)
    ciram.write_word(0x0000, test_value).unwrap();

    // Horizontal mirroring: nametable 0 mirrors to nametable 1
    assert_eq!(ciram.read_word(0x0400).unwrap(), test_value);

    // Write to nametable 2 (0x0800-0x2BFF)
    ciram.write_word(0x0800, test_value + 1).unwrap();

    // Horizontal mirroring: nametable 2 mirrors to nametable 3
    assert_eq!(ciram.read_word(0x0C00).unwrap(), test_value + 1);

    // Verify original addresses still contain the values
    assert_eq!(ciram.read_word(0x0000).unwrap(), test_value);
    assert_eq!(ciram.read_word(0x0800).unwrap(), test_value + 1);

    // Verify that nametable 0 and 1 share the same memory
    ciram.write_word(0x0400, test_value + 2).unwrap();
    assert_eq!(ciram.read_word(0x0000).unwrap(), test_value + 2);

    // Verify that nametable 2 and 3 share the same memory
    ciram.write_word(0x0C00, test_value + 3).unwrap();
    assert_eq!(ciram.read_word(0x0800).unwrap(), test_value + 3);
}

#[test]
fn single_screen_lower_mirroring_maps_word_addresses_correctly() {
    init();

    let mut ciram = CiramMemory::new(PpuNameTableMirroring::SingleScreenLower);
    let test_value = 0xABCD;

    // Write to nametable 0 (0x0000-0x23FF)
    ciram.write_word(0x0000, test_value).unwrap();

    // Single screen lower: all nametables map to the same memory
    assert_eq!(ciram.read_word(0x0400).unwrap(), test_value); // nametable 1
    assert_eq!(ciram.read_word(0x0800).unwrap(), test_value); // nametable 2
    assert_eq!(ciram.read_word(0x0C00).unwrap(), test_value); // nametable 3

    // Write to nametable 1 and verify all nametables see the change
    ciram.write_word(0x0400, test_value + 1).unwrap();
    assert_eq!(ciram.read_word(0x0000).unwrap(), test_value + 1);
    assert_eq!(ciram.read_word(0x0800).unwrap(), test_value + 1);
    assert_eq!(ciram.read_word(0x0C00).unwrap(), test_value + 1);

    // Write to nametable 2 and verify all nametables see the change
    ciram.write_word(0x0800, test_value + 2).unwrap();
    assert_eq!(ciram.read_word(0x0000).unwrap(), test_value + 2);
    assert_eq!(ciram.read_word(0x0400).unwrap(), test_value + 2);
    assert_eq!(ciram.read_word(0x0C00).unwrap(), test_value + 2);

    // Write to nametable 3 and verify all nametables see the change
    ciram.write_word(0x0C00, test_value + 3).unwrap();
    assert_eq!(ciram.read_word(0x0000).unwrap(), test_value + 3);
    assert_eq!(ciram.read_word(0x0400).unwrap(), test_value + 3);
    assert_eq!(ciram.read_word(0x0800).unwrap(), test_value + 3);
}

#[test]
fn single_screen_upper_mirroring_maps_word_addresses_correctly() {
    init();

    let mut ciram = CiramMemory::new(PpuNameTableMirroring::SingleScreenUpper);
    let test_value = 0xABCD;

    // Write to nametable 0 (0x0000-0x23FF)
    ciram.write_word(0x0000, test_value).unwrap();

    // Single screen upper: all nametables map to the same memory
    assert_eq!(ciram.read_word(0x0400).unwrap(), test_value); // nametable 1
    assert_eq!(ciram.read_word(0x0800).unwrap(), test_value); // nametable 2
    assert_eq!(ciram.read_word(0x0C00).unwrap(), test_value); // nametable 3

    // Write to nametable 1 and verify all nametables see the change
    ciram.write_word(0x0400, test_value + 1).unwrap();
    assert_eq!(ciram.read_word(0x0000).unwrap(), test_value + 1);
    assert_eq!(ciram.read_word(0x0800).unwrap(), test_value + 1);
    assert_eq!(ciram.read_word(0x0C00).unwrap(), test_value + 1);

    // Write to nametable 2 and verify all nametables see the change
    ciram.write_word(0x0800, test_value + 2).unwrap();
    assert_eq!(ciram.read_word(0x0000).unwrap(), test_value + 2);
    assert_eq!(ciram.read_word(0x0400).unwrap(), test_value + 2);
    assert_eq!(ciram.read_word(0x0C00).unwrap(), test_value + 2);

    // Write to nametable 3 and verify all nametables see the change
    ciram.write_word(0x0C00, test_value + 3).unwrap();
    assert_eq!(ciram.read_word(0x0000).unwrap(), test_value + 3);
    assert_eq!(ciram.read_word(0x0400).unwrap(), test_value + 3);
    assert_eq!(ciram.read_word(0x0800).unwrap(), test_value + 3);
}

#[test]
fn vertical_mirroring_maps_word_addresses_correctly_through_bus_and_virtual_addresses() {
    init();

    let mut bus = NESBus::new();
    let ciram = CiramMemory::new(PpuNameTableMirroring::Vertical);
    let test_value = 0xAB;

    bus.add_device(Rc::new(RefCell::new(ciram))).unwrap();

    bus.write_byte(0x2000, test_value).unwrap();

    // Vertical mirroring: nametable 0 mirrors to nametable 2
    assert_eq!(bus.read_byte(0x2800).unwrap(), test_value);

    // Write to nametable 1 (0x0400-0x27FF)
    bus.write_byte(0x2400, test_value + 1).unwrap();

    // Vertical mirroring: nametable 1 mirrors to nametable 3
    assert_eq!(bus.read_byte(0x2C00).unwrap(), test_value + 1);

    // Verify original addresses still contain the values
    assert_eq!(bus.read_byte(0x2000).unwrap(), test_value);
    assert_eq!(bus.read_byte(0x2400).unwrap(), test_value + 1);

    // Verify that nametable 0 and 2 share the same memory
    bus.write_byte(0x2800, test_value + 2).unwrap();
    assert_eq!(bus.read_byte(0x2000).unwrap(), test_value + 2);

    // Verify that nametable 1 and 3 share the same memory
    bus.write_byte(0x2C00, test_value + 3).unwrap();
    assert_eq!(bus.read_byte(0x2400).unwrap(), test_value + 3);
}

#[test]
fn vertical_mirroring_maps_word_addresses_correctly_through_bus_and_virtual_addresses_and_with_bus_mirroring() {
    init();

    let mut bus = NESBus::new();
    let ciram = CiramMemory::new(PpuNameTableMirroring::Vertical);
    let test_value = 0xAB;

    bus.add_device(Rc::new(RefCell::new(ciram))).unwrap();

    // write beyond nametables and trigger bus mirroring LOOP ICI
    bus.write_byte(0x3000, test_value).unwrap();

    // Vertical mirroring: nametable 0 mirrors to nametable 2
    assert_eq!(bus.read_byte(0x2000).unwrap(), test_value);
    assert_eq!(bus.read_byte(0x2800).unwrap(), test_value);

    // Write to nametable 1 (0x0400-0x27FF)
    bus.write_byte(0x3400, test_value + 1).unwrap();

    // Vertical mirroring: nametable 1 mirrors to nametable 3
    assert_eq!(bus.read_byte(0x2400).unwrap(), test_value + 1);
    assert_eq!(bus.read_byte(0x2C00).unwrap(), test_value + 1);

    // Verify original addresses still contain the values
    assert_eq!(bus.read_byte(0x2000).unwrap(), test_value);
    assert_eq!(bus.read_byte(0x2400).unwrap(), test_value + 1);

    // Verify that nametable 0 and 2 share the same memory
    bus.write_byte(0x2800, test_value + 2).unwrap();
    assert_eq!(bus.read_byte(0x2000).unwrap(), test_value + 2);

    // Verify that nametable 1 and 3 share the same memory
    bus.write_byte(0x2C00, test_value + 3).unwrap();
    assert_eq!(bus.read_byte(0x2400).unwrap(), test_value + 3);
}