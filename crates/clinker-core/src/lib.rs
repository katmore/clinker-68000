//! The Clinker 68000 machine.
//!
//! This crate owns the 24-bit address map ([`SystemBus`]) and assembles the CPU
//! with it ([`Machine`]). It is the surface the (separate) GTK C++ front end will
//! eventually drive.
//!
//! ## Scope right now
//!
//! CPU + address decoder, no peripherals. The map follows
//! `docs/CLINKER-68000-SPEC.md` §3.1 — RAM low, then boot ROM, a display window,
//! and an I/O page in the top 256 KB, plus the reset ROM overlay. The display and
//! I/O windows are inert stubs; the CRTC text buffer, FDC + drive-select
//! expander, NCR 5380, DUART, Centronics, ACIA/OPL2 MIDI, and keyboard link all
//! come later (spec §4–§9).
//!
//! ## Front-end boundary (not implemented this pass)
//!
//! When peripherals land, the front end needs exactly these operations against
//! `Machine`, and nothing else:
//!   * read the 80×25 text buffer (+ cursor) for display
//!   * inject keyboard scan-code / wheel-delta packets
//!   * mount / unmount a floppy image per bay (0..=9)
//!   * mount / unmount a SCSI (T-38) target image
//!   * step the machine and read back cycle/timing info
//!
//! Keeping that list short is a design constraint, not just a description.

mod machine;
mod system_bus;

pub use machine::Machine;
pub use system_bus::{SystemBus, IO_BASE, RAM_BYTES, RAM_END, ROM_BASE, ROM_BYTES, SYSCTL_ADDR};

pub use clinker_m68k;
