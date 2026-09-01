//! The Clinker 68000 address decoder.
//!
//! Implements the map from `docs/CLINKER-68000-SPEC.md` §3.1: 15.75 MB of RAM
//! low, then boot ROM, a display window, and an I/O page in the top 256 KB.
//!
//! Peripherals are **not** implemented here yet — the display and I/O windows are
//! inert stubs (reads return `$FF`, writes are dropped) except for the one bit of
//! the system-control register that clears the reset overlay. Wiring real
//! peripheral models behind those ranges is a later pass.

use clinker_m68k::{Bus, BusError};

/// Populated RAM: the full 24-bit space (spec §3). I/O/ROM decode wins where they
/// overlap the top, so not all of it is reachable.
pub const RAM_BYTES: usize = 16 * 1024 * 1024;

/// First address that is no longer RAM.
pub const RAM_END: u32 = 0xFC_0000;

pub const ROM_BASE: u32 = 0xFC_0000;
pub const ROM_BYTES: usize = 128 * 1024;

pub const DISPLAY_BASE: u32 = 0xFE_0000;
pub const DISPLAY_END: u32 = 0xFF_0000;

pub const IO_BASE: u32 = 0xFF_0000;
pub const IO_END: u32 = 0x100_0000;

/// System-control register (provisional — see spec Open Questions). Bit 0 is the
/// reset-overlay enable: set at reset, software writes 0 to drop the overlay.
pub const SYSCTL_ADDR: u32 = IO_BASE;
const SYSCTL_OVERLAY_BIT: u8 = 0x01;

/// During reset the low 1 KB (the CPU vector table) reads from the ROM's vector
/// image instead of RAM, so the 68000 can fetch a valid SSP/PC from `$0`.
const VECTOR_ALIAS_END: u32 = 0x400;

/// Where a decoded address actually lands.
enum Target {
    Ram(usize),
    Rom(usize),
    Display,
    Io(u32),
    /// Defensive only: the map above covers the whole 24-bit space, so nothing
    /// currently decodes here. Kept so adding a hole later is a one-line change.
    Unmapped,
}

/// CPU-side view of the whole machine's memory and I/O.
pub struct SystemBus {
    ram: Vec<u8>,
    rom: Vec<u8>,
    reset_overlay: bool,
}

impl Default for SystemBus {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemBus {
    pub fn new() -> Self {
        SystemBus {
            ram: vec![0; RAM_BYTES],
            rom: vec![0; ROM_BYTES],
            reset_overlay: true,
        }
    }

    /// Load the boot ROM image (truncated or zero-padded to [`ROM_BYTES`]).
    pub fn load_rom(&mut self, image: &[u8]) {
        let n = image.len().min(ROM_BYTES);
        self.rom[..n].copy_from_slice(&image[..n]);
        for b in &mut self.rom[n..] {
            *b = 0;
        }
    }

    /// Write bytes straight into RAM (bring-up / test helper, not a bus cycle).
    pub fn load_ram(&mut self, addr: u32, data: &[u8]) {
        let start = addr as usize;
        self.ram[start..start + data.len()].copy_from_slice(data);
    }

    /// Is the reset ROM overlay still active?
    pub fn reset_overlay(&self) -> bool {
        self.reset_overlay
    }

    pub fn ram(&self) -> &[u8] {
        &self.ram
    }

    fn decode(&self, addr: u32) -> Target {
        let addr = addr & 0x00FF_FFFF;

        if self.reset_overlay && addr < VECTOR_ALIAS_END {
            return Target::Rom(addr as usize);
        }
        match addr {
            a if a < RAM_END => Target::Ram(a as usize),
            a if (ROM_BASE..ROM_BASE + ROM_BYTES as u32).contains(&a) => {
                Target::Rom((a - ROM_BASE) as usize)
            }
            a if (DISPLAY_BASE..DISPLAY_END).contains(&a) => Target::Display,
            a if (IO_BASE..IO_END).contains(&a) => Target::Io(a - IO_BASE),
            _ => Target::Unmapped,
        }
    }
}

impl Bus for SystemBus {
    fn read_u8(&mut self, addr: u32) -> Result<u8, BusError> {
        match self.decode(addr) {
            Target::Ram(i) => Ok(self.ram[i]),
            Target::Rom(i) => Ok(self.rom[i]),
            // Inert display window reads as open bus.
            Target::Display => Ok(0xFF),
            Target::Io(off) => Ok(self.read_io(off)),
            Target::Unmapped => Err(BusError::Unmapped(addr & 0x00FF_FFFF)),
        }
    }

