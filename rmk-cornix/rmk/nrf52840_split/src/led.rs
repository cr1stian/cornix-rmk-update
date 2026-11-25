//! Cornix indicator LED restoration — follows the official PandaKB manual patterns.
//!
//! Pin assignments recovered by reverse-engineering the official PandaKB V1.12 firmware
//! (see ROADMAP.md). Each half has two indicator lights; both show multiple colors, so
//! both are SK6812 addressable LEDs on a chain driven by PWM0. The extra GPIO pin
//! (P0_13 left / P0_24 right) is the SK6812 power rail (empirically: driving it low
//! kills the color lights on that half). The steady white seen on the right half is
//! suspected to be an extra SK6812 on the chain that never receives data and latches
//! white at power-up — we send an explicit "off" frame to a third chain position to
//! test/kill that.
//!
//! Official behavior from the manual (pandakb.com Cornix wireless split user manual):
//!
//! Left half (central):
//! - Left light  = Bluetooth status: green/red/blue = channel 1/2/3,
//!   flashing = searching (advertising), solid-then-off = connection successful.
//! - Right light = split link + battery: red flash = low battery, green breathing =
//!   charging, blue breathing = right half disconnected, blue on-then-off = right half
//!   connected.
//!
//! Right half (peripheral):
//! - Left light  = battery: red flash = low battery, green breathing = charging.
//! - Right light = split link: blue breathing = left half disconnected,
//!   blue on-then-off = left half connected.

use core::ptr::addr_of_mut;

use embassy_nrf::pac;
use rmk::event::{
    BatteryStatusEvent, CentralConnectedEvent, ChargingStateEvent, ConnectionStatusChangeEvent,
    PeripheralConnectedEvent, SleepStateEvent,
};
use rmk::macros::processor;
use rmk::types::battery::{BatteryStatus, ChargeState};
use rmk::types::ble::BleState;
use rmk::types::connection::{ConnectionStatus, UsbState};

/// RGB color (R, G, B)
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Color(pub u8, pub u8, pub u8);

const OFF: Color = Color(0, 0, 0);
/// Deep blue: the SK6812 blue die reads cyan at full tilt, so mix in red and drop
/// green; kept dim per user preference. (Official firmware uses pure 0,0,255.)
const BLUE: Color = Color(25, 0, 140);
/// Green bright enough to out-shine the always-on white power lamp in the inner
/// light, with a slight forest tint from the blue component.
const GREEN: Color = Color(0, 220, 70);
const RED: Color = Color(255, 0, 0);

/// Bluetooth channel colors: channels 1/2/3 = green/red/blue (RMK profiles 0/1/2).
const fn channel_color(profile: u8) -> Color {
    match profile {
        0 => GREEN,
        1 => RED,
        2 => BLUE,
        _ => BLUE,
    }
}

/// Scale a color to `k`/1000 of its brightness (used for breathing effects).
const fn scale(c: Color, k: u16) -> Color {
    Color(
        (c.0 as u16 * k / 1000) as u8,
        (c.1 as u16 * k / 1000) as u8,
        (c.2 as u16 * k / 1000) as u8,
    )
}

/// Triangle-wave breathing envelope, 2 s period, 10 %..100 % brightness.
const fn breathe(tick: u32) -> u16 {
    let phase = tick % 20; // 20 * 100 ms = 2 s
    let x = if phase < 10 { phase } else { 19 - phase }; // 0..9
    100 + x as u16 * 100
}

// ---------------------------------------------------------------------------
// SK6812 (WS2812-compatible) driver on PWM0 — mirrors the official firmware's
// setup: MODE=up, COUNTERTOP=20 @ 16 MHz (800 kHz), simple decoder, DMA seq 0.
// ---------------------------------------------------------------------------

const COUNTERTOP: u16 = 20;
const T1_HIGH: u16 = 13; // ~812 ns high ("1")
const T0_HIGH: u16 = 6; //  ~375 ns high ("0")
/// 3rd position: suspected extra SK6812 (right half) that never receives data and
/// latches white at power-up; we always send it an explicit OFF to keep it dark.
const NUM_LEDS: usize = 3;
const BUF_BITS: usize = 24 * NUM_LEDS + 8; // trailing zeros act as the reset pulse

/// EasyDMA ignores the low 2 bits of the buffer pointer, so the buffer must be
/// 4-byte aligned or the LEDs latch shifted (garbage) data.
#[repr(align(4))]
struct DmaBuf([u16; BUF_BITS]);
static mut DMA_BUF: DmaBuf = DmaBuf([0; BUF_BITS]);

