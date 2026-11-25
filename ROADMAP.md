# Roadmap: Cornix firmware on RMK ≥ 0.9 (to get working `PDF` keys)

## Why

- The Cornix (PandaKB) runs **RMK** firmware (nRF52840 halves, BLE split, native Vial support).
- `PDF(n)` (persistent default layer) was only added in **RMK 0.9.0 (2026-08-27)**.
- The newest official firmware, **V1.12 (built 2026-03-02)**, is RMK 0.8-era → `PDF` keys are
  silently dropped on write (they show in Vial until the keyboard is re-read, then vanish).
- Goal: build a RMK ≥ 0.9 firmware for the Cornix so `PDF(0)`/`PDF(1)` on layer 3 work and
  persist across reboots. `main-cornix.vil` then needs **no changes**.

## Starting point

- **Base project: [`adong660/rmk-cornix`](https://github.com/adong660/rmk-cornix)** — unofficial
  community RMK config for the Cornix. Verified contents:
  - Correct hardware: nRF52840, BLE split (central = left, peripheral = right), row/col pin maps,
    1 rotary encoder per half, battery ADC (`P0_05`), DCDC config.
  - `[layout] rows = 8, cols = 7, layers = 10` → matches the 10 layers Vial shows today.
  - `[host] vial_enabled = true`, `[storage] enabled = true` (PDF needs storage ✓).
  - GitHub Actions workflow that builds both uf2s (no local toolchain required).
  - **Pinned to `rmk = "0.8"`** → this is what we bump.
- Rollback safety net: official V1.12 uf2s saved in `firmware-official-v1.12/` (this repo).
- Keymap safety net: `main-cornix.vil` (current layout) + `main-cornix.vil.bak` (pre-rearrange).

## Phase 0 — Prep (≈ 30 min)

- [ ] Export the live layout from Vial one more time (File → Save) as a fresh backup.
- [ ] Note current quirks to re-test later: BT profile keys (USER00–02), switch-output, encoders,
      battery readout, sleep behavior.
- [ ] Decide build path:
  - **A (recommended): GitHub Actions** — fork `adong660/rmk-cornix`, push changes, download
    uf2 artifacts. No local Rust toolchain needed.
  - **B: local build** — `rustup target add thumbv7em-none-eabi`, `cargo install cargo-make`,
    `cargo build --release` (or `cargo make`).

## Phase 1 — Scaffold (≈ 15 min)

- [ ] Fork `adong660/rmk-cornix`, clone locally.
- [ ] Sanity build at RMK 0.8 first (confirms toolchain/Actions works before any migration).

## Phase 2 — Bump to RMK 0.9 + migrate (the real work, ≈ 1–2 h)

> **Status: DONE (2026-11-23), with corrections discovered during the work:**
>
> 1. **The `[rmk]` key renames (`num_ble_profiles`, `max_combos`, …) are NOT in 0.9.0** —
>    they're in RMK's *Unreleased* changelog section. RMK 0.9.0 still uses the 0.8-era
>    names (`ble_profiles_num`, `combo_max_num`, `fork_max_num`, `morse_max_num`,
>    `split_peripherals_num`). Only these changes actually belong to 0.9.0:
>    `[layout].matrix_map` → `map`, `[[layer]]` → `[keymap]` + `[[keymap.layer]]`,
>    and strict rejection of unknown keys.
> 2. **`rmkit` v0.1.1 is buggy** — it validates `keyboard.toml` with its bundled
>    rmk-config 0.8, so it rejects valid 0.9 configs. Workaround: use the template repo
>    directly: `rmk-rs/rmk-template`, tag `1b2d061` ("pin templates to the published rmk 0.9"),
>    `nrf52840_split/` directory — it's structurally identical to the adong660 project.
> 3. **Vial UID mystery solved**: the UID is hardcoded in the template's `build.rs`
>    (`0xB9,0xBC,0x09,0xB2,0x9D,0x37,0x4C,0xEA`) — identical to the official Cornix
>    firmware and to `main-cornix.vil` (UID 16882930253541522617 little-endian).
>    **Our builds will load the .vil with no UID warning.**
> 4. `[host] vial_enabled` (default true) and `unlock_keys` are valid in 0.9.0.
>
> Local build path used: template `nrf52840_split` + migrated `keyboard.toml` +
> adong660's `vial.json` (custom USER keycodes) → `cargo build --release`.
> Toolchain installed: `thumbv7em-none-eabihf`, `flip-link`, `arm-none-eabi-gcc` (brew), `rmkit`.
>
> **Toolchain gotcha**: Homebrew's `arm-none-eabi-gcc` is compiler-only (no newlib C
> headers → `stdint.h: No such file` when building trouble's P-256 assembly). Fixed by
> using ARM's official toolchain (includes newlib), extracted at
> `/private/var/folders/8n/jnkgnnvs5yd3v63_zhmvj1r80000gn/T/opencode/arm-gnu-toolchain-14.2.rel1-darwin-arm64-arm-none-eabi`.
> **Prepend its `bin/` to PATH before building:**
> `export PATH="/private/var/folders/8n/jnkgnnvs5yd3v63_zhmvj1r80000gn/T/opencode/arm-gnu-toolchain-14.2.rel1-darwin-arm64-arm-none-eabi/bin:$PATH"`

Follow the official [v0.8 → v0.9 migration guide](https://rmk.rs/docs/migration/v08_v09).
Concrete changes for this repo:

### `Cargo.toml`
- [ ] `rmk = "0.9"` (keep features: `nrf52840_ble`, `split`, `async_matrix`, `adafruit_bl`).
- [ ] Dependency bumps required by 0.9: `embassy-executor` 0.10 (`platform-cortex-m`),
      `embassy-nrf` 0.11, `bt-hci` 0.10, `nrf-sdc`/`nrf-mpsl` 0.4 from crates.io
      (drop the git pin at `rev = "11d5c3c"`, which is pinned to the old bt-hci 0.6).
- [ ] Easiest: copy the dependency block from the official
      [rmk-project-template](https://github.com/rmk-rs/rmk-project-template) /
      RMK's `examples/use_rust/nrf52840_ble_split`, then re-add Cornix-specific bits.

### `keyboard.toml` (0.9 renamed keys and rejects unknown ones)
- [ ] `[rmk]` renames:
      `split_peripherals_num` → `num_split_peripherals`,
      `ble_profiles_num` → `num_ble_profiles`,
      `morse_max_num` → `max_morses`,
      `combo_max_num` → `max_combos`,
      `fork_max_num` → `max_forks`.
- [ ] `[layout].matrix_map` → `[layout].map` (contents unchanged).
- [ ] Move `[[layer]]` tables into the new `[keymap]` structure:
      `[[keymap.layer]]` with `name`, `keys`, `encoders`.
- [ ] `[host]`: keep `vial_enabled = true` and `unlock_keys`; verify names against the 0.9 docs
      (`vial_insecure` was renamed to `insecure` — this repo doesn't use it).
- [ ] Keep `[storage] enabled = true` — PDF persistence depends on it.
- [ ] Check the Vial **UID**: target is `16882930253541522617` (the UID in `main-cornix.vil`).
      If RMK 0.9 doesn't let TOML projects set it, Vial will warn "UID mismatch" when loading
      the .vil — still loadable, just a warning. Verify during bring-up.

### `src/central.rs`, `src/peripheral.rs`
- [ ] 0.9 replaced `run_rmk` with explicit transports + `run_all!` (`HostService`,
      `UsbTransport`, `BleTransport`). Copy the 0.9 BLE-split example structure and keep the
      TOML-config entrypoint pattern — minimal custom code.
- [ ] `memory.x`, `build.rs`, `rust-toolchain.toml`: take the template's versions unless
      `memory.x` differs from the bootloader layout (official app starts at `0x1000`).

### Build
- [ ] `cargo build --release` (or push to the fork and let Actions build).
- [ ] Expect two uf2s: left (central) and right (peripheral).

## Phase 3 — Flash & bring-up (≈ 30 min)

> **Status: FIRMWARE BUILT (2026-11-23).** UF2s ready in `firmware-custom-rmk09/`:
> - `rmk-cornix-central.uf2` → LEFT half (904 KB, official was 877 KB)
> - `rmk-cornix-peripheral.uf2` → RIGHT half (574 KB, official was 569 KB)
>
> Rebuild recipe (from `rmk-cornix/rmk/nrf52840_split/`):
> ```sh
> export PATH="/private/var/folders/8n/jnkgnnvs5yd3v63_zhmvj1r80000gn/T/opencode/arm-gnu-toolchain-14.2.rel1-darwin-arm64-arm-none-eabi/bin:$PATH"
> cargo build --release
> OBJCOPY=~/.rustup/toolchains/stable-aarch64-apple-darwin/lib/rustlib/aarch64-apple-darwin/bin/llvm-objcopy
> T=target/thumbv7em-none-eabihf/release
> $OBJCOPY -O ihex $T/central central.hex && $OBJCOPY -O ihex $T/peripheral peripheral.hex
> cargo hex-to-uf2 --input-path central.hex --output-path rmk-cornix-central.uf2 --family nrf52840
> cargo hex-to-uf2 --input-path peripheral.hex --output-path rmk-cornix-peripheral.uf2 --family nrf52840
> ```

- [ ] In the OS Bluetooth settings, **forget the keyboard** (flashing wipes bonds).
- [ ] Per PandaKB's manual: double-tap each half's reset button → `CORNIX` USB drive appears →
      drag the matching uf2 (left/right). Repeat for the other half.
- [ ] Power-cycle; the halves auto-pair to each other; re-pair your host (or re-pair via BLE profile).
- [ ] Open [vial.rocks](https://vial.rocks) (wired = most reliable), connect, load
      `main-cornix.vil` (accept the UID warning if present).
- [ ] Macros/combos/tap-dances in the current file are all empty → nothing to rebuild.

## Phase 4 — Verification checklist

- [ ] Vial shows layers 0–9; layout matches `main-cornix.vil`.
- [x] **PDF test**: ✅ PASSED (2026-11-24) — PDF(1) survives power-cycle on custom RMK 0.9.0
      firmware. Core objective achieved.
- [ ] Encoders on every layer (moved with layers in the .vil).
- [ ] USER keys: BT0/BT1/BT2 switching, switch-output, clear-peer still behave as before.
- [ ] Split link stable; battery % reported; sleep/wake OK.
- [ ] Layer 3's other keys (USER05, media, numpad-ish cluster) still work.

## Phase 5 — Fallbacks & risks

### LED indicators — PINS RECOVERED from official V1.12 firmware (REA + manual disassembly)
Recovered by extracting the official UF2s (`cornix-left.bin`/`cornix-right.bin`, base 0x1000) and
tracing every GPIO PIN_CNF/OUTSET/OUTCLR/PWM write in the init code. Cross-checked against the
official manual ("2 indicator lights per hand") and the BOM (SK6812MINI-E).

**Each half has 2 indicator LEDs, on the same two pins on BOTH halves: `P0_13` and `P0_24`.**
Per half, one is a plain GPIO LED and the other is an **SK6812/WS2812-style addressable RGB**
driven by PWM0 (COUNTERTOP=20 @16 MHz = 800 kHz bit rate):

| Half | Plain GPIO LED | SK6812 data line (PWM0) |
|------|----------------|-------------------------|
| Left (central)     | **P0_13** (init LOW)  | **P0_24** (PIN_CNF=0x303, PWM0.PSEL=24) |
| Right (peripheral) | **P0_24** (init HIGH) | **P0_13** (PIN_CNF=0x303, PWM0.PSEL=13) |

- Both pins configured `PIN_CNF = 0x303` = output, high-drive, **pull-up** → LEDs are almost
  certainly **active-low** (pull-up keeps them off while floating).
- Evidence sites: left init 0x29d04–0x29d3a (OUTCLR 1<<13, output cfg pin 13, OUTCLR 1<<24,
  PIN_CNF[24]=0x303, PWM0.PSEL=24); right init 0x301e–0x305e (OUTSET 1<<24, output cfg pin 24,
  OUTCLR 1<<13, PIN_CNF[13]=0x303, PWM0.PSEL=13).
- Left also configures **P1_9** in the same mode as the encoder pins (input) — NOT an LED
  (possibly charge/status sense). No other unknown output pins exist in either half.
- Official LED blink patterns live in PandaKB's private `src/rgb/mod.rs` (string at 0x6074b);
  stock RMK 0.9 has no ws2812/rgb support at all.

**Restoring LEDs in the custom firmware — ✅ IMPLEMENTED to match the official manual (2026-11-24):**
`src/led.rs` adds a `CornixLed` RMK processor (registered via `#[register_processor(poll)]` in
both `central.rs` and `peripheral.rs`). It drives a 2-LED SK6812 chain (custom PWM0 DMA driver
replicating the official register setup); the extra GPIO pin mirrors the official init
(low on left, high on right — likely the LED power rail) and is toggled off during sleep.
Official patterns from the manual (images `image-1-1024x618.png` / `image-2-1024x499.png`):
- Left half, left light (Bluetooth): green/red/blue = channels 1/2/3 · flashing = searching ·
  solid 2 s then off = connection successful · off while USB configured.
- Left half, right light (split + battery): blue on-then-off = right half connected ·
  blue breathing = right half disconnected · green breathing = charging · red flash = low battery.
- Right half, left light (battery): green breathing = charging · red flash = low battery.
- Right half, right light (split): blue on-then-off = left half connected · blue breathing = disconnected.
- All LEDs off + power pin toggled during sleep (SleepStateEvent).
- `keyboard.toml` gained `[event.*] subs` bumps so the processor's subscribers fit.
- Tunables if reality differs: SK6812 LED order (`sk6812_write(frame.0, frame.1)` — swap for
  left/right lights), GPIO power-pin level (`power_on_high`), blink/breathe timing constants.
- **LED byte order is R, B, G** (not GRB!) — determined empirically with fixed red/blue/green
  test patterns. Chain: inner light = position #1, outer = #2. Both power rails active-high.
- **White lights = lamps on the SK6812 power rail** (right P0_24 / left P0_13). They
  light whenever the rail is on; driving the rail low kills all colors (verified). A 3rd
  chain position forced OFF did not affect them → not addressable LEDs. ~~Not
  software-controllable separately~~ **Correction (rail-gating build):** the rail itself
  IS software-switchable, so the lamps go off whenever the firmware idles the rail — see
  "LED white-latch bug" section below.
- **Official firmware colors (from REA)**: no static RGB tables — `src/rgb/mod.rs` (left code
  ~0x3124–0x32be) packs colors as HSV triples and runs a software HSV→RGB converter per frame.
  Only hues used anywhere: 0x00 (red), 0x55 (green), 0xAA (blue), always saturation 255; the
  value byte is modulated for breathing/blinking. I.e. official colors are pure R/G/B primaries —
  identical to ours; the cyan-ish "blue" is the SK6812 blue die, same on official firmware.

### Sleep / power verification (do this!)
- Sleep is **automatic idle sleep** (no user action needed). BLE stays connected; first
  keypress wakes the half(s).
- **Wake test:** idle 15 min → type → first key after a brief wake, rest instant.
- **Battery-drain test (the real one):** record battery %, leave keyboard ON overnight.
  Healthy = a few % over 12 h. Losing 20%+ overnight = BLE/split not sleeping → report and
  we tune (this is the "power tuning" risk).
- **Split wake test:** after long idle, type on the RIGHT half — the pair should wake.
- Note: `split_central_sleep_timeout_seconds` defaults to 0 (disabled) in 0.9.0 TOML config;
  general idle sleep is built-in behavior.

### Other risks
- **Rollback always available**: reflash official V1.12 from `firmware-official-v1.12/`.
- Flashing new firmware **wipes RMK storage** (keymap resets to the keyboard.toml default) →
  just reload the .vil in Vial.
- RMK 0.9.0 is recent — if a bug bites, check RMK's GitHub issues; PDF specifically needs the
  `storage` feature (enabled ✓).
- **Lowest-effort alternative**: ask PandaKB (Discord/GitHub) for a V1.13 built on RMK ≥ 0.9 —
  they have RMK's author building their firmware. Worth asking in parallel with this project.

## LED white-latch bug (flash-then-off build) — driver hardened; outer white is hardware

After BT reconnect, lights sat **solid white** on both halves while typing stayed fine.
Driver bugs fixed anyway (they were real):

1. PWM `LOOP.CNT = 0` = infinite re-clocking of the last frame. If the BLE stack stopped
   HFCLK mid-pass, the data line could freeze high → SK6812 latch white.
   **Fix:** `LOOP.CNT = 1` (single pass) + wait for `LOOPSDONE` (bounded ~90 µs spin) +
   explicit STOP task, so the line can never be left high.
2. `last_frame` was updated even when `sk6812_write` bailed out (HFCLK unavailable), so a
   skipped frame was never retried → firmware state and strip state desynced permanently.
   **Fix:** `sk6812_write` returns success; `last_frame` only updates on a real write.
3. Belt-and-braces: the current frame is now **re-sent every 5 s** even when unchanged, so
   any future latch glitch (power brownout etc.) self-heals within seconds.

**Final verdict (rail-gating build):** the white lamps ARE on the switchable SK6812 power
rail — the earlier "hardwired, not software-controllable" conclusion was wrong. The
firmware now **cuts the rail ~3 s after the lights have nothing to show** (all-dark frame
persists), which kills the white lamps too and saves battery; the rail returns ~0.1 s
before the next event light (rail-up tick clears the SK6812 power-on white latch, frame
goes out the next tick). User confirmed: lights dark when connected+idle, only events
light them — matches official behavior. Right half goes dark a beat before the left
(left's BLE flash starts later, so its rail timer ends later) — accepted.

## Encoder fix — root cause: missing pull-ups

Both halves' encoders (left = volume, right = scroll) did nothing. RMK's encoder config
defaults `internal_pullup = false` ("external pull-up resistor is needed"), and the Cornix
board has none on the encoder lines → pins floated, zero quadrature edges, zero events.
**Fix:** `internal_pullup = true` on both `[[split.*.input_device.encoder]]` blocks.
Verified working on hardware. Pins P1_04/P1_06 (both halves) from the community port are
correct after all; `phase = "resolution", resolution = 4` decodes fine.

## Out of scope / later

- **Dongle mode:** RMK supports a USB dongle host (like the Beebkeeb Prospector for ZMK);
  not used here. Note: proprietary 2.4 GHz receivers (Logitech Bolt/Lightspeed/Unifying)
  cannot serve as dongles — sealed firmware, proprietary protocols; RMK dongles must be
  BLE-capable boards (nRF52840-class) running RMK's dongle firmware.

- Fine-tuning LED colors/patterns to exactly match the official firmware (behavior is
  reconstructed from the manual, not frame-captured from the official `src/rgb/mod.rs`).
- Power-consumption tuning to official levels.
- Setting a custom Vial UID if the warning matters.
