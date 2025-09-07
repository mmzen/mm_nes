use std::fmt::{Debug, Display};


#[derive(Debug, Clone, Copy)]
pub enum DebugStopReason {
    None,
    BreakpointHit(u16),
    SingleStep,
    CreditsConsumed(u32),
}

pub trait CpuSnapshot: Debug + Display + Send {
    fn pc(&self) -> u16;
    fn a(&self) -> u8;
    fn x(&self) -> u8;
    fn y(&self) -> u8;
    fn sp(&self) -> u8;
    fn p(&self) -> u8;
    fn total_cycles(&self) -> u64;
}

pub trait Breakpoints: Debug {
    fn set(&mut self, addr: u16);
    fn clear(&mut self, addr: u16);
    fn contains(&self, addr: u16) -> bool;
    fn list(&self) -> Vec<u16>;
}