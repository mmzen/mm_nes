use std::fmt::Debug;

#[derive(Debug, Clone, Copy)]
pub enum DebugStopReason {
    None,
    BreakpointHit(u16),
    SingleStep,
    CreditsConsumed(u32),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DebugCommand {
    StepInstruction,
    Paused,
    Run,
    StepInto,
    StepOut,
    StepOver,
    AddBreakpoint(u16),
    DeleteBreakpoint(u16),
    DeleteAllBreakpoints,
    ListBreakpoints,
    Detach
}

pub trait CpuSnapshot: Debug + Send {
    fn pc(&self) -> u16;
    fn a(&self) -> u8;
    fn x(&self) -> u8;
    fn y(&self) -> u8;
    fn sp(&self) -> u8;
    fn p(&self) -> u8;
    fn instruction(&self) -> Vec<u8>;
    fn mnemonic(&self) -> String;
    fn is_illegal(&self) -> bool;
    fn operand(&self) -> String;
    fn cycles(&self) -> u32;
}

pub trait Breakpoints: Debug {
    fn set(&mut self, addr: u16);
    fn clear(&mut self, addr: u16);
    fn contains(&self, addr: u16) -> bool;
    fn list(&self) -> Vec<u16>;
}