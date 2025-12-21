// Authorship: Human 0% | Claude 100%
//! Tests for error type Display implementations
//!
//! These tests ensure proper error formatting for debugging and logging.

use crate::tests::init;
use crate::ppu::PpuError;
use crate::cpu::CpuError;
use crate::bus::BusError;
use crate::memory::MemoryError;
use crate::ppu::PpuType;

// ============================================================================
// PpuType Display tests
// ============================================================================

#[test]
fn ppu_type_display_shows_nes2c02() {
    init();
    let ppu_type = PpuType::NES2C02;
    let display = format!("{}", ppu_type);
    assert!(display.contains("NES2C02"));
}

#[test]
fn ppu_type_equality_works() {
    init();
    let ppu1 = PpuType::NES2C02;
    let ppu2 = PpuType::NES2C02;
    assert_eq!(ppu1, ppu2);
}

// ============================================================================
// PpuError Display tests
// ============================================================================

#[test]
fn ppu_error_bus_error_display() {
    init();
    let bus_error = BusError::Unmapped(0x1234);
    let ppu_error = PpuError::BusError(bus_error);
    let display = format!("{}", ppu_error);
    assert!(display.contains("bus error"));
}

#[test]
fn ppu_error_memory_error_display() {
    init();
    let memory_error = MemoryError::OutOfRange(0xABCD);
    let ppu_error = PpuError::MemoryError(memory_error);
    let display = format!("{}", ppu_error);
    assert!(display.contains("memory error"));
}

#[test]
fn ppu_error_unsupported_configuration_display() {
    init();
    let ppu_error = PpuError::UnsupportedConfiguration("test config".to_string());
    let display = format!("{}", ppu_error);
    assert!(display.contains("unsupported configuration"));
    assert!(display.contains("test config"));
}

#[test]
fn ppu_error_from_memory_error() {
    init();
    let memory_error = MemoryError::OutOfRange(0x1000);
    let ppu_error: PpuError = memory_error.into();
    match ppu_error {
        PpuError::MemoryError(_) => {},
        _ => panic!("Expected MemoryError variant"),
    }
}

#[test]
fn ppu_error_from_bus_error() {
    init();
    let bus_error = BusError::Unmapped(0x2000);
    let ppu_error: PpuError = bus_error.into();
    match ppu_error {
        PpuError::BusError(_) => {},
        _ => panic!("Expected BusError variant"),
    }
}

// ============================================================================
// CpuError Display tests
// ============================================================================

#[test]
fn cpu_error_from_memory_error() {
    init();
    let memory_error = MemoryError::OutOfRange(0x3000);
    let cpu_error: CpuError = memory_error.into();
    match cpu_error {
        CpuError::MemoryError(_) => {},
        _ => panic!("Expected MemoryError variant"),
    }
}

// ============================================================================
// MemoryError Display tests
// ============================================================================

#[test]
fn memory_error_out_of_range_display() {
    init();
    let error = MemoryError::OutOfRange(0xFFFF);
    let display = format!("{}", error);
    assert!(display.contains("out of bounds"));
    assert!(display.contains("FFFF") || display.contains("ffff"));
}

#[test]
fn memory_error_bus_error_display() {
    init();
    let error = MemoryError::BusError(0x1234);
    let display = format!("{}", error);
    assert!(display.contains("bus error"));
}

#[test]
fn memory_error_illegal_state_display() {
    init();
    let error = MemoryError::IllegalState("test state".to_string());
    let display = format!("{}", error);
    assert!(display.contains("illegal state"));
    assert!(display.contains("test state"));
}

#[test]
fn memory_error_invalid_address_space_display() {
    init();
    let error = MemoryError::InvalidAddressSpace("test space".to_string());
    let display = format!("{}", error);
    assert!(display.contains("invalid address space"));
    assert!(display.contains("test space"));
}

// ============================================================================
// BusError Display tests
// ============================================================================

#[test]
fn bus_error_unmapped_display() {
    init();
    let error = BusError::Unmapped(0xDEAD);
    let display = format!("{}", error);
    assert!(display.contains("unmapped"));
    assert!(display.contains("DEAD") || display.contains("dead"));
}

#[test]
fn memory_error_from_bus_error() {
    init();
    let bus_error = BusError::Unmapped(0x4000);
    let memory_error: MemoryError = bus_error.into();
    match memory_error {
        MemoryError::BusError(_) => {},
        _ => panic!("Expected BusError variant"),
    }
}
