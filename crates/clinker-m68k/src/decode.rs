//! Opcode decode.
//!
//! This is a *stub table*. It recognises the small set of instructions the
//! skeleton needs to demonstrate the fetch/decode/execute loop and reports
//! everything else as [`Decoded::Unimplemented`] so `execute` can raise Illegal
//! Instruction. Adding the real decoder means expanding [`decode`] into the full
//! 16-bit opcode map (likely a generated 65 536-entry jump table, the approach
//! Musashi and Moira both take — see `docs/ARCHITECTURE.md`).

/// A decoded instruction, ready for [`crate::execute`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decoded {
    /// `NOP` (0x4E71).
    Nop,
    /// `RESET` (0x4E70) — assert the reset line to peripherals. No peripherals
    /// yet, so this is currently a privileged no-op.
    Reset,
    /// `RTS` (0x4E75).
    Rts,
    /// `TRAP #n` (0x4E4n).
    Trap(u8),
    /// `MOVEQ #imm,Dn` (0111 rrr0 dddddddd).
    Moveq { reg: u8, value: i8 },
    /// `BRA <label>` with an 8-bit displacement (0110 0000 dddddddd, disp != 0).
    /// 16-/32-bit displacement forms are not decoded yet.
    BraShort { disp: i8 },
    /// `ILLEGAL` (0x4AFC) — architecturally defined to raise vector 4.
    Illegal,
    /// Opcode in the "line A" ($Axxx) space.
    LineA,
    /// Opcode in the "line F" ($Fxxx) space.
    LineF,
    /// A real opcode we simply haven't implemented; treated as Illegal Instruction.
    Unimplemented(u16),
}

/// Decode a single opcode word. The 68000 is fixed-16-bit-opcode with optional
/// trailing extension words; this stub only handles opcodes with no extensions
/// (or ignores them), which is fine for the current instruction set.
pub fn decode(opcode: u16) -> Decoded {
    match opcode {
        0x4E71 => Decoded::Nop,
        0x4E70 => Decoded::Reset,
        0x4E75 => Decoded::Rts,
        0x4AFC => Decoded::Illegal,
        0x4E40..=0x4E4F => Decoded::Trap((opcode & 0x0F) as u8),
        _ if opcode & 0xF000 == 0xA000 => Decoded::LineA,
        _ if opcode & 0xF000 == 0xF000 => Decoded::LineF,
        _ if opcode & 0xF100 == 0x7000 => Decoded::Moveq {
            reg: ((opcode >> 9) & 0x07) as u8,
            value: opcode as i8,
        },
        _ if opcode & 0xFF00 == 0x6000 => {
            let disp = opcode as i8;
            if disp == 0 {
                // 16-bit displacement form — needs an extension word we don't read yet.
                Decoded::Unimplemented(opcode)
            } else {
                Decoded::BraShort { disp }
            }
        }
        _ => Decoded::Unimplemented(opcode),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_stub_set() {
        assert_eq!(decode(0x4E71), Decoded::Nop);
        assert_eq!(decode(0x4E75), Decoded::Rts);
        assert_eq!(decode(0x4E4A), Decoded::Trap(0xA));
        assert_eq!(decode(0x7204), Decoded::Moveq { reg: 1, value: 4 });
        assert_eq!(decode(0x70FF), Decoded::Moveq { reg: 0, value: -1 });
        assert_eq!(decode(0x60FE), Decoded::BraShort { disp: -2 });
        assert_eq!(decode(0xA123), Decoded::LineA);
        assert_eq!(decode(0xFEDC), Decoded::LineF);
    }

    #[test]
    fn bra_word_form_not_yet_decoded() {
        assert_eq!(decode(0x6000), Decoded::Unimplemented(0x6000));
    }

    #[test]
    fn unknown_opcode_is_unimplemented() {
        assert_eq!(decode(0xD280), Decoded::Unimplemented(0xD280)); // ADD.l
    }
}
