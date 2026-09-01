//! `Machine` — CPU + address map, with the loop helpers a front end or test
//! harness needs.

use clinker_m68k::{Cpu, RunState, StepResult};

use crate::system_bus::SystemBus;

/// A whole Clinker 68000 (well, the CPU and its address space, for now).
pub struct Machine {
    pub cpu: Cpu,
    pub bus: SystemBus,
}

impl Default for Machine {
    fn default() -> Self {
        Self::new()
    }
}

impl Machine {
    pub fn new() -> Self {
        Machine {
            cpu: Cpu::new(),
            bus: SystemBus::new(),
        }
    }

    /// Install the boot ROM image (spec §3.1, mapped at `$FC0000`; its first 1 KB
    /// is also aliased at `$0` until the reset overlay is dropped).
    pub fn load_rom(&mut self, image: &[u8]) {
        self.bus.load_rom(image);
    }

    /// Poke bytes into RAM (bring-up helper).
    pub fn load_ram(&mut self, addr: u32, data: &[u8]) {
        self.bus.load_ram(addr, data);
    }

    /// Run the CPU reset sequence (reads the initial SSP and PC from vectors 0/1,
    /// which the overlay routes to ROM).
    pub fn reset(&mut self) {
        self.cpu.reset(&mut self.bus);
    }

    /// Execute a single instruction (or service one exception).
    pub fn step(&mut self) -> StepResult {
        self.cpu.step(&mut self.bus)
    }

    /// Step until the CPU leaves [`RunState::Running`] or `max_steps` is reached.
    /// Returns the number of steps actually executed.
    pub fn run(&mut self, max_steps: u64) -> u64 {
        let mut n = 0;
        while n < max_steps && self.cpu.state() == RunState::Running {
            self.step();
            n += 1;
        }
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system_bus::{ROM_BASE, ROM_BYTES};
    use clinker_m68k::{ConditionCode, Exception, RunState};

    const INITIAL_SSP: u32 = 0x00F0_0000; // somewhere in RAM
    const ENTRY: u32 = ROM_BASE + 0x400; // boot code entry, in ROM

    /// Build a ROM image with reset vectors (SSP, PC) and optional extra bytes at
    /// given ROM offsets.
    fn boot_rom(patches: &[(usize, &[u8])]) -> Vec<u8> {
        let mut rom = vec![0u8; ROM_BYTES];
        rom[0..4].copy_from_slice(&INITIAL_SSP.to_be_bytes());
        rom[4..8].copy_from_slice(&ENTRY.to_be_bytes());
        for (off, bytes) in patches {
            rom[*off..*off + bytes.len()].copy_from_slice(bytes);
        }
        rom
    }

    fn machine_with(patches: &[(usize, &[u8])]) -> Machine {
        let mut m = Machine::new();
        m.load_rom(&boot_rom(patches));
        m.reset();
        m
    }

    #[test]
    fn reset_loads_vectors_through_overlay() {
        let m = machine_with(&[]);
        assert_eq!(m.cpu.state(), RunState::Running);
        assert_eq!(m.cpu.regs.ssp, INITIAL_SSP);
        assert_eq!(m.cpu.regs.pc, ENTRY);
        assert!(m.cpu.regs.sr.supervisor());
        assert!(m.bus.reset_overlay());
    }

    #[test]
    fn executes_nop_then_moveq() {
        // 0x400: NOP ; 0x402: MOVEQ #$2A,D1
        let mut m = machine_with(&[(0x400, &[0x4E, 0x71, 0x72, 0x2A])]);
        m.step();
        assert_eq!(m.cpu.regs.pc, ENTRY + 2);
        m.step();
        assert_eq!(m.cpu.regs.d[1], 0x2A);
        assert!(!m.cpu.regs.sr.contains(ConditionCode::Zero));
    }

    #[test]
    fn illegal_opcode_vectors_through_table() {
        // vector 4 (offset 0x10) -> ROM_BASE+0x500 ; 0x400: ILLEGAL
        let handler = (ROM_BASE + 0x500).to_be_bytes();
        let mut m = machine_with(&[(0x10, &handler), (0x400, &[0x4A, 0xFC])]);
        let r = m.step();
        assert_eq!(r.exception, Some(Exception::IllegalInstruction));
        assert_eq!(m.cpu.regs.pc, ROM_BASE + 0x500);
        assert_eq!(m.cpu.regs.ssp, INITIAL_SSP - 6);
    }

    #[test]
    fn bra_short_loops() {
        let mut m = machine_with(&[(0x400, &[0x60, 0xFE])]); // BRA -2
        m.step();
        assert_eq!(m.cpu.regs.pc, ENTRY);
    }

    #[test]
    fn step_accounts_cycles() {
        let mut m = machine_with(&[(0x400, &[0x4E, 0x71])]); // NOP
        let before = m.cpu.cycles;
        let r = m.step();
        assert_eq!(r.cycles, 4); // single opcode-fetch bus cycle
        assert_eq!(m.cpu.cycles, before + 4);
    }
}
