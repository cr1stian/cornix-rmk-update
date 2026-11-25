# Cornix RMK Update

Custom RMK firmware for the [PandaKB Cornix](https://www.pandakb.com/) wireless split
keyboard (nRF52840, BLE split), built on **RMK 0.9.0** and configurable with **Vial**.

This project restores the indicator-LED behavior described in the official PandaKB manual
(on a modern RMK that dropped it), reorganizes the layer/keymap, persists layouts across
reboots, and adds battery-friendly power management — while keeping typing, Bluetooth,
and Vial latency untouched.

## Features

- **Official-style indicator LEDs** (see table below): lights flash on connect, then go
  fully dark; they only relight for events (searching, split lost, low battery, charging).
- **LED power-rail gating**: ~3 s after the lights have nothing to show, the firmware
  cuts the SK6812 power rail — this also turns off the white lamps wired to that rail
  and saves battery. The rail returns ~0.1 s before the next event light.
- **Idle sleep** (RMK built-in, enabled here): radio + CPU sleep after 15 min of no
  typing; the first keypress wakes the keyboard. Host-suspend sleep (Mac sleeps →
  keyboard sleeps) works as well.
- **Reduced BLE TX power** (−8 dBm vs the 0 dBm default): ~6× less radio transmit power
  with no practical range loss at desk distance.
- **Hardened SK6812 driver**: single-pass PWM frames (the data line can never freeze
  mid-frame and latch the LEDs white), failed frame writes are retried, and the current
  frame is re-sent every 5 s as a self-heal against latch glitches.
- **Layout persistence**: Vial keymap changes (including the PDF layer) survive reboot,
  via RMK's storage backend.
- **Tuned colors**: deep blue `RGB(25,0,140)` and forest green `RGB(0,220,70)` — the
  SK6812 blue die reads cyan at full tilt, so the blue mixes in red and drops green.

### LED behavior

| State | Left inner | Left outer | Right inner | Right outer |
|---|---|---|---|---|
| Bluetooth searching | blink (channel color) | — | — | — |
| Bluetooth connected | solid 2 s → dark | — | — | — |
| Halves linked | — | solid blue 2 s → dark | — | solid blue 2 s → dark |
| Split link lost | — | blue breathing | — | blue breathing |
| Low battery (≤ 20 %) | — | red flash 1 Hz | red flash 1 Hz | — |
| Charging | — | green breathing | green breathing | — |
| Idle & connected | dark | dark (rail off) | dark (rail off) | dark (rail off) |

Channel colors: profile 1/2/3 = green/red/blue. The white lamps (right inner + both
outers) sit on the switchable LED power rail, so they are dark whenever the rail is
gated off while idle.

## Repository layout

```
firmware-custom-rmk09/        Flashable UF2s (current custom build: central + peripheral)
firmware-official-v1.12/      Stock PandaKB UF2s (rollback)
main-cornix.vil               Vial layout (reload after every flash)
rmk-cornix/                   Firmware source (vendored from adong660/rmk-cornix, RMK 0.9)
  rmk/nrf52840_split/
    keyboard.toml             Hardware + behavior config (matrix, BLE, sleep, event subs)
    src/led.rs                Custom CornixLed processor + raw SK6812 PWM driver
    src/central.rs            Central (left half) entry, registers the LED processor
    src/peripheral.rs         Peripheral (right half) entry
ROADMAP.md                    Full reverse-engineering notes, findings, and decision log
```

## Building

Prereqs: Rust (stable, `thumbv7em-none-eabihf` target), `cargo-hex-to-uf2`
(`cargo install cargo-hex-to-uf2`), and an ARM GNU toolchain providing `llvm-objcopy`
(or any `llvm-objcopy`).

```sh
cd rmk-cornix/rmk/nrf52840_split
export PATH="/path/to/arm-gnu-toolchain/bin:$PATH"   # provides arm-none-eabi-*
cargo build --release
OBJCOPY=llvm-objcopy   # e.g. ~/.rustup/toolchains/stable-*/lib/rustlib/*/bin/llvm-objcopy
T=target/thumbv7em-none-eabihf/release
$OBJCOPY -O ihex $T/central central.hex
$OBJCOPY -O ihex $T/peripheral peripheral.hex
cargo hex-to-uf2 --input-path central.hex    --output-path rmk-cornix-central.uf2    --family nrf52840
cargo hex-to-uf2 --input-path peripheral.hex --output-path rmk-cornix-peripheral.uf2 --family nrf52840
```

## Flashing

1. Power the half on, **double-press its reset button** → a `cornix` USB drive appears.
2. Copy the matching UF2 to it (`central` = left half, `peripheral` = right half). The
   drive disappears when the flash completes and the half reboots.
3. Flash **one half at a time** (only one `cornix` drive should be mounted at once).

```sh
cp firmware-custom-rmk09/rmk-cornix-central.uf2    /Volumes/cornix/   # left
cp firmware-custom-rmk09/rmk-cornix-peripheral.uf2 /Volumes/cornix/   # right
```

**After every flash**: RMK storage and BLE bonds are wiped — re-pair the keyboard with
your host(s) and reload `main-cornix.vil` in Vial.

Rollback: flash the stock UF2s from `firmware-official-v1.12/` the same way.

## Hardware notes (verified empirically)

- Each half: two SK6812 addressable LEDs on a PWM-driven data line, byte order **R,B,G**
  (not GRB), chain order inner → outer, plus a third forced-OFF chain slot.
- A GPIO pin per half (left `P0_13`, right `P0_24`) is the **SK6812 power rail**
  (active-high). Driving it low kills all lights on that half — which is exactly how the
  idle rail-gating turns the white lamps off.
- The white lamps are not addressable LEDs; they light whenever the rail is powered.
- Battery sense: `P0_05` via divider (2000/2866 measured/total).
- Official firmware colors (from RE analysis): pure HSV primaries (hue 0/85/170,
  saturation 255) computed by a software HSV→RGB converter — no hidden palette.

## Known issues / backlog

- **Encoders**: both halves' encoders are declared in `keyboard.toml` and mapped in the
  default keymap, but scroll/volume actions currently do nothing — under investigation
  (see ROADMAP).
- **Dongle mode**: RMK supports a USB dongle host (like the Beebkeeb Prospector for ZMK);
  not used here.
- Full history of findings and decisions: [`ROADMAP.md`](ROADMAP.md).

## Credits

- [RMK](https://github.com/rmk-rs/rmk) by Haobo Gu — firmware framework (0.9.0).
- [adong660/rmk-cornix](https://github.com/adong660/rmk-cornix) — base Cornix port
  (matrix/pin definitions) this source tree is vendored from.
- [PandaKB](https://www.pandakb.com/) — hardware and the official user manual whose LED
  behavior this firmware restores.
