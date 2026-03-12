#![no_std]
#![no_main]

use core::cell::RefCell;
use defmt::*;
// use display_interface_spi::SPIInterface;
use crate::display_leds::DisplayRgbLedController;
use embassy_embedded_hal::shared_bus::blocking::spi::SpiDeviceWithConfig;
use embassy_executor::Spawner;
use embassy_futures::select::{select4, Either4};
use embassy_rp::gpio::{Input, Level, Output, Pull};
use embassy_rp::spi::Spi;
use embassy_rp::{config, spi, Peri, Peripherals};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::blocking_mutex::Mutex;
use embassy_time::{Delay, Timer};
use embedded_graphics::image::{Image, ImageRawLE};
use embedded_graphics::mono_font::ascii::FONT_10X20;
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{PrimitiveStyleBuilder, Rectangle};
use embedded_graphics::text::Text;
use mipidsi::models::ST7789;
use mipidsi::options::{Orientation, Rotation};
use mipidsi::Builder;

use {defmt_rtt as _, panic_probe as _};

const DISPLAY_FREQ: u32 = 16_000_000;
const TOUCH_FREQ: u32 = 200_000;

mod display_leds {
    use embassy_rp::peripherals::{PIN_6, PIN_7, PIN_8, PWM_SLICE3, PWM_SLICE4};
    use embassy_rp::pwm::{self, Pwm};
    use embassy_rp::Peri;
    pub struct DisplayRgbLedController<'a> {
        // r-g share slice
        rg_pwm_slice: Pwm<'a>,
        b_pwm_slice: Pwm<'a>,
        rg_conf: pwm::Config,
        b_conf: pwm::Config,
    }
    impl<'a> DisplayRgbLedController<'a> {
        pub fn new(
            slice_3: Peri<'a, PWM_SLICE3>,
            slice_4: Peri<'a, PWM_SLICE4>,
            pin_6: Peri<'a, PIN_6>,
            pin_7: Peri<'a, PIN_7>,
            pin_8: Peri<'a, PIN_8>,
        ) -> DisplayRgbLedController<'a> {
            let mut pwm_config = pwm::Config::default();
            // high is off
            pwm_config.invert_a = true;
            pwm_config.invert_b = true;
            // max period per datasheet
            pwm_config.top = 65535;
            let rg_pwm_slice = Pwm::new_output_ab(slice_3, pin_6, pin_7, pwm_config.clone());
            let b_pwm_slice = Pwm::new_output_a(slice_4, pin_8, pwm_config.clone());
            Self {
                rg_pwm_slice,
                b_pwm_slice,
                rg_conf: pwm_config.clone(),
                b_conf: pwm_config,
            }
        }
        pub fn red_on(&mut self) {
            self.rg_conf.compare_a = 0xffff;
            self.rg_pwm_slice.set_config(&self.rg_conf);
        }
        pub fn red_off(&mut self) {
            self.rg_conf.compare_a = 0x0000;
            self.rg_pwm_slice.set_config(&self.rg_conf);
        }
        pub fn green_on(&mut self) {
            self.rg_conf.compare_b = 0xffff;
            self.rg_pwm_slice.set_config(&self.rg_conf);
        }
        pub fn green_off(&mut self) {
            self.rg_conf.compare_b = 0x0000;
            self.rg_pwm_slice.set_config(&self.rg_conf);
        }
        pub fn blue_on(&mut self) {
            self.b_conf.compare_a = 0xffff;
            self.b_pwm_slice.set_config(&self.b_conf);
        }
        pub fn blue_off(&mut self) {
            self.b_conf.compare_a = 0x0000;
            self.b_pwm_slice.set_config(&self.b_conf);
        }
    }
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    info!("Hello World!");

    let bl = p.PIN_20;
    let display_cs = p.PIN_17;
    let dcx = p.PIN_16;
    let mosi = p.PIN_19;
    let clk = p.PIN_18;

    let mut led =
        DisplayRgbLedController::new(p.PWM_SLICE3, p.PWM_SLICE4, p.PIN_6, p.PIN_7, p.PIN_8);

    // fn set_rgb(colour: Rgb565, brightness_percentage_int: u8, rg_slice: &mut
    // Pwm<'_>, b_slice: Pwm<'_>, config: &mut pwm::Config) { }

    let mut button_a = Input::new(p.PIN_12, Pull::Up);
    let mut button_b = Input::new(p.PIN_13, Pull::Up);
    let mut button_x = Input::new(p.PIN_14, Pull::Up);
    let mut button_y = Input::new(p.PIN_15, Pull::Up);

    // let miso = p.PIN_12;
    // let touch_cs = p.PIN_16;
    // let touch_irq = p.PIN_17;

    // create SPI
    let mut display_config = spi::Config::default();
    display_config.frequency = DISPLAY_FREQ;
    display_config.phase = spi::Phase::CaptureOnSecondTransition;
    display_config.polarity = spi::Polarity::IdleHigh;

    let spi = Spi::new_blocking_txonly(p.SPI0, clk, mosi, display_config.clone());
    let spi_bus: Mutex<NoopRawMutex, _> = Mutex::new(RefCell::new(spi));
    let display_spi = SpiDeviceWithConfig::new(
        &spi_bus,
        Output::new(display_cs, Level::High),
        display_config,
    );

    let dcx = Output::new(dcx, Level::Low);
    // dcx: 0 = command, 1 = data

    // Enable LCD backlight - required for screen to operate
    let _bl = Output::new(bl, Level::High);

    let mut buffer = [0_u8; 512];

    // display interface abstraction from SPI and DC
    let di = mipidsi::interface::SpiInterface::new(display_spi, dcx, &mut buffer);

    // Define the display from the display interface and initialize it
    let mut display = Builder::new(ST7789, di)
        // Magic numbers for pico display offset.
        .display_offset(52, 40)
        // Actual w/h for pico display.
        .display_size(135, 240)
        // Required for pico display.
        .invert_colors(mipidsi::options::ColorInversion::Inverted)
        .orientation(Orientation::new().rotate(Rotation::Deg0))
        .init(&mut Delay)
        .unwrap();

    let raw_image_data = ImageRawLE::new(include_bytes!("../assets/ferris.raw"), 86);
    let ferris = Image::new(&raw_image_data, Point::new(20, 20));
    let style = MonoTextStyle::new(&FONT_10X20, Rgb565::GREEN);

    // Clear display
    display.clear(Rgb565::BLACK).unwrap();
    // Display the image
    ferris.draw(&mut display).unwrap();
    // First text
    text_1.draw(&mut display).unwrap();

    let mut red_on_state = false;
    let mut green_on_state = false;
    let mut blue_on_state = false;
    let mut text_toggled = false;

    loop {
        info!("Start of loop!");
        let result = select4(
            button_a.wait_for_low(),
            button_b.wait_for_low(),
            button_x.wait_for_low(),
            button_y.wait_for_low(),
        )
        .await;
        match result {
            Either4::First(_) => {
                match red_on_state {
                    false => {
                        led.red_on();
                        red_on_state = true;
                    }
                    true => {
                        led.red_off();
                        red_on_state = false;
                    }
                }
                button_a.wait_for_high().await;
            }
            Either4::Second(_) => {
                match green_on_state {
                    false => {
                        led.green_on();
                        green_on_state = true;
                    }
                    true => {
                        led.green_off();
                        green_on_state = false;
                    }
                }
                button_b.wait_for_high().await;
            }
            Either4::Third(_) => {
                match blue_on_state {
                    false => {
                        led.blue_on();
                        blue_on_state = true;
                    }
                    true => {
                        led.blue_off();
                        blue_on_state = false;
                    }
                }
                button_x.wait_for_high().await;
            }
            Either4::Fourth(_) => {
                match text_toggled {
                    false => {
                        // Clear display
                        display.clear(Rgb565::CSS_PURPLE).unwrap();
                        // Display the image
                        ferris.draw(&mut display).unwrap();
                        // First text
                        text_2.draw(&mut display).unwrap();
                        text_toggled = true;
                    }
                    true => {
                        // Clear display
                        display.clear(Rgb565::BLACK).unwrap();
                        // Display the image
                        ferris.draw(&mut display).unwrap();
                        // First text
                        text_1.draw(&mut display).unwrap();
                        text_toggled = false;
                    }
                }
                button_y.wait_for_high().await;
            }
        }
    }
}
