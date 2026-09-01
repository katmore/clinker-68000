//! Register file: eight data registers, seven address registers plus the banked
//! stack pointer, the program counter, and the status register.

/// The 68000 condition codes (low byte of the SR).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConditionCode {
    Carry,
    Overflow,
    Zero,
    Negative,
    Extend,
}

impl ConditionCode {
    const fn bit(self) -> u16 {
        match self {
            ConditionCode::Carry => 1 << 0,
            ConditionCode::Overflow => 1 << 1,
            ConditionCode::Zero => 1 << 2,
            ConditionCode::Negative => 1 << 3,
            ConditionCode::Extend => 1 << 4,
        }
    }
}

/// The 16-bit status register. System byte (bits 8..15) holds the interrupt mask,
/// the supervisor (S) bit and the trace (T) bit; user byte holds the CCR.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StatusRegister(pub u16);

impl StatusRegister {
    pub const TRACE: u16 = 1 << 15;
    pub const SUPERVISOR: u16 = 1 << 13;
    pub const INT_MASK: u16 = 0b111 << 8;

    /// SR value the CPU forces on reset: supervisor, trace off, interrupt mask 7.
    pub const RESET: StatusRegister = StatusRegister(Self::SUPERVISOR | Self::INT_MASK);

    pub fn supervisor(self) -> bool {
        self.0 & Self::SUPERVISOR != 0
    }

    pub fn trace(self) -> bool {
        self.0 & Self::TRACE != 0
    }

    /// Interrupt priority mask, 0..=7.
    pub fn interrupt_mask(self) -> u8 {
        ((self.0 & Self::INT_MASK) >> 8) as u8
    }

    pub fn contains(self, cc: ConditionCode) -> bool {
        self.0 & cc.bit() != 0
    }

    pub fn set(&mut self, cc: ConditionCode, on: bool) {
        if on {
            self.0 |= cc.bit();
        } else {
            self.0 &= !cc.bit();
        }
    }

    /// Condition-code register (low byte) in isolation.
    pub fn ccr(self) -> u8 {
        self.0 as u8
    }
}

/// Complete architectural state of the CPU (no cached prefetch words yet).
///
/// `Default` is all-zero, which is also the power-on state before reset loads the
/// vectors; [`crate::Cpu::reset`] additionally forces the SR.
#[derive(Debug, Clone, Default)]
pub struct Registers {
    /// D0..D7.
    pub d: [u32; 8],
    /// A0..A6. A7 is [`Self::usp`]/[`Self::ssp`], selected by the S bit.
    pub a: [u32; 7],
    /// User stack pointer (A7 when S = 0).
    pub usp: u32,
    /// Supervisor stack pointer (A7 when S = 1).
    pub ssp: u32,
    pub pc: u32,
    pub sr: StatusRegister,
}

impl Registers {
    /// Read the active stack pointer (A7) for the current privilege level.
    pub fn sp(&self) -> u32 {
        if self.sr.supervisor() {
            self.ssp
        } else {
            self.usp
        }
    }

    /// Write the active stack pointer (A7).
    pub fn set_sp(&mut self, value: u32) {
        if self.sr.supervisor() {
            self.ssp = value;
        } else {
            self.usp = value;
        }
    }

    /// Address register by index, 0..=7, with 7 mapping to the active SP.
    pub fn addr(&self, index: usize) -> u32 {
        match index {
            0..=6 => self.a[index],
            7 => self.sp(),
            _ => panic!("address register index out of range: {index}"),
        }
    }

    pub fn set_addr(&mut self, index: usize, value: u32) {
        match index {
            0..=6 => self.a[index] = value,
            7 => self.set_sp(value),
            _ => panic!("address register index out of range: {index}"),
        }
    }
}
