//! Exception vectors.
//!
//! Only the pieces the skeleton actually exercises are wired up: the reset vector
//! fetch, and turning an illegal/unimplemented opcode or a [`crate::BusError`] into
//! a vectored jump. Group 0 (reset/bus error/address error) stack-frame formats are
//! stubbed — see `docs/ARCHITECTURE.md`.

/// The exception vector table occupies the first 1 KiB of the address space
/// (256 vectors × 4 bytes).
pub const VECTOR_TABLE_BYTES: u32 = 256 * 4;

/// A CPU exception, identified by the condition that raised it. Convert to a
/// vector number with [`Exception::vector`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exception {
    /// Vector 2. Carries the faulting address.
    BusError(u32),
    /// Vector 3. Word/long access to an odd address; carries that address.
    AddressError(u32),
    /// Vector 4.
    IllegalInstruction,
    /// Vector 5.
    DivideByZero,
    /// Vector 8.
    PrivilegeViolation,
    /// Vector 9.
    Trace,
    /// Vector 10. Opcode with bits 15..12 = 1010 ("line A").
    LineA,
    /// Vector 11. Opcode with bits 15..12 = 1111 ("line F").
    LineF,
    /// Vectors 32..47. `TRAP #n`, n = 0..=15.
    Trap(u8),
    /// Vectors 25..31. Auto-vectored interrupt at the given level, 1..=7.
    Interrupt(u8),
}

impl Exception {
    /// Vector *number* (multiply by 4 for the byte offset into the table).
    pub const fn vector(self) -> u8 {
        match self {
            Exception::BusError(_) => 2,
            Exception::AddressError(_) => 3,
            Exception::IllegalInstruction => 4,
            Exception::DivideByZero => 5,
            Exception::PrivilegeViolation => 8,
            Exception::Trace => 9,
            Exception::LineA => 10,
            Exception::LineF => 11,
            Exception::Trap(n) => 32 + (n & 0x0F),
            Exception::Interrupt(level) => 24 + (level & 0x07),
        }
    }

    /// Byte offset of this vector within the table.
    pub const fn vector_offset(self) -> u32 {
        (self.vector() as u32) * 4
    }
}
