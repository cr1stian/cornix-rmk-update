#![no_main]
#![no_std]

mod led;

use rmk::macros::rmk_central;

#[rmk_central]
mod keyboard_central {
    /// Cornix indicator LEDs (recovered from official firmware):
    /// plain active-low LED on P0_13, SK6812 on P0_24.
    #[register_processor(poll)]
    fn cornix_led() -> crate::led::CornixLed {
        crate::led::CornixLed::new(13, 24, true)
    }
}
