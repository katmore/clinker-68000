//! MC68000 CPU core.
//!
//! Scope of this crate right now: a *skeleton* — register file, a `Bus` trait for
//! all memory access, reset behaviour, and a `step()` fetch/decode/execute loop that
//! recognises a handful of instructions and raises the Illegal Instruction exception
//! for everything else. This is enough to single-step reset vectors and prove the
//! loop shape; it is **not** a working 68000 yet.
//!
//! Deliberate non-goals for now (tracked in `docs/ARCHITECTURE.md`):
//!   * full instruction set / effective-address decode
//!   * cycle-exact timing and prefetch pipeline modelling
//!   * bus error / address error stack frame layout details
//!   * peripherals, interrupts beyond the level-compare stub
//!
//! The CPU is big-endian and operates on a 24-bit external address bus (the top byte
//! of any 32-bit address register is ignored on bus cycles — see [`Bus`]).

#![forbid(unsafe_code)]

mod bus;
mod cpu;
mod decode;
mod exception;
mod execute;
mod registers;

pub use bus::{Bus, BusError, Width};
pub use cpu::{Cpu, RunState, StepResult};
pub use exception::{Exception, VECTOR_TABLE_BYTES};
pub use registers::{ConditionCode, Registers, StatusRegister};

/// External clock the Clinker wires to the 68000 (spec §3). Not used by the core's
/// logic yet; kept here so callers have a single source of truth.
pub const CLOCK_HZ: u32 = 8_000_000;

/// Mask applied to every address before it reaches the [`Bus`]: the 68000 only
/// drives A1..A23 externally (A0 is encoded in the data strobes), but for a
/// byte-addressable emulator bus we expose all 24 bits and just mask the top 8.
pub const ADDRESS_MASK: u32 = 0x00FF_FFFF;
