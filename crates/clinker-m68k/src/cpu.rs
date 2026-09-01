//! The CPU object and the public step loop.

use crate::bus::{Bus, BusError, Width};
use crate::exception::Exception;
use crate::execute;
use crate::registers::{Registers, StatusRegister};
use crate::ADDRESS_MASK;

/// What the core is doing between steps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunState {
    /// Executing instructions normally.
    Running,
    /// `STOP` executed (not decoded yet) — waiting for an interrupt.
    Stopped,
    /// Double bus fault: the CPU has asserted HALT and will not run until reset.
    Halted,
}

/// Outcome of a single [`Cpu::step`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StepResult {
    /// Clock cycles this step is modelled as taking. Accurate per-instruction
    /// totals; not yet cycle-exact within the instruction (see `docs/ARCHITECTURE.md`).
    pub cycles: u32,
    /// The exception taken during this step, if any (vectoring has already
    /// happened; this is for tracing/debugging).
    pub exception: Option<Exception>,
}

/// An MC68000.
#[derive(Debug, Clone)]
pub struct Cpu {
    pub regs: Registers,
    /// Free-running cycle counter since construction.
    pub cycles: u64,
    /// Cycles accrued during the step currently in progress (so bus callbacks
    /// and `StepResult` agree).
    step_cycles: u32,
    state: RunState,
}

impl Default for Cpu {
    fn default() -> Self {
        Self::new()
    }
}

impl Cpu {
    pub fn new() -> Self {
        Cpu {
            regs: Registers::default(),
            cycles: 0,
            step_cycles: 0,
            state: RunState::Halted,
        }
    }

    pub fn state(&self) -> RunState {
        self.state
    }

    /// Perform the reset sequence: force SR, load SSP from vector 0 and PC from
    /// vector 1. A bus error on either fetch is a fatal double fault → [`RunState::Halted`].
    pub fn reset(&mut self, bus: &mut impl Bus) {
        self.regs = Registers::default();
        self.regs.sr = StatusRegister::RESET;
        self.step_cycles = 0;

        bus.advance(4);
        let ssp = bus.read_u32(0);
        bus.advance(8);
        let pc = bus.read_u32(4);
        bus.advance(4);

        match (ssp, pc) {
            (Ok(ssp), Ok(pc)) => {
                self.regs.ssp = ssp & ADDRESS_MASK;
                self.regs.pc = pc & ADDRESS_MASK;
                self.state = RunState::Running;
                self.cycles = self.cycles.wrapping_add(16);
            }
            _ => self.state = RunState::Halted,
        }
    }

    /// Execute one instruction (or service one pending exception / interrupt).
    pub fn step(&mut self, bus: &mut impl Bus) -> StepResult {
        if self.state != RunState::Running {
            return StepResult {
                cycles: 0,
                exception: None,
            };
        }
        self.step_cycles = 0;

        // Instruction-boundary interrupt sample.
        if let Some(level) = self.pending_interrupt(bus) {
            let exc = Exception::Interrupt(level);
            self.enter_exception(bus, exc);
            return self.finish_step(Some(exc));
        }

        let outcome = match self.fetch_opcode(bus) {
            Ok(opcode) => {
                let decoded = crate::decode::decode(opcode);
                execute::execute(self, bus, decoded)
            }
            Err(exc) => Err(exc),
        };

        let taken = match outcome {
            Ok(internal_cycles) => {
                self.tick(bus, internal_cycles);
                None
            }
            Err(exc) => {
                self.enter_exception(bus, exc);
                Some(exc)
            }
        };

        self.finish_step(taken)
    }

    fn finish_step(&mut self, exception: Option<Exception>) -> StepResult {
        self.cycles = self.cycles.wrapping_add(self.step_cycles as u64);
        StepResult {
            cycles: self.step_cycles,
            exception,
        }
    }

    /// Highest unmasked interrupt level currently asserted, if any. Level 7 is
    /// non-maskable. (Edge-triggered level-7 nuance is not modelled yet.)
    fn pending_interrupt(&self, bus: &mut impl Bus) -> Option<u8> {
        let level = bus.interrupt_level();
        if level == 7 || (level > 0 && level > self.regs.sr.interrupt_mask()) {
            Some(level)
        } else {
            None
        }
    }