fn port_of(pin: u8) -> pac::gpio::Gpio {
    if pin < 32 {
        pac::P0
    } else {
        pac::P1
    }
}

fn pin_of(pin: u8) -> u8 {
    pin % 32
}

fn set_pin_output(pin: u8, high_drive: bool) {
    port_of(pin).pin_cnf(pin_of(pin) as usize).write(|w| {
        w.set_dir(pac::gpio::vals::Dir::Output);
        w.set_input(pac::gpio::vals::Input::Disconnect);
        w.set_pull(pac::gpio::vals::Pull::Disabled);
        w.set_drive(if high_drive {
            pac::gpio::vals::Drive::H0h1
        } else {
            pac::gpio::vals::Drive::S0s1
        });
    });
}

fn write_pin(pin: u8, high: bool) {
    let p = port_of(pin);
    if high {
        p.outset().write(|w| w.0 = 1u32 << pin_of(pin));
    } else {
        p.outclr().write(|w| w.0 = 1u32 << pin_of(pin));
    }
}

fn sk6812_init(pin: u8) {
    set_pin_output(pin, true);
    write_pin(pin, false); // data line idles low
    // PWM runs off HFCLK; make sure it's running (BLE may have left it off).
    pac::CLOCK.tasks_hfclkstart().write_value(1);
    let pwm = pac::PWM0;
    pwm.enable().write(|w| w.set_enable(false));
    pwm.mode().write(|w| w.set_updown(pac::pwm::vals::Updown::Up));
    pwm.countertop().write(|w| w.set_countertop(COUNTERTOP));
    pwm.prescaler()
        .write(|w| w.set_prescaler(pac::pwm::vals::Prescaler::Div1));
    pwm.decoder().write(|w| {
        w.set_load(pac::pwm::vals::Load::Common);
        w.set_mode(pac::pwm::vals::Mode::RefreshCount);
    });
    // One sequence pass per start task (0 would loop forever: if the BLE stack
    // stopped HFCLK mid-loop the data line could freeze high and latch the
    // SK6812s white). sk6812_write waits for the pass to finish, then stops.
    pwm.loop_().write(|w| w.set_cnt(1.into()));
    pwm.psel().out(0).write(|w| {
        w.set_port(pin >= 32);
        w.set_pin(pin_of(pin));
    });
    pwm.intenclr().write(|w| w.0 = 0xFFFF_FFFF);
    pwm.shorts().write(|w| w.0 = 0);
    pwm.events_stopped().write_value(0);
    pwm.events_seqend(0).write_value(0);
    pwm.events_loopsdone().write_value(0);
    pwm.enable().write(|w| w.set_enable(true));
}

/// Latch `led0` (first on the chain), then `led1`, then `led2`. ~120 us frame time.
/// Returns `false` if the frame was *not* clocked out (HFCLK unavailable), so the
/// caller can retry instead of believing the LEDs match our state.
fn sk6812_write(led0: Color, led1: Color, led2: Color) -> bool {
    unsafe {
        // PWM runs off HFCLK, which the BLE stack switches off whenever the
        // radio is idle. Start it if it's off; bail out (no spin-hogging!) if
        // it doesn't come up quickly so the keyboard is never starved.
        let clock = pac::CLOCK;
        if !clock.hfclkstat().read().state() {
            clock.events_hfclkstarted().write_value(0);
            clock.tasks_hfclkstart().write_value(1);
            let mut hf = 0u32;
            while !clock.hfclkstat().read().state() {
                hf += 1;
                if hf > 100_000 {
                    return false; // ~15 ms max; skip this frame instead of hogging the CPU
                }
            }
        }
        let buf = &mut (*addr_of_mut!(DMA_BUF)).0;
        let mut i = 0;
        for c in [led0, led1, led2] {
            // These SK6812s latch channels in R, B, G order (empirically verified
            // with fixed red/blue/green test patterns).
            for byte in [c.0, c.2, c.1] {
                for bit in (0..8).rev() {
                    buf[i] = if (byte >> bit) & 1 == 1 { T1_HIGH } else { T0_HIGH };
                    i += 1;
                }
            }
        }
        while i < BUF_BITS {
            buf[i] = 0;
            i += 1;
        }
        let pwm = pac::PWM0;
        pwm.events_loopsdone().write_value(0);
        let seq = pwm.dma().seq(0);
        seq.ptr().write_value(buf.as_ptr() as u32);
        seq.maxcnt().write(|w| w.set_cnt((BUF_BITS as u16).into()));
        seq.refresh().write(|w| w.set_cnt(0.into()));
        pwm.tasks_dma().seq(0).start().write_value(1);
        // Wait for the single pass (~90 us) to finish before returning, so the
        // BLE stack can't stop HFCLK mid-frame and freeze the data line high
        // (which latches the SK6812s white). Bounded so we never hog the CPU.
        let mut done = 0u32;
        while pwm.events_loopsdone().read() == 0 {
            done += 1;
            if done > 100_000 {
                break;
            }
        }
        pwm.tasks_stop().write_value(1);
        pwm.events_loopsdone().write_value(0);
        true
    }
}

