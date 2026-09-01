//! Per-instruction execution for the handful of opcodes [`crate::decode`] knows.
//!
//! Each arm returns the instruction's *internal* (non-bus) cycle count — the
//! opcode fetch and any data accesses are charged by the ticked helpers on
//! [`Cpu`] as they happen. Returning an [`Exception`] hands control to the
//! vectoring path, which does its own cycle accounting.

use crate::bus::Bus;
use crate::cpu::Cpu;
use crate::decode::Decoded;
use crate::exception::Exception;
use crate::registers::{ConditionCode, StatusRegister};
use crate::ADDRESS_MASK;

pub(crate) fn execute(
    cpu: &mut Cpu,
    bus: &mut impl Bus,
    decoded: Decoded,
) -> Result<u32, Exception> {
    match decoded {
        Decoded::Nop => Ok(0),

        Decoded::Reset => {
            require_supervisor(cpu)?;
            // Would pulse the peripheral reset line; nothing to pulse yet.
            Ok(128)
        }

        Decoded::Rts => {
            let sp = cpu.regs.sp();
            let pc = cpu.mem_read_u32(bus, sp)?;
            cpu.regs.set_sp(sp.wrapping_add(4));
            cpu.regs.pc = pc & ADDRESS_MASK;
            Ok(4)
        }

        Decoded::Trap(n) => Err(Exception::Trap(n)),

        Decoded::Moveq { reg, value } => {
            let v = value as i32 as u32;
            cpu.regs.d[reg as usize] = v;
            cpu.regs.sr.set(ConditionCode::Negative, (v as i32) < 0);
            cpu.regs.sr.set(ConditionCode::Zero, v == 0);
            cpu.regs.sr.set(ConditionCode::Overflow, false);
            cpu.regs.sr.set(ConditionCode::Carry, false);
            Ok(0)
        }

        Decoded::BraShort { disp } => {
            // Displacement is relative to PC *after* the opcode word, which is
            // exactly where PC sits now.
            cpu.regs.pc = cpu.regs.pc.wrapping_add(disp as i32 as u32) & ADDRESS_MASK;
            Ok(6)
        }

        Decoded::Illegal | Decoded::Unimplemented(_) => Err(Exception::IllegalInstruction),
        Decoded::LineA => Err(Exception::LineA),
        Decoded::LineF => Err(Exception::LineF),
    }
}

fn require_supervisor(cpu: &Cpu) -> Result<(), Exception> {
    if cpu.regs.sr.0 & StatusRegister::SUPERVISOR != 0 {
        Ok(())
    } else {
        Err(Exception::PrivilegeViolation)
    }
}
