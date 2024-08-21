use std::cell::RefCell;
use std::rc::Rc;
use crate::bus::MockBusStub;
use crate::dma::Dma;
use crate::dma_device::MockDmaDeviceStub;
use crate::memory::Memory;
use crate::ppu_dma::PpuDma;
use crate::tests::init;

const BYTE_DATA: u8 = 0xAB;

fn create_bus() -> MockBusStub {
    let mut bus = MockBusStub::new();

    bus.expect_read_byte().times(256).returning(|_| Ok(BYTE_DATA));
    bus
}

fn create_dma_device() -> MockDmaDeviceStub {
    let mut dma = MockDmaDeviceStub::new();

    let mut n= 0;
    dma.expect_dma_write().times(256).returning(move |offset, value| {
        if offset == n && value == BYTE_DATA {
            n = n.wrapping_add(1);
            Ok(())
        } else {
            panic!("unexpected DMA write: offset={}, value=0x{:02X}", offset, value)
        }
    });
    dma
}

fn create_ppu_dma() -> PpuDma {
    let bus = create_bus();
    let device = create_dma_device();

    PpuDma::new(Rc::new(RefCell::new(device)), Rc::new(RefCell::new(bus)))
}

#[test]
fn dma_transfer_works() {
    init();

    let mut ppu_dma = create_ppu_dma();
    let value = 0x20;

    ppu_dma.transfer_memory(value).unwrap();
}

#[test]
fn dma_transfer_through_register_works() {
    init();

    let mut ppu_dma = create_ppu_dma();
    let value = 0x20;

    ppu_dma.write_byte(0x1234, value).unwrap();
}

