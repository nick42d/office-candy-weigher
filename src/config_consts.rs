use embassy_time::Duration;
use embedded_graphics::{pixelcolor::Rgb565, prelude::RgbColor};

use crate::pimoroni_display_leds::Percentage;

pub const DEFAULT_LOLLY_WEIGHT: f32 = 25.0;
pub const TOTAL_LED_FADEOUT_STEPS: u16 = 8;
pub const MAX_MOMENTARY_BUTTON_ON_TIME: Duration = Duration::from_millis(100);
pub const MAX_LED_ON_TIME: Duration = Duration::from_millis(500);
pub const BUTTON_TOOLTIP_COLOUR: Rgb565 = Rgb565::GREEN;
pub const BUTTON_SEMICIRCLE_COLOUR: Rgb565 = Rgb565::WHITE;
pub const SEMICIRCLE_DIAMETER: u32 = 44;
pub const LOW_BACKLIGHT_PERCENTAGE: Percentage = Percentage(20);
pub const TIME_TO_BACKLIGHT_LOW: Duration = Duration::from_secs(10);
// Disable backlight off mode
pub const TIME_FROM_BACKLIGHT_LOW_TO_OFF: Option<Duration> = None;
// Raw tare value for scale - obtained via averaging raw reading with no weight
// on scale.
pub const SCALE_RAW_TARE: f32 = 4190.0;
// Raw tare value for scale - obtained via averaging raw reading with 50g
// calibration weight, subtracting `SCALE_RAW_TARE` and dividing by 50.
pub const SCALE_RAW_1G_STEP: f32 = (39807.0 - SCALE_RAW_TARE) / 50.0;
