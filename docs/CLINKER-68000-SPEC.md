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
- **RAM:** Flat address space, 16 MB stock — the 68000's full 24-bit address bus (2^24 = 16,777,216 bytes), populated from day one, no expansion slots needed. Caveat: that 16 MB is the *entire* address space, not RAM's exclusive share — display buffer, ROM, and every memory-mapped I/O register (FDC, NCR 5380, ACIA/OPL2, CRTC) also live somewhere in that same 24-bit range. Actual usable contiguous RAM is 16 MB minus whatever you carve out for those. Worth deciding your address map before treating 16 MB as the literal RAM figure — one plausible period-correct approach: map RAM low, I/O registers high (top of the space), so software sees one big contiguous block up front.
- **Bus:** Novel. Simplest path: treat everything as memory-mapped I/O on the 68000's own address/data bus rather than modeling a real backplane (VMEbus-style) — you don't need card-slot mechanics for an emulator, just address decode ranges. Saves you from emulating bus arbitration you don't need.

## 4. Display

- 80×25 monochrome text mode (hard constraint).
- **Controller:** Novel, but the easiest-to-emulate period-correct choice is an MC6845-style CRTC + character generator ROM (same family used in MDA/Hercules cards) — a handful of registers (cursor position, start address, sync timing) rather than a full bitmap framebuffer state machine.

## 5. Storage — 10-Bay Floppy Array

- 10 physical 5.25" bays, one drive addressable/active at a time (matches the "old-school 2-drive Apple array" behavior you described — convenience slots, not simultaneous R/W).
- **Disk format:** Standard, not HP's proprietary format — recommend plain IBM-compatible MFM 5.25" (360 KB or 1.2 MB), since that gives you off-the-shelf tooling for building disk images and no need to reverse-engineer HP's LIF filesystem.
- **Controller:** NEC µPD765/8272A-style FDC (industry-standard, well-documented, widely emulated already — good reference implementations exist). Native drive-select is only 2 bits (4 drives); to hit 10 you add a **novel drive-select expander** — a small multiplexer sitting between the FDC's drive-select lines and the bay connectors, fed by a memory-mapped "active bay" register. Software picks the bay, expander routes select/motor lines to it. This is the proprietary-bus part — doesn't need to touch SCSI at all, as you said.

## 6. SCSI — "T-38" Port

I couldn't find "T-38" as a standard historical SCSI designation — treating it as your project's internal name for this port unless you meant something specific (a connector type, a mil-spec designator?). Flagging that as an assumption below.

- **Controller:** NCR 5380 — the most period-correct, well-documented, and already-emulated SCSI controller chip of the era (Mac Plus, Amiga 2000, early Sun gear all used it or a close variant). Good existing reference implementations to build your Rust core against.
- Physical connector doesn't matter for emulation purposes — pick whatever's convenient for your mental model (DB-25, Centronics-50, whatever "T-38" is naming).

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

## 10. Emulation Notes (suggestions only, not implementation)

- 68000 core, FDC, CRTC, NCR 5380, and PC/XT-style keyboard protocol all have existing open reference implementations/docs you can study before writing your own Rust versions — none of this is unexplored territory.
- The novel pieces (drive-select expander, bus glue) are small enough to spec as a handful of memory-mapped registers each — shouldn't balloon your address-decode logic.
- GTK C++ front end just needs to talk to the Rust core for: framebuffer/text buffer read, keyboard/wheel packet injection, floppy image mount per bay, SCSI image mount. Clean boundary.

## Open Questions / Unverified Assumptions

- **"T-38"** — assumed to be your own internal label for the SCSI port, not a historical standard I should recognize. If you meant a specific connector or spec, that changes the physical-layer section (not the controller-chip choice).
- **68000 clock speed** — assumed 8 MHz to match the HP 9836 reference exactly, since you listed CPU as a hard constraint. If clock speed itself is flexible and only the CPU family is hard, that opens up performance headroom.
- **Wheel packet routing** — assumed piggybacking on the keyboard link is fine per your "keep it simple" ask. If the wheel needs to be independently hot-pluggable or separately addressable in software, it should get its own UART channel instead.
