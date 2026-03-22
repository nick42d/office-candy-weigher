use embassy_time::Duration;
use embedded_graphics::{pixelcolor::Rgb565, prelude::RgbColor};

pub const DEFAULT_LOLLY_WEIGHT: f32 = 25.0;
pub const TOTAL_LED_FADEOUT_STEPS: u16 = 8;
pub const MAX_MOMENTARY_BUTTON_ON_TIME: Duration = Duration::from_millis(100);
pub const MAX_LED_ON_TIME: Duration = Duration::from_millis(500);
pub const BUTTON_TOOLTIP_COLOUR: Rgb565 = Rgb565::GREEN;
pub const BUTTON_SEMICIRCLE_COLOUR: Rgb565 = Rgb565::WHITE;
pub const SEMICIRCLE_DIAMETER: u32 = 44;