// ---------------------------------------------------------------------------
// LED state processor
// ---------------------------------------------------------------------------

/// Ticks (100 ms each) a "solid then off" success indication stays lit: 2 s.
const SUCCESS_FLASH_TICKS: u32 = 20;
/// Battery percentage at or below which the low-battery flash starts.
const LOW_BATTERY_PCT: u8 = 20;

/// Indicator LED processor. `power_pin` is the SK6812 power rail (must stay high or
/// the color lights die), `rgb_pin` is the SK6812 data line. `is_central` selects the
/// left-half (Bluetooth) light layout.
#[processor(
    subscribe = [
        ConnectionStatusChangeEvent,
        BatteryStatusEvent,
        ChargingStateEvent,
        CentralConnectedEvent,
        PeripheralConnectedEvent,
        SleepStateEvent,
    ],
    poll_interval = 100
)]
pub struct CornixLed {
    /// SK6812 power rail (left P0_13 / right P0_24), active-high.
    power_pin: u8,
    is_central: bool,
    conn: ConnectionStatus,
    battery: BatteryStatus,
    charging: bool,
    split_up: bool,
    sleeping: bool,
    tick: u32,
    /// Consecutive ticks with nothing to display (used to power down the rail).
    dark_ticks: u32,
    /// Tick deadline for the "split connected" blue solid-then-off flash.
    split_flash_until: Option<u32>,
    /// Tick deadline for the "BLE connected" channel-color solid-then-off flash.
    ble_flash_until: Option<u32>,
    last_frame: (Color, Color),
    last_power: bool,
}

impl CornixLed {
    pub fn new(power_pin: u8, rgb_pin: u8, is_central: bool) -> Self {
        // The power rail must be high for the SK6812s to light at all (empirically
        // verified: driving it low kills all colors on that half).
        set_pin_output(power_pin, false);
        write_pin(power_pin, true);
        sk6812_init(rgb_pin);
        sk6812_write(OFF, OFF, OFF); // clear the SK6812 power-on glitch
        Self {
            power_pin,
            is_central,
            conn: ConnectionStatus::new(),
            battery: BatteryStatus::Unavailable,
            charging: false,
            split_up: true,
            sleeping: false,
            tick: 0,
            dark_ticks: 0,
            split_flash_until: None,
            ble_flash_until: None,
            last_frame: (OFF, OFF),
            last_power: true,
        }
    }

    async fn on_connection_status_change_event(&mut self, event: ConnectionStatusChangeEvent) {
        let was_connected = matches!(self.conn.ble.state, BleState::Connected);
        let is_connected = matches!(event.0.ble.state, BleState::Connected);
        if is_connected && !was_connected {
            // "Solid light that then turns off indicates successful Bluetooth connection"
            self.ble_flash_until = Some(self.tick + SUCCESS_FLASH_TICKS);
        }
        self.conn = event.0;
    }

    async fn on_battery_status_event(&mut self, event: BatteryStatusEvent) {
        if let BatteryStatus::Available { charge_state, .. } = &event.0 {
            self.charging = *charge_state == ChargeState::Charging;
        }
        self.battery = event.0;
    }

    async fn on_charging_state_event(&mut self, event: ChargingStateEvent) {
        self.charging = event.charging;
    }

    async fn on_central_connected_event(&mut self, event: CentralConnectedEvent) {
        if !self.is_central {
            if event.connected && !self.split_up {
                self.split_flash_until = Some(self.tick + SUCCESS_FLASH_TICKS);
            }
            self.split_up = event.connected;
        }
    }

    async fn on_peripheral_connected_event(&mut self, event: PeripheralConnectedEvent) {
        if self.is_central {
            if event.connected && !self.split_up {
                self.split_flash_until = Some(self.tick + SUCCESS_FLASH_TICKS);
            }
            self.split_up = event.connected;
        }
    }

    async fn on_sleep_state_event(&mut self, event: SleepStateEvent) {
        self.sleeping = event.0;
    }

