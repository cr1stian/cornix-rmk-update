#![no_main]
#![no_std]

mod led;

use rmk::macros::rmk_peripheral;

#[rmk_peripheral(id = 0)]
mod keyboard_peripheral {
    /// Cornix indicator LEDs (recovered from official firmware):
    /// plain active-low LED on P0_24, SK6812 on P0_13.
    #[register_processor(poll)]
    fn cornix_led() -> crate::led::CornixLed {
        crate::led::CornixLed::new(24, 13, false)
    }
}
