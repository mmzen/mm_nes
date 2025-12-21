// Authorship: Human 20% | Claude 80%
//! Tests for PPU DMA signal-based interface.
//!
//! The actual byte-by-byte DMA transfer is tested in dma_controller.rs tests.
//! These tests verify that PpuDma correctly signals DMA start via the shared cell.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use crate::bus::MockBusStub;
use crate::dma_controller::DmaController;
use crate::dma_device::MockDmaDeviceStub;
use crate::memory::Memory;
use crate::ppu_dma::PpuDma;
use crate::tests::init;

/// Creates a PpuDma with mock signals for testing.
/// Returns (ppu_dma, dma_signal, data_bus)
fn create_ppu_dma() -> (PpuDma, Rc<Cell<Option<u8>>>, Rc<Cell<u8>>) {
    let dma_signal = Rc::new(Cell::new(None));
    let data_bus = Rc::new(Cell::new(0x00));
    let ppu_dma = PpuDma::new_with_dma_signal(dma_signal.clone(), data_bus.clone());
    (ppu_dma, dma_signal, data_bus)
}

#[test]
fn dma_transfer_works() {
    init();

    let (mut ppu_dma, dma_signal, _data_bus) = create_ppu_dma();
    ppu_dma.initialize().unwrap();

    // Write to $4014 should signal DMA start with the page value
    ppu_dma.write_byte(0x00, 0x20).unwrap();

    assert_eq!(dma_signal.get(), Some(0x20), "DMA should be signaled with page 0x20");
}

#[test]
fn dma_transfer_through_register_works() {
    init();

    let (mut ppu_dma, dma_signal, _data_bus) = create_ppu_dma();
    ppu_dma.initialize().unwrap();

    // Write to any offset (only $4014 is mapped) should signal DMA
    ppu_dma.write_byte(0x00, 0x02).unwrap();

    assert_eq!(dma_signal.get(), Some(0x02), "DMA should be signaled with page 0x02");
}

#[test]
fn read_returns_open_bus_not_last_written_value() {
    init();

    let (mut ppu_dma, _dma_signal, data_bus) = create_ppu_dma();
    ppu_dma.initialize().unwrap();

    // Set data bus to a known value
    data_bus.set(0x42);

    // Write a different value to $4014
    ppu_dma.write_byte(0x00, 0x20).unwrap();

    // Reading should return open bus (0x42), NOT the written value (0x20)
    // $4014 is write-only on real hardware
    assert_eq!(ppu_dma.read_byte(0x00).unwrap(), 0x42,
        "$4014 read should return open bus value, not last written value");
}

#[test]
fn double_write_last_value_wins() {
    init();

    let (mut ppu_dma, dma_signal, _data_bus) = create_ppu_dma();
    ppu_dma.initialize().unwrap();

    // Write twice before scheduler samples
    ppu_dma.write_byte(0x00, 0x10).unwrap();
    ppu_dma.write_byte(0x00, 0x20).unwrap();

    // Last write should win
    assert_eq!(dma_signal.get(), Some(0x20),
        "Double-write: last value should win");
}

/// Creates a DmaController with mock bus/ppu for timing tests.
fn create_dma_controller() -> DmaController<MockBusStub, MockDmaDeviceStub> {
    let mut bus = MockBusStub::new();
    bus.expect_read_byte().returning(|addr| Ok((addr & 0xFF) as u8));
    bus.expect_write_byte().returning(|_, _| Ok(()));

    let mut ppu = MockDmaDeviceStub::new();
    ppu.expect_dma_write().returning(|_, _| Ok(()));

    DmaController::new(
        Rc::new(RefCell::new(bus)),
        Rc::new(RefCell::new(ppu))
    )
}

/// Integration test: Verifies the N+1 timing contract.
///
/// Writing to $4014 sets the latch; DMA does NOT begin until the scheduler
/// samples the latch on the NEXT master cycle. This test demonstrates:
/// 1. Writing to PpuDma sets dma_start_page signal
/// 2. DmaController is NOT active until start_oam_dma is called
/// 3. Only after the scheduler "samples" (next cycle) is DMA active
#[test]
fn dma_begins_on_cycle_n_plus_1_not_cycle_n() {
    init();

    let dma_signal = Rc::new(Cell::new(None));
    let data_bus = Rc::new(Cell::new(0x00));
    let mut ppu_dma = PpuDma::new_with_dma_signal(dma_signal.clone(), data_bus.clone());
    ppu_dma.initialize().unwrap();

    let mut dma_controller = create_dma_controller();

    // CYCLE N: CPU writes to $4014
    // This sets the signal but does NOT start DMA yet
    ppu_dma.write_byte(0x00, 0x20).unwrap();

    // At this point in cycle N (after the write):
    // - dma_start_page is set to Some(0x20)
    // - DmaController is still idle (DMA has NOT started)
    assert_eq!(dma_signal.get(), Some(0x20), "Signal should be set after write");
    assert!(!dma_controller.is_oam_dma_active(),
        "DMA should NOT be active immediately after write (still in cycle N)");

    // CYCLE N+1: Scheduler samples the latch and starts DMA
    // In the real scheduler, this happens at the start of step_master_cycle
    let page = dma_signal.take().unwrap();
    dma_controller.start_oam_dma(page);

    // Now DMA is active (PendingHalt state)
    assert!(dma_controller.is_oam_dma_active(),
        "DMA should be active after scheduler samples signal (cycle N+1)");

    // Signal should be cleared after sampling
    assert_eq!(dma_signal.get(), None, "Signal should be cleared after sampling");
}
