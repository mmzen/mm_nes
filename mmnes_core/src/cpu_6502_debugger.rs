use crate::cpu_debugger::{Breakpoints, CpuSnapshot};

#[derive(Debug, Clone)]
struct Cpu6502Breakpoints {
    map: Vec<bool>
}

impl Cpu6502Breakpoints {
    pub fn new(addr_space: usize) -> Self {
        Cpu6502Breakpoints { map: vec![false; addr_space] }
    }
}

impl Breakpoints for Cpu6502Breakpoints {
    fn set(&mut self, addr: u16) {
        self.map[addr as usize] = true;
    }

    fn clear(&mut self, addr: u16) {
        self.map[addr as usize] = false;
    }

    fn contains(&self, addr: u16) -> bool {
        self.map[addr as usize]
    }

    fn list(&self) -> Vec<u16> {
        self.map
            .iter()
            .enumerate()
            .filter_map(|(addr, &b)| b.then(|| addr as u16))
            .collect()
    }
}

pub enum DebugCommand {
    Step,
    Continue,
    BreakAt(u16),
    ShowCpuState,
}

struct Cpu6502Debugger<B: Breakpoints, C: CpuSnapshot> {
    breakpoints: B,
    cpu_snapshot: Option<C>
}

impl<B: Breakpoints + Default, C: CpuSnapshot> Cpu6502Debugger<B, C> {
    pub fn new() -> Self {
        Cpu6502Debugger {
            breakpoints: B::default(),
            cpu_snapshot: None
        }
    }

    fn add_breakpoint(&mut self, addr: u16) {
        self.breakpoints.set(addr);
    }

    fn remove_breakpoint(&mut self, addr: u16) {
        self.breakpoints.clear(addr);
    }

    fn list_breakpoints(&self) -> Vec<u16> {
        self.breakpoints.list()
    }

    fn set_cpu_snapshot(&mut self, snapshot: C) {
        self.cpu_snapshot = Some(snapshot);
    }

    fn clear_cpu_snapshot(&mut self) {
        self.cpu_snapshot = None;
    }

    pub fn display_cpu_state(&self) {
        if let Some(ref snapshot) = self.cpu_snapshot {
            println!("PC={:04X} A={:02X} X={:02X} Y={:02X} SP={:04X} P={:02X} CYCLES={}",
                     snapshot.pc(), snapshot.a(), snapshot.x(), snapshot.y(), snapshot.sp(), snapshot.p(),
                     snapshot.total_cycles());
        } else {
            println!("No CPU snapshot available");
        }
    }
}