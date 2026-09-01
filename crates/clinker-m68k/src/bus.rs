//! The CPU's view of the outside world.
//!
//! Everything the 68000 touches — RAM, ROM, memory-mapped I/O — goes through [`Bus`].
//! The CPU core has no other way to reach memory, which keeps `clinker-m68k`
//! completely free of machine specifics.
//!
//! ## Clock-driven
//!
//! The core is clock-driven (see `docs/ARCHITECTURE.md` §10.1): the CPU calls
//! [`Bus::advance`] as it works through an instruction, so an implementation that
//! owns peripherals sees monotonic time *interleaved* with CPU activity rather
//! than one lump at the end. The prefetch pipeline isn't modelled yet, so the
//! `advance` calls are currently at bus-transaction and internal-cycle-group
//! granularity, not per clock — but the shape is right for tightening later.

/// Operand width for a bus transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Width {
    Byte,
    Word,
    Long,
}

impl Width {
    /// Number of bytes moved.
    pub const fn bytes(self) -> u32 {
        match self {
            Width::Byte => 1,
            Width::Word => 2,
            Width::Long => 4,
        }
    }

    /// Clock periods for one access of this width on the 68000's 16-bit bus:
    /// byte and word are a single 4-clock bus cycle; long is two.
    pub const fn bus_cycles(self) -> u32 {
        match self {
            Width::Byte | Width::Word => 4,
            Width::Long => 8,
        }
    }
}

/// Raised by a [`Bus`] implementation when a transaction cannot be completed.
///
/// The CPU turns this into the appropriate exception (bus error / address error).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BusError {
    /// Nothing decodes at this address (open bus / unmapped region).
    Unmapped(u32),
    /// A word/long access was attempted at an odd address.
    Address(u32),
}

/// Memory + I/O access for the CPU.
///
/// Addresses passed in are already masked to 24 bits by the CPU
/// (see [`crate::ADDRESS_MASK`]). All values are big-endian. Implementations only
/// have to provide byte access; the wider helpers compose it. `advance` and
/// `interrupt_level` have no-op defaults so a plain RAM array can implement the
/// trait without caring about time or interrupts.
pub trait Bus {
    fn read_u8(&mut self, addr: u32) -> Result<u8, BusError>;
    fn write_u8(&mut self, addr: u32, value: u8) -> Result<(), BusError>;

    /// Advance peripheral/bus time by `cycles` clock periods. The CPU calls this
    /// as it executes; the default ignores it.
    fn advance(&mut self, cycles: u32) {
        let _ = cycles;
    }

    /// Current interrupt priority asserted on IPL2..0, `0` meaning none. Sampled
    /// by the CPU at each instruction boundary and compared against the SR mask.
    /// The default (no interrupt sources) is `0`.
    fn interrupt_level(&mut self) -> u8 {
        0
    }

    fn read_u16(&mut self, addr: u32) -> Result<u16, BusError> {
        if addr & 1 != 0 {
            return Err(BusError::Address(addr));
        }
        let hi = self.read_u8(addr)? as u16;
        let lo = self.read_u8(addr.wrapping_add(1))? as u16;
        Ok((hi << 8) | lo)
    }

    fn write_u16(&mut self, addr: u32, value: u16) -> Result<(), BusError> {
        if addr & 1 != 0 {
            return Err(BusError::Address(addr));
        }
        self.write_u8(addr, (value >> 8) as u8)?;
        self.write_u8(addr.wrapping_add(1), value as u8)
    }

    fn read_u32(&mut self, addr: u32) -> Result<u32, BusError> {
        let hi = self.read_u16(addr)? as u32;
        let lo = self.read_u16(addr.wrapping_add(2))? as u32;
        Ok((hi << 16) | lo)
    }

    fn write_u32(&mut self, addr: u32, value: u32) -> Result<(), BusError> {
        self.write_u16(addr, (value >> 16) as u16)?;
        self.write_u16(addr.wrapping_add(2), value as u16)
    }
}