    fn write_u8(&mut self, addr: u32, value: u8) -> Result<(), BusError> {
        match self.decode(addr) {
            Target::Ram(i) => {
                self.ram[i] = value;
                Ok(())
            }
            // Writes to the vector alias fall through to the RAM underneath, so
            // boot code can build the real vector table before dropping the overlay.
            Target::Rom(_) if self.reset_overlay && (addr & 0x00FF_FFFF) < VECTOR_ALIAS_END => {
                self.ram[(addr & 0x00FF_FFFF) as usize] = value;
                Ok(())
            }
            // ROM and inert regions: writes are dropped, not faulted.
            Target::Rom(_) | Target::Display => Ok(()),
            Target::Io(off) => {
                self.write_io(off, value);
                Ok(())
            }
            Target::Unmapped => Err(BusError::Unmapped(addr & 0x00FF_FFFF)),
        }
    }
}

impl SystemBus {
    fn read_io(&self, offset: u32) -> u8 {
        match offset {
            0 => {
                if self.reset_overlay {
                    SYSCTL_OVERLAY_BIT
                } else {
                    0
                }
            }
            // No peripherals yet.
            _ => 0xFF,
        }
    }

    fn write_io(&mut self, offset: u32, value: u8) {
        if offset == 0 && value & SYSCTL_OVERLAY_BIT == 0 {
            self.reset_overlay = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_routes_low_reads_to_rom_then_ram() {
        let mut b = SystemBus::new();
        let mut rom = vec![0u8; ROM_BYTES];
        rom[0..4].copy_from_slice(&[0x11, 0x22, 0x33, 0x44]);
        b.load_rom(&rom);
        b.load_ram(0, &[0xAA, 0xBB, 0xCC, 0xDD]);

        // Overlay active: low addresses read the ROM vector image...
        assert_eq!(b.read_u32(0).unwrap(), 0x11223344);
        // ...but writes fall through to the RAM underneath.
        b.write_u16(0, 0x5566).unwrap();
        assert_eq!(b.ram()[0..2], [0x55, 0x66]);

        // Drop the overlay via the system-control register.
        b.write_u8(SYSCTL_ADDR, 0).unwrap();
        assert!(!b.reset_overlay());
        assert_eq!(b.read_u32(0).unwrap(), 0x5566CCDD);
    }

    #[test]
    fn rom_is_read_only() {
        let mut b = SystemBus::new();
        b.write_u8(SYSCTL_ADDR, 0).unwrap(); // clear overlay
        let mut rom = vec![0u8; ROM_BYTES];
        rom[0] = 0x99;
        b.load_rom(&rom);

        assert_eq!(b.read_u8(ROM_BASE).unwrap(), 0x99);
        b.write_u8(ROM_BASE, 0x00).unwrap(); // dropped, not faulted
        assert_eq!(b.read_u8(ROM_BASE).unwrap(), 0x99);
    }

    #[test]
    fn display_and_io_windows_are_inert() {
        let mut b = SystemBus::new();
        assert_eq!(b.read_u8(DISPLAY_BASE).unwrap(), 0xFF);
        b.write_u8(DISPLAY_BASE, 0x42).unwrap();
        assert_eq!(b.read_u8(DISPLAY_BASE).unwrap(), 0xFF);
        // I/O offset 1+ has no peripheral behind it yet.
        assert_eq!(b.read_u8(IO_BASE + 4).unwrap(), 0xFF);
    }
}
