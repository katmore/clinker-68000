# Clinker 68000 — Spec Sheet (Draft v0.2)

Not a real HP 9836. Loosely derived from it — CPU/RAM/display class match, everything else is free to diverge where it makes emulation easier.

## 1. Hard Constraints (must hold)

| Subsystem | Spec |
|---|---|
| CPU | Motorola 68000, 8 MHz (matches HP 9836 reference) |
| RAM | 16 MB stock (68000's full 24-bit address space, no expansion needed) |
| Display | 80×25 monochrome text mode |
| Storage | 10-bay 5.25" floppy array |
| SCSI | "T-38" port + controller |
| Serial | RS-232 |
| Parallel | Centronics |
| MIDI | Onboard synthesis — standalone playback, no external synth required |

## 2. Everything Else — Novel / Free Choice

Bus topology, disk encoding, SCSI/display/keyboard controller chips, connector types, mechanical layout. Only constraint: it has to be a real, period-plausible (mid-80s) implementation choice, not vaporware — since the whole point is that this could theoretically have been built and is now realistically emulable.

## 3. CPU & Memory

- **CPU:** MC68000 @ 8 MHz, 16-bit external bus, no MMU (period-correct — the real 9836 didn't ship with one either).
- **RAM:** Flat address space, 16 MB stock — the 68000's full 24-bit address bus (2^24 = 16,777,216 bytes), populated from day one, no expansion slots needed. All 16 MB of RAM is physically present; I/O and ROM simply win the address decode where they overlap it, so a sliver of RAM at the top of the space is unreachable. That is not "lost" capacity in any meaningful sense (~1.5%) and does not change the "16 MB stock" figure.

### 3.1 Address map (decided)

RAM low, everything else in the top 256 KB. Contiguous user RAM: **15.75 MB** (`$000000`–`$FBFFFF`).

| Range | Size | Contents |
|---|---|---|
| `$000000`–`$FBFFFF` | 15.75 MB | Main RAM. Exception vector table lives at `$000000`. |
| `$FC0000`–`$FDFFFF` | 128 KB | Boot / monitor ROM. *(Size provisional — see Open Questions.)* |
| `$FE0000`–`$FEFFFF` | 64 KB | Text display buffer + MC6845-style CRTC registers. |
| `$FF0000`–`$FFFFFF` | 64 KB | Memory-mapped I/O: FDC + drive-select expander, NCR 5380, 68681 DUART, Centronics, dual 6850 ACIA, OPL2, keyboard/wheel link. Detailed register layout TBD. |

**Reset overlay:** the 68000 reads its initial SSP and PC from `$000000`–`$000007`, which is RAM (indeterminate at power-on). On reset the boot ROM is therefore *also* aliased at `$000000`. Early boot code copies the vector table into low RAM and then clears the overlay via a bit in a system-control register in the I/O page, after which `$000000` is ordinary RAM. (Same technique as the Amiga / Atari ST / Mac.)

**Display buffer:** dedicated static RAM inside the display window, not a reserved slice of main RAM — matches how MC6845 character-mode cards worked and keeps display access off the main-RAM timing path.

- **Bus:** Novel. Everything is memory-mapped on the 68000's own address/data bus — no backplane, no card slots, just address-decode ranges (table above). No bus arbitration modelled (no DMA masters other than, later, the FDC — which will be handled as a coarse CPU stall, not full bus sharing).

## 4. Display

- 80×25 monochrome text mode (hard constraint).
- **Controller:** Novel, but the easiest-to-emulate period-correct choice is an MC6845-style CRTC + character generator ROM (same family used in MDA/Hercules cards) — a handful of registers (cursor position, start address, sync timing) rather than a full bitmap framebuffer state machine.

## 5. Storage — 10-Bay Floppy Array

- 10 physical 5.25" bays, one drive addressable/active at a time (matches the "old-school 2-drive Apple array" behavior you described — convenience slots, not simultaneous R/W).
- **Disk format:** Standard, not HP's proprietary format — recommend plain IBM-compatible MFM 5.25" (360 KB or 1.2 MB), since that gives you off-the-shelf tooling for building disk images and no need to reverse-engineer HP's LIF filesystem.
- **Controller:** NEC µPD765/8272A-style FDC (industry-standard, well-documented, widely emulated already — good reference implementations exist). Native drive-select is only 2 bits (4 drives); to hit 10 you add a **novel drive-select expander** — a small multiplexer sitting between the FDC's drive-select lines and the bay connectors, fed by a memory-mapped "active bay" register. Software picks the bay, expander routes select/motor lines to it. This is the proprietary-bus part — doesn't need to touch SCSI at all, as you said.

## 6. SCSI — "T-38" Port

"T-38" is Clinker's own designation — a proprietary DB25 pinout for the SCSI port (not a historical/industry standard).

- **Controller:** NCR 5380 — the most period-correct, well-documented, and already-emulated SCSI controller chip of the era (Mac Plus, Amiga 2000, early Sun gear all used it or a close variant). Good existing reference implementations to build your Rust core against.
- **Connector:** DB25, Clinker-proprietary pinout (not standard SCSI-1 DB25, which reused Mac-style signal assignments) — worth documenting the pin assignment explicitly once you get to it, since "DB25" alone won't tell future-you or anyone else which signals live where.

## 7. Serial / Parallel

- **RS-232:** Simple UART — 8251-style USART or a 68681 DUART if you want two serial channels for cheap (one could double as the keyboard/wheel link, see below).
- **Centronics:** Byte-wide parallel port with strobe/ack/busy handshake — trivially emulated as a memory-mapped register with a couple of status bits. No need for a dedicated PIA chip in emulation, though one (6821-style) is the period-correct hardware answer if you want mechanical plausibility.

## 8. Keyboard + Wheel

Confirmed the HP reference machines actually did have a rotary knob (an RPG — rotary pulse generator — readable from BASIC) on their keyboards, connected via HP-HIL. HP-HIL itself is a pain to emulate (proprietary daisy-chain protocol, HP-specific silicon). Recommend avoiding it entirely:

- **Keyboard protocol:** A simple synchronous serial scan-code link — same shape as the IBM PC/XT keyboard protocol (clock + data, scan codes on make/break) or the classic Mac/Apple keyboard link. Either is a two-wire shift-register protocol, dead simple to emulate.
- **Wheel:** Piggyback it on the same serial link as a second packet type (relative delta bytes, like a period serial mouse) rather than giving it a fully separate port. Keeps you at one keyboard device to emulate instead of two. If you'd rather it be fully independent (e.g., its own mini-UART on the RS-232 controller), that's also easy — just a design preference call.

## 9. MIDI (Onboard Synthesis)

Decided: **Option B** — self-contained playback, not just a data pipe to external gear.

- **I/O layer:** Dual 6850 ACIA, one IN one OUT/THRU — same approach the Atari ST used. Nearly free to add, and gives you MIDI THRU to real external gear as a side effect even though it's not the primary playback path. Fixed 31.25 kbaud, 8-N-1 — no need for cycle-accurate UART timing in emulation, model it as a byte queue at a fixed rate.
- **Synthesis:** Yamaha OPL2-class FM chip. Well-documented, period-correct for mid-80s, and FM synthesis emulation is a solved problem with existing reference cores to study rather than build from scratch.
- **Driver:** Small onboard routine consumes incoming MIDI messages (Note On/Off, Program Change, etc. — either live over the ACIA or read from a stored sequence) and maps them to OPL2 register writes (operator/envelope/feedback settings per voice).
- **Emulation split:** ACIA side is trivial (byte queue, fixed rate). OPL2 side is the real work — you're emulating an actual synthesis engine (operators, envelopes, feedback loops), not just data transport. Budget more time here than anywhere else in the audio path.

## 10. Emulation Notes

### 10.1 Timing fidelity (decided)

Target: **clock-driven core with accurate per-instruction cycle totals.** Bus accesses
tick a shared clock and the execution engine is re-entrant, but the 68000's prefetch
queue (IRC/IR/IRD) is *not* modelled yet. Per-instruction cycle counts follow the
Motorola figures / yacht.txt; interrupts are sampled at instruction boundaries.

Full cycle-exact + prefetch remains a possible later upgrade — it is additive on top
of the clock-driven design, not a rewrite — but is only worth doing if a specific
piece of software (disk copy-protection, a CPU-speed-calibrated delay loop) is shown
to need it. The text-mode display, command-driven FDC, and fixed-rate MIDI byte queue
do not.

### 10.2 Exception model (decided)

- MC68000 (not 010/020): groups 1 & 2 push the **6-byte frame** (SR word + PC long,
  no format word).
- Bus error / address error (group 0) push the 68000's **14-byte frame**, filled
  **diagnostic-grade**: fault address, R/W and function-code bits accurate; the
  undocumented internal-state bits zeroed. Because the machine has **no MMU**, these
  exceptions are a fatal path (handler diagnoses and reboots) — clean instruction
  restart after bus error is *not* a goal (the 68000 hardware can't do it reliably
  either).
- Exception priority / simultaneity logic lands with interrupts, not before.

### 10.3 Reference implementations

68000 core, FDC, CRTC, NCR 5380, and the PC/XT-style keyboard protocol all have open
reference implementations/docs to study before writing the Rust versions. See
`docs/ARCHITECTURE.md` for the specific list and how each is used. No third-party
code is vendored.

### 10.4 Front-end boundary

GTK C++ front end (separate repo) talks to the Rust core for exactly: text-buffer +
cursor read, keyboard/wheel packet injection, floppy image mount per bay (0–9), SCSI
(T-38) image mount, and stepping the machine. Keeping that list short is a constraint.

## Open Questions / Unverified Assumptions

- **68000 clock speed** — assumed 8 MHz to match the HP 9836 reference exactly, since you listed CPU as a hard constraint. If clock speed itself is flexible and only the CPU family is hard, that opens up performance headroom.
- **Wheel packet routing** — assumed piggybacking on the keyboard link is fine per your "keep it simple" ask. If the wheel needs to be independently hot-pluggable or separately addressable in software, it should get its own UART channel instead.
- **Boot ROM size / contents** — map (§3.1) provisionally reserves 128 KB at `$FC0000`. Depends on whether BASIC (or whatever the primary language/OS is) is resident in ROM or loaded from floppy like the HP 9836's "BASIC Workstation" disc. If resident, ROM grows to 256–512 KB and the map shifts down. Blocks nothing until we write/import firmware.
- **I/O register layout** — the `$FF0000`–`$FFFFFF` page is allocated as a block; the per-chip register offsets within it (FDC, 5380, DUART, Centronics, ACIA ×2, OPL2, keyboard, system-control/overlay) are not assigned yet. Needed before the first peripheral.
- **System-control register** — the reset-overlay-clear bit needs a home (address + bit position) in the I/O page. Part of the I/O layout question above.
