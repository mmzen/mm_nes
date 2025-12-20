// Authorship: Human 30% | Claude 70%
//! Tests for PPU DMA signal-based interface.
//!
//! The actual byte-by-byte DMA transfer is tested in dma_controller.rs tests.
//! These tests verify that PpuDma correctly signals DMA start via the shared cell.

use std::cell::Cell;
use std::rc::Rc;
use crate::memory::Memory;
use crate::ppu_dma::PpuDma;
use crate::tests::init;

fn create_ppu_dma() -> (PpuDma, Rc<Cell<Option<u8>>>) {
    let dma_signal = Rc::new(Cell::new(None));
    let ppu_dma = PpuDma::new_with_dma_signal(dma_signal.clone());
    (ppu_dma, dma_signal)
}

#[test]
fn dma_transfer_works() {
    init();

    let (mut ppu_dma, dma_signal) = create_ppu_dma();
    ppu_dma.initialize().unwrap();

    // Write to $4014 should signal DMA start with the page value
    ppu_dma.write_byte(0x00, 0x20).unwrap();

    assert_eq!(dma_signal.get(), Some(0x20), "DMA should be signaled with page 0x20");
}

#[test]
fn dma_transfer_through_register_works() {
    init();

    let (mut ppu_dma, dma_signal) = create_ppu_dma();
    ppu_dma.initialize().unwrap();

    // Write to any offset (only $4014 is mapped) should signal DMA
    ppu_dma.write_byte(0x00, 0x02).unwrap();

    assert_eq!(dma_signal.get(), Some(0x02), "DMA should be signaled with page 0x02");

    // Reading should return the last written value
    assert_eq!(ppu_dma.read_byte(0x00).unwrap(), 0x02);
}