    fn flash_active(deadline: &Option<u32>, tick: u32) -> bool {
        match deadline {
            Some(d) => tick < *d,
            None => false,
        }
    }

    /// Left-half light layout: LED0 = Bluetooth status, LED1 = split link + battery.
    fn render_central(&mut self) -> (Color, Color) {
        let blink_1hz = (self.tick / 5) % 2 == 0;
        let ch = channel_color(self.conn.ble.profile);

        // --- LED0: Bluetooth channel status ---
        let led0 = if matches!(self.conn.usb, UsbState::Configured) {
            OFF // wired mode: no Bluetooth to indicate
        } else if Self::flash_active(&self.ble_flash_until, self.tick) {
            ch // solid channel color for 2 s, then off = connection successful
        } else {
            match self.conn.ble.state {
                BleState::Advertising => {
                    if blink_1hz {
                        ch // flashing = searching
                    } else {
                        OFF
                    }
                }
                // Connected: stays dark after the success flash (official style).
                BleState::Connected | BleState::Inactive => OFF,
            }
        };

        // --- LED1: split link + battery ---
        let low_battery = matches!(self.battery, BatteryStatus::Available { level: Some(l), .. } if l <= LOW_BATTERY_PCT);
        let led1 = if self.charging {
            scale(GREEN, breathe(self.tick)) // green breathing = charging
        } else if low_battery && blink_1hz {
            RED // flashing red = low battery
        } else if !self.split_up {
            scale(BLUE, breathe(self.tick)) // blue breathing = right half disconnected
        } else if Self::flash_active(&self.split_flash_until, self.tick) {
            BLUE // solid blue for 2 s, then off = right half connected
        } else {
            OFF
        };

        (led0, led1)
    }

    /// Right-half light layout: LED0 = battery, LED1 = split link.
    fn render_peripheral(&mut self) -> (Color, Color) {
        let blink_1hz = (self.tick / 5) % 2 == 0;

        // --- LED0: battery level ---
        let low_battery = matches!(self.battery, BatteryStatus::Available { level: Some(l), .. } if l <= LOW_BATTERY_PCT);
        let led0 = if self.charging {
            scale(GREEN, breathe(self.tick)) // green breathing = charging
        } else if low_battery && blink_1hz {
            RED // flashing red = low battery
        } else {
            OFF
        };

        // --- LED1: split link ---
        let led1 = if !self.split_up {
            scale(BLUE, breathe(self.tick)) // blue breathing = left half disconnected
        } else if Self::flash_active(&self.split_flash_until, self.tick) {
            BLUE // solid blue for 2 s, then off = left half connected
        } else {
            OFF
        };

        (led0, led1)
    }

    async fn poll(&mut self) {
        self.tick = self.tick.wrapping_add(1);

        if self.sleeping {
            if self.last_frame != (OFF, OFF) && sk6812_write(OFF, OFF, OFF) {
                self.last_frame = (OFF, OFF);
            }
            // Cut LED power while asleep.
            if self.last_power {
                write_pin(self.power_pin, false);
                self.last_power = false;
            }
            return;
        }

        let frame = if self.is_central {
            self.render_central()
        } else {
            self.render_peripheral()
        };

        if frame == (OFF, OFF) && self.last_frame == (OFF, OFF) {
            self.dark_ticks = self.dark_ticks.saturating_add(1);
            // Nothing to show for 3 s: cut the LED power rail. The white lamps
            // wired to it go dark too (matching the official firmware's idle
            // behavior) and it saves battery.
            if self.dark_ticks >= 30 && self.last_power {
                write_pin(self.power_pin, false);
                self.last_power = false;
            }
            return;
        }
        self.dark_ticks = 0;

        if !self.last_power {
            // Rail was off: bring it up and clear the SK6812 power-on white
            // latch now; the actual frame goes out on the next tick once the
            // chips are fully powered.
            write_pin(self.power_pin, true);
            self.last_power = true;
            sk6812_write(OFF, OFF, OFF);
            self.last_frame = (OFF, OFF);
            return;
        }

        // Resend periodically (every 5 s) even if unchanged: SK6812s latch white
        // whenever their supply or data line glitches, and a self-heal refresh
        // repairs that within seconds.
        if frame != self.last_frame || self.tick % 50 == 0 {
            // Third chain position forced OFF: kills the white power-on glow if it
            // comes from an extra SK6812 on the chain.
            if sk6812_write(frame.0, frame.1, OFF) {
                self.last_frame = frame;
            }
        }
    }
}
