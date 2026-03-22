use crate::pimoroni_display_leds::Percentage;
use core::cell::RefCell;
use defmt::debug;
use embassy_embedded_hal::shared_bus::blocking::spi::SpiDeviceWithConfig;
use embassy_rp::{
    gpio::{Level, Output},
    peripherals::{PIN_16, PIN_17, PIN_20, PWM_SLICE2, SPI0},
    pwm::{self, Pwm},
    spi::{self, Async, Spi},
    Peri,
};
use embassy_sync::blocking_mutex::{raw::NoopRawMutex, Mutex};
use embassy_time::Delay;
use embedded_graphics::{
    framebuffer::Framebuffer,
    pixelcolor::{
        raw::{BigEndian, RawU16},
        Rgb565,
    },
};
use mipidsi::{
    interface::SpiInterface,
    models::ST7789,
    options::{Orientation, Rotation},
    Builder, Display, NoResetPin,
};

pub const DISPLAY_FREQ: u32 = 16_000_000;
pub const DISPLAY_H: u16 = 135;
pub const DISPLAY_W: u16 = 240;
pub const DISPLAY_H_AS_USIZE: usize = DISPLAY_H as usize;
pub const DISPLAY_W_AS_USIZE: usize = DISPLAY_W as usize;
pub const DISPLAY_PX: usize = DISPLAY_H_AS_USIZE * DISPLAY_W_AS_USIZE;
pub const DISPLAY_BYTES: usize = DISPLAY_PX * 2;
// Should correspond to 5 kHz.
pub const BACKLIGHT_PWM_TOP: u16 = 24_999;

type Spi0<'a> = Spi<'a, SPI0, Async>;
type PimoroniDisplay<'a> = Display<
    SpiInterface<'a, SpiDeviceWithConfig<'a, NoopRawMutex, Spi0<'a>, Output<'a>>, Output<'a>>,
    ST7789,
    NoResetPin,
>;

pub struct PimoroniDisplayController<'a> {
    display: PimoroniDisplay<'a>,
    framebuffer: Framebuffer<
        Rgb565,
        RawU16,
        BigEndian,
        DISPLAY_W_AS_USIZE,
        DISPLAY_H_AS_USIZE,
        DISPLAY_BYTES,
    >,
}
pub struct PimoroniDisplayBacklightController<'a> {
    backlight: Pwm<'a>,
    backlight_conf: pwm::Config,
}

impl<'a> PimoroniDisplayBacklightController<'a> {
    pub fn new(pin20: Peri<'a, PIN_20>, slice2: Peri<'a, PWM_SLICE2>) -> Self {
        let mut backlight_pwm_conf = pwm::Config::default();
        backlight_pwm_conf.top = BACKLIGHT_PWM_TOP;
        let backlight = Pwm::new_output_a(slice2, pin20, backlight_pwm_conf.clone());
        Self {
            backlight,
            backlight_conf: backlight_pwm_conf,
        }
    }
    pub fn turn_off_display(&mut self) {
        self.backlight_conf.compare_a = 0;
        self.backlight.set_config(&self.backlight_conf);
    }
    pub fn turn_on_display(&mut self, percentage: Percentage) {
        self.backlight_conf.compare_a = BACKLIGHT_PWM_TOP / 100 * percentage.0;
        self.backlight.set_config(&self.backlight_conf);
    }
}
impl<'a> PimoroniDisplayController<'a> {
    pub fn new(
        pin16: Peri<'a, PIN_16>,
        pin17: Peri<'a, PIN_17>,
        spi_bus: &'a Mutex<NoopRawMutex, RefCell<Spi0<'a>>>,
        buffer: &'a mut [u8; 512],
    ) -> Self {
        // dcx is the data command/control output required for the display
        // 0 = command, 1 = data
        let display_dcx = Output::new(pin16, Level::Low);
        // Chip select pin for SPI
        let display_spi_cs = Output::new(pin17, Level::High);

        // Display-specifi SPI config
        let mut display_config = spi::Config::default();
        display_config.frequency = DISPLAY_FREQ;
        display_config.phase = spi::Phase::CaptureOnSecondTransition;
        display_config.polarity = spi::Polarity::IdleHigh;

        // SPI device for display
        let display_spi = SpiDeviceWithConfig::new(spi_bus, display_spi_cs, display_config);

        // Display interface abstraction
        // TODO: consider lcd-async crate to use framebuffer approach.
        let di = SpiInterface::new(display_spi, display_dcx, buffer);

        let display = Builder::new(ST7789, di)
            // Magic numbers for pico display offset.
            .display_offset(52, 40)
            // Actual w/h for pico display.
            .display_size(DISPLAY_H, DISPLAY_W)
            // Required for pico display.
            .invert_colors(mipidsi::options::ColorInversion::Inverted)
            // This puts button A in top left and button Y in bottom right.
            .orientation(Orientation::new().rotate(Rotation::Deg90))
            .init(&mut Delay)
            .unwrap();

        let framebuffer = Framebuffer::new();
        Self {
            display,
            framebuffer,
        }
    }
    pub fn draw_via_framebuffer(
        &mut self,
        draw_fn: impl FnOnce(
            &mut Framebuffer<
                Rgb565,
                RawU16,
                BigEndian,
                DISPLAY_W_AS_USIZE,
                DISPLAY_H_AS_USIZE,
                DISPLAY_BYTES,
            >,
        ),
    ) {
        draw_fn(&mut self.framebuffer);
        self.flush_buffer_to_screen();
    }
    fn flush_buffer_to_screen(&mut self) {
        debug!("Flushing framebuffer to screen");
        // TODO: Use DMA here instead of synchronous write.
        let pixels = self
            .framebuffer
            .data()
            .chunks_exact(2)
            .map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]))
            .map(RawU16::new)
            .map(Rgb565::from);

        // Note - in testing this shaves off 15ms of 75ms compared to
        // `self.framebuffer.as_image.draw(..)`
        self.display
            .set_pixels(0, 0, DISPLAY_W - 1, DISPLAY_H - 1, pixels)
            .unwrap();
    }
}
