use core::num::NonZeroU32;

use crate::{hardware_controllers::pimoroni_display_leds::Percentage, utils::ScaleRawWeight};
#[cfg(feature = "hardware-sim")]
use embassy_rp::peripherals::{PIN_26, PIN_27, PIO0};
use embassy_rp::{
    Peri,
    peripherals::{
        CORE1, DMA_CH0, DMA_CH1, FLASH, PIN_6, PIN_7, PIN_8, PIN_10, PIN_11, PIN_12, PIN_13,
        PIN_14, PIN_15, PIN_16, PIN_17, PIN_18, PIN_19, PIN_20, PIO1, PWM_SLICE2, PWM_SLICE3,
        PWM_SLICE4, SPI0,
    },
};
use embassy_time::Duration;
use embedded_graphics::{
    pixelcolor::Rgb565,
    prelude::{RgbColor, WebColors},
};

pub const DEFAULT_LOLLY_WEIGHT: f32 = 25.0;
pub const TOTAL_LED_FADEOUT_STEPS: u16 = 8;
pub const MAX_LED_ON_TIME: Duration = Duration::from_millis(500);
pub const BUTTON_TOOLTIP_COLOUR: Rgb565 = Rgb565::GREEN;
pub const BUTTON_SEMICIRCLE_COLOUR: Rgb565 = Rgb565::WHITE;
pub const BUTTON_SEMICIRCLE_HELD_COLOUR: Rgb565 = Rgb565::BLACK;
pub const SEMICIRCLE_DIAMETER: u32 = 44;
pub const LOW_BACKLIGHT_PERCENTAGE: Percentage = Percentage(20);
pub const BUTTON_LONG_PRESS_THRESHOLD: Duration = Duration::from_millis(1000);
pub const BUTTON_LONG_PRESS_PROGRESS_CHUNKS: NonZeroU32 = NonZeroU32::new(10).unwrap();
pub const BUTTON_REPEAT_THRESHOLD: Duration = Duration::from_millis(100);
pub const TIME_TO_BACKLIGHT_LOW: Duration = Duration::from_secs(10);
pub const TIME_FROM_BACKLIGHT_LOW_TO_OFF: Option<Duration> = Some(Duration::from_secs(60 * 5)); // 5 mins
pub const DEFAULT_SCALE_RAW_TARE: ScaleRawWeight = ScaleRawWeight::from_raw(4190.0);
pub const DEFAULT_SCALE_RAW_50G: ScaleRawWeight = ScaleRawWeight::from_raw(39807.0);

// Raw tare value for scale - obtained via averaging raw reading with 50g
// calibration weight, subtracting `SCALE_RAW_TARE` and dividing by 50.
pub const fn scale_raw_1g_step(scale_raw_tare: f32, scale_raw_50g: f32) -> f32 {
    (scale_raw_50g - scale_raw_tare) / 50.0
}

// By constraining all peripherals used in the project to this struct, it gives
// the reader a single source of truth for connecting wiring.
pub struct OfficeCandyWeigherPeripherals {
    pub display_manager_pwm_slice: Peri<'static, PWM_SLICE2>,
    pub display_led_controller_rg_pwm_slice: Peri<'static, PWM_SLICE3>,
    pub display_led_controller_b_pwm_slice: Peri<'static, PWM_SLICE4>,
    pub flash: Peri<'static, FLASH>,
    pub display_manager_dma: Peri<'static, DMA_CH0>,
    pub flash_dma: Peri<'static, DMA_CH1>,
    pub core_1: Peri<'static, CORE1>,
    pub display_manager_spi: Peri<'static, SPI0>,
    pub hx710_pio: Peri<'static, PIO1>,
    #[cfg(feature = "hardware-sim")]
    pub rotary_encoder_pio: Peri<'static, PIO0>,

    pub display_led_controller_r_pin: Peri<'static, PIN_6>,
    pub display_led_controller_g_pin: Peri<'static, PIN_7>,
    pub display_led_controller_b_pin: Peri<'static, PIN_8>,
    pub hx710_sclk_pin: Peri<'static, PIN_10>,
    pub hx710_dout_pin: Peri<'static, PIN_11>,
    pub button_a_pin: Peri<'static, PIN_12>,
    pub button_b_pin: Peri<'static, PIN_13>,
    pub button_x_pin: Peri<'static, PIN_14>,
    pub button_y_pin: Peri<'static, PIN_15>,
    pub display_manager_dcx_pin: Peri<'static, PIN_16>,
    pub display_manager_spi_cs_pin: Peri<'static, PIN_17>,
    pub display_manager_spi_clk_pin: Peri<'static, PIN_18>,
    pub display_manager_spi_mosi_pin: Peri<'static, PIN_19>,
    pub display_manager_backlight_pin: Peri<'static, PIN_20>,
    #[cfg(feature = "hardware-sim")]
    pub rotary_encoder_sclk_pin: Peri<'static, PIN_26>,
    #[cfg(feature = "hardware-sim")]
    pub rotary_encoder_dout_pin: Peri<'static, PIN_27>,
}

pub const fn assign_peripherals(
    peripherals: embassy_rp::Peripherals,
) -> OfficeCandyWeigherPeripherals {
    OfficeCandyWeigherPeripherals {
        display_led_controller_rg_pwm_slice: peripherals.PWM_SLICE3,
        display_led_controller_b_pwm_slice: peripherals.PWM_SLICE4,
        display_led_controller_r_pin: peripherals.PIN_6,
        display_led_controller_g_pin: peripherals.PIN_7,
        display_led_controller_b_pin: peripherals.PIN_8,
        flash: peripherals.FLASH,
        flash_dma: peripherals.DMA_CH1,
        core_1: peripherals.CORE1,
        display_manager_spi: peripherals.SPI0,
        display_manager_spi_clk_pin: peripherals.PIN_18,
        display_manager_spi_mosi_pin: peripherals.PIN_19,
        display_manager_dcx_pin: peripherals.PIN_16,
        display_manager_spi_cs_pin: peripherals.PIN_17,
        display_manager_backlight_pin: peripherals.PIN_20,
        display_manager_pwm_slice: peripherals.PWM_SLICE2,
        display_manager_dma: peripherals.DMA_CH0,
        button_a_pin: peripherals.PIN_12,
        button_b_pin: peripherals.PIN_13,
        button_x_pin: peripherals.PIN_14,
        button_y_pin: peripherals.PIN_15,
        hx710_pio: peripherals.PIO1,
        hx710_sclk_pin: peripherals.PIN_10,
        hx710_dout_pin: peripherals.PIN_11,
        #[cfg(feature = "hardware-sim")]
        rotary_encoder_pio: peripherals.PIO0,
        #[cfg(feature = "hardware-sim")]
        rotary_encoder_sclk_pin: peripherals.PIN_26,
        #[cfg(feature = "hardware-sim")]
        rotary_encoder_dout_pin: peripherals.PIN_27,
    }
}