    /// Charge `cycles` to the in-progress step and let the bus advance peripherals.
    pub(crate) fn tick(&mut self, bus: &mut impl Bus, cycles: u32) {
        self.step_cycles = self.step_cycles.saturating_add(cycles);
        bus.advance(cycles);
    }

    // --- Ticked memory access, shared with `execute` -------------------------

    pub(crate) fn mem_read_u16(&mut self, bus: &mut impl Bus, addr: u32) -> Result<u16, Exception> {
        self.tick(bus, Width::Word.bus_cycles());
        map_bus(bus.read_u16(addr & ADDRESS_MASK))
    }

    pub(crate) fn mem_read_u32(&mut self, bus: &mut impl Bus, addr: u32) -> Result<u32, Exception> {
        self.tick(bus, Width::Long.bus_cycles());
        map_bus(bus.read_u32(addr & ADDRESS_MASK))
    }

    pub(crate) fn mem_write_u16(
        &mut self,
        bus: &mut impl Bus,
        addr: u32,
        value: u16,
    ) -> Result<(), Exception> {
        self.tick(bus, Width::Word.bus_cycles());
        map_bus(bus.write_u16(addr & ADDRESS_MASK, value))
    }

    pub(crate) fn mem_write_u32(
        &mut self,
        bus: &mut impl Bus,
        addr: u32,
        value: u32,
    ) -> Result<(), Exception> {
        self.tick(bus, Width::Long.bus_cycles());
        map_bus(bus.write_u32(addr & ADDRESS_MASK, value))
    }

    /// Fetch the opcode word at PC and advance PC past it.
    fn fetch_opcode(&mut self, bus: &mut impl Bus) -> Result<u16, Exception> {
        let pc = self.regs.pc & ADDRESS_MASK;
        if pc & 1 != 0 {
            return Err(Exception::AddressError(pc));
        }
        let opcode = self.mem_read_u16(bus, pc)?;
        self.regs.pc = pc.wrapping_add(2) & ADDRESS_MASK;
        Ok(opcode)
    }

    /// Vector to an exception handler: stack a frame, load the new PC.
    ///
    /// Pushes the 68000 6-byte frame (SR, PC). The larger group-0 (bus/address
    /// error) frame is **not** built yet — those exceptions currently push the
    /// short frame too. See `docs/ARCHITECTURE.md` §10.2.
    fn enter_exception(&mut self, bus: &mut impl Bus, exc: Exception) {
        let return_pc = self.regs.pc;
        let saved_sr = self.regs.sr;

        // Enter supervisor mode, disable tracing.
        self.regs.sr.0 = (self.regs.sr.0 | StatusRegister::SUPERVISOR) & !StatusRegister::TRACE;
        if let Exception::Interrupt(level) = exc {
            self.regs.sr.0 =
                (self.regs.sr.0 & !StatusRegister::INT_MASK) | (((level as u16) & 0x7) << 8);
        }

        let mut sp = self.regs.ssp;
        sp = sp.wrapping_sub(4);
        let push_pc = self.mem_write_u32(bus, sp, return_pc);
        sp = sp.wrapping_sub(2);
        let push_sr = self.mem_write_u16(bus, sp, saved_sr.0);
        self.regs.ssp = sp & ADDRESS_MASK;

        if push_pc.is_err() || push_sr.is_err() {
            // Fault while stacking a fault: HALT.
            self.state = RunState::Halted;
            self.tick(bus, 8);
            return;
        }

        match self.mem_read_u32(bus, exc.vector_offset()) {
            Ok(handler) => {
                self.regs.pc = handler & ADDRESS_MASK;
                // Internal overhead beyond the three bus cycles already charged.
                self.tick(bus, 18);
            }
            Err(_) => {
                self.state = RunState::Halted;
                self.tick(bus, 8);
            }
        }
    }
}

fn map_bus<T>(r: Result<T, BusError>) -> Result<T, Exception> {
    r.map_err(|e| match e {
        BusError::Address(a) => Exception::AddressError(a),
        BusError::Unmapped(a) => Exception::BusError(a),
    })
}
