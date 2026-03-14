#![no_std]
#![no_main]

use core::cell::RefCell;
use defmt::*;
use embassy_rp::peripherals::{PIN_12, PIN_13, PIN_14, PIN_15, SPI0};
use embassy_sync::channel::{Channel, Receiver, Sender};
// use display_interface_spi::SPIInterface;
use crate::display_leds::DisplayRgbLedController;
use core::fmt::Write;
use embassy_embedded_hal::shared_bus::blocking::spi::SpiDeviceWithConfig;
use embassy_executor::Spawner;
use embassy_futures::select::{select, select4, Either4};
use embassy_rp::gpio::{Input, Level, Output, Pull};
use embassy_rp::spi::{Blocking, Config, Spi};
use embassy_rp::{config, spi, Peri, Peripherals};
use embassy_sync::blocking_mutex::raw::{NoopRawMutex, RawMutex, ThreadModeRawMutex};
use embassy_sync::blocking_mutex::Mutex;
use embassy_time::{Delay, Timer};
use embedded_graphics::image::{Image, ImageRawLE};
use embedded_graphics::mono_font::ascii::FONT_10X20;
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{
    Arc, Circle, PrimitiveStyle, PrimitiveStyleBuilder, Rectangle, StyledDrawable,
};
use embedded_graphics::text::Text;
use mipidsi::models::ST7789;
use mipidsi::options::{Orientation, Rotation};
use mipidsi::{Builder, Display};

use {defmt_rtt as _, panic_probe as _};

const DISPLAY_FREQ: u32 = 16_000_000;
const DISPLAY_H: u16 = 135;
const DISPLAY_W: u16 = 240;

static LED_CHANNEL: Channel<ThreadModeRawMutex, ButtonPressedMessage, 8> = Channel::new();
static DISPLAY_CHANNEL: Channel<ThreadModeRawMutex, ButtonPressedMessage, 8> = Channel::new();

struct Percentage(pub u16);

mod display_leds {
    use embassy_rp::peripherals::{PIN_6, PIN_7, PIN_8, PWM_SLICE3, PWM_SLICE4};
    use embassy_rp::pwm::{self, Pwm};
    use embassy_rp::Peri;

    use crate::Percentage;
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
        pub fn set_red(&mut self, brightness: Percentage) {
            self.rg_conf.compare_a = 0xffff * brightness.0 / 1000;
            self.rg_pwm_slice.set_config(&self.rg_conf);
        }
        pub fn red_on(&mut self) {
            self.rg_conf.compare_a = 0xffff;
            self.rg_pwm_slice.set_config(&self.rg_conf);
        }
        pub fn red_off(&mut self) {
            self.rg_conf.compare_a = 0x0000;
            self.rg_pwm_slice.set_config(&self.rg_conf);
        }
        pub fn set_green(&mut self, brightness: Percentage) {
            self.rg_conf.compare_b = 0xffff * brightness.0 / 1000;
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
        pub fn set_blue(&mut self, brightness: Percentage) {
            self.rg_conf.compare_a = 0xffff * brightness.0 / 1000;
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

struct DisplayState {
    scale_weight_g: f32,
    lolly_weight_g: f32,
    lolly_count: u32,
    lolly_count_change: u32,
    t_l_pressed: bool,
    b_l_pressed: bool,
    t_r_pressed: bool,
    b_r_pressed: bool,
}

fn draw<D>(state: &DisplayState, display: &mut D)
where
    D: DrawTarget<Color = Rgb565>,
    <D as embedded_graphics::draw_target::DrawTarget>::Error: core::fmt::Debug,
{
    let arc_diameter = 40;
    let circle_style = PrimitiveStyle::with_fill(Rgb565::WHITE);
    let arc_style = PrimitiveStyle::with_stroke(Rgb565::WHITE, 2);
    let arc_t_l = Arc::with_center(Point::new(0, 0), arc_diameter, 0.0.deg(), 90.0.deg())
        .into_styled(arc_style);
    let arc_b_l = Arc::with_center(
        Point::new(0, DISPLAY_H as i32),
        arc_diameter,
        270.0.deg(),
        90.0.deg(),
    )
    .into_styled(arc_style);
    let arc_t_r = Arc::with_center(
        Point::new(DISPLAY_W as i32, 0),
        arc_diameter,
        90.0.deg(),
        90.0.deg(),
    )
    .into_styled(arc_style);
    let arc_b_r = Arc::with_center(
        Point::new(DISPLAY_W as i32, DISPLAY_H as i32),
        arc_diameter,
        180.0.deg(),
        90.0.deg(),
    )
    .into_styled(arc_style);
    let circle_t_l = Circle::with_center(Point::new(0, 0), arc_diameter).into_styled(circle_style);
    let circle_b_l = Circle::with_center(Point::new(0, DISPLAY_H as i32), arc_diameter)
        .into_styled(circle_style);
    let circle_t_r = Circle::with_center(Point::new(DISPLAY_W as i32, 0), arc_diameter)
        .into_styled(circle_style);
    let circle_b_r =
        Circle::with_center(Point::new(DISPLAY_W as i32, DISPLAY_H as i32), arc_diameter)
            .into_styled(circle_style);
    let button_text_style = MonoTextStyle::new(&FONT_10X20, Rgb565::GREEN);
    let text_t_l = Text::new("+", Point::new(28, 20), button_text_style);
    let text_b_l = Text::new(
        "-",
        Point::new(28, DISPLAY_H as i32 - 10),
        button_text_style,
    );
    let text_t_r = Text::with_alignment(
        "R",
        Point::new(DISPLAY_W as i32 - 28, 20),
        button_text_style,
        embedded_graphics::text::Alignment::Right,
    );
    let text_b_r = Text::with_alignment(
        "T",
        Point::new(DISPLAY_W as i32 - 28, DISPLAY_H as i32 - 10),
        button_text_style,
        embedded_graphics::text::Alignment::Right,
    );
    let weight_text_style = MonoTextStyle::new(&FONT_10X20, Rgb565::BLUE);
    let mut scale_weight_str = heapless::String::<30>::new();
    let mut lolly_weight_str = heapless::String::<30>::new();
    let mut lolly_count_str = heapless::String::<30>::new();
    let mut lolly_count_change_str = heapless::String::<30>::new();
    core::write!(
        &mut scale_weight_str,
        "Scale weight: {:.1}",
        state.scale_weight_g
    )
    .unwrap();
    core::write!(
        &mut lolly_weight_str,
        "Lolly weight: {:.1}",
        state.lolly_weight_g
    )
    .unwrap();
    core::write!(&mut lolly_count_str, "Lolly count: {}", state.lolly_count).unwrap();
    core::write!(
        &mut lolly_count_change_str,
        "Lolly change: {}",
        state.lolly_count_change
    )
    .unwrap();
    let text_scale_weight = Text::new(&scale_weight_str, Point::new(28, 40), weight_text_style);
    let text_lolly_weight = Text::new(&lolly_weight_str, Point::new(28, 60), weight_text_style);
    let text_lolly_count = Text::new(&lolly_count_str, Point::new(28, 80), weight_text_style);
    let text_lolly_change = Text::new(
        &lolly_count_change_str,
        Point::new(28, 100),
        weight_text_style,
    );

    display.clear(Rgb565::BLACK).unwrap();

    if state.t_l_pressed {
        circle_t_l.draw(display).unwrap();
    } else {
        arc_t_l.draw(display).unwrap();
    };
    if state.t_r_pressed {
        circle_t_r.draw(display).unwrap();
    } else {
        arc_t_r.draw(display).unwrap();
    };
    if state.b_l_pressed {
        circle_b_l.draw(display).unwrap();
    } else {
        arc_b_l.draw(display).unwrap();
    };
    if state.b_r_pressed {
        circle_b_r.draw(display).unwrap();
    } else {
        arc_b_r.draw(display).unwrap();
    };
    text_t_l.draw(display).unwrap();
    text_b_l.draw(display).unwrap();
    text_t_r.draw(display).unwrap();
    text_b_r.draw(display).unwrap();
    text_scale_weight.draw(display).unwrap();
    text_lolly_weight.draw(display).unwrap();
    text_lolly_count.draw(display).unwrap();
    text_lolly_change.draw(display).unwrap();
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    info!("Hello World!");

    let bl = p.PIN_20;
    let display_cs = p.PIN_17;
    let dcx = p.PIN_16;
    let mosi = p.PIN_19;
    let clk = p.PIN_18;

    let led = DisplayRgbLedController::new(p.PWM_SLICE3, p.PWM_SLICE4, p.PIN_6, p.PIN_7, p.PIN_8);

    let button_a = Input::new(p.PIN_12, Pull::Up);
    let button_b = Input::new(p.PIN_13, Pull::Up);
    let button_x = Input::new(p.PIN_14, Pull::Up);
    let button_y = Input::new(p.PIN_15, Pull::Up);

    // let miso = p.PIN_12;
    // let touch_cs = p.PIN_16;
    // let touch_irq = p.PIN_17;

    // Enable LCD backlight - required for screen to operate
    let _bl = Output::new(bl, Level::High);

    // dcx: 0 = command, 1 = data
    let dcx = Output::new(dcx, Level::Low);

    let display_cs_output = Output::new(display_cs, Level::High);

    // create SPI
    let mut display_config = spi::Config::default();
    display_config.frequency = DISPLAY_FREQ;
    display_config.phase = spi::Phase::CaptureOnSecondTransition;
    display_config.polarity = spi::Polarity::IdleHigh;
    let spi = Spi::new_blocking_txonly(p.SPI0, clk, mosi, display_config.clone());

    spawner
        .spawn(input_manager(
            button_a,
            button_b,
            button_x,
            button_y,
            LED_CHANNEL.sender(),
            DISPLAY_CHANNEL.sender(),
        ))
        .unwrap();
    spawner
        .spawn(lights_manager(led, LED_CHANNEL.receiver()))
        .unwrap();
    spawner
        .spawn(display_manager(
            spawner,
            spi,
            dcx,
            display_cs_output,
            display_config,
            DISPLAY_CHANNEL.receiver(),
        ))
        .unwrap();
}

enum ButtonPressedMessage {
    PressedA,
    PressedB,
    PressedX,
    PressedY,
}

enum ButtonTimeoutMessage {
    A,
}

#[embassy_executor::task]
async fn button_pressed_timeout(tx: Sender<'static, ThreadModeRawMutex, ButtonTimeoutMessage, 8>) {
    embassy_time::Timer::after_millis(100).await;
    tx.send(ButtonTimeoutMessage::A).await;
}
#[embassy_executor::task]
async fn display_manager(
    spawner: Spawner,
    spi: Spi<'static, SPI0, Blocking>,
    dcx: Output<'static>,
    display_cs_output: Output<'static>,
    display_config: Config,
    rx: Receiver<'static, ThreadModeRawMutex, ButtonPressedMessage, 8>,
) {
    let spi_bus: Mutex<NoopRawMutex, _> = Mutex::new(RefCell::new(spi));
    let display_spi = SpiDeviceWithConfig::new(&spi_bus, display_cs_output, display_config);

    let mut buffer = [0_u8; 512];

    // display interface abstraction from SPI and DC
    // TODO: consider lcd-async crate to use framebuffer approach.
    let di = mipidsi::interface::SpiInterface::new(display_spi, dcx, &mut buffer);

    // Define the display from the display interface and initialize it
    let mut display = Builder::new(ST7789, di)
        // Magic numbers for pico display offset.
        .display_offset(52, 40)
        // Actual w/h for pico display.
        .display_size(DISPLAY_H, DISPLAY_W)
        // Required for pico display.
        .invert_colors(mipidsi::options::ColorInversion::Inverted)
        .orientation(Orientation::new().rotate(Rotation::Deg90))
        .init(&mut Delay)
        .unwrap();

    static TIMEOUT_CHANNEL: Channel<ThreadModeRawMutex, ButtonTimeoutMessage, 8> = Channel::new();
    let tx2 = TIMEOUT_CHANNEL.sender();
    let rx2 = TIMEOUT_CHANNEL.receiver();

    let mut state = DisplayState {
        scale_weight_g: 300.0,
        lolly_weight_g: 25.0,
        lolly_count: 17,
        lolly_count_change: 3,
        t_l_pressed: false,
        b_l_pressed: false,
        t_r_pressed: false,
        b_r_pressed: false,
    };
    draw(&state, &mut display);
    loop {
        let result = select(rx.receive(), rx2.receive()).await;
        match result {
            embassy_futures::select::Either::First(ButtonPressedMessage::PressedA) => {
                state.lolly_weight_g += 0.1;
                state.t_l_pressed = true;
                spawner.spawn(button_pressed_timeout(tx2)).unwrap();
                draw(&state, &mut display);
            }
            embassy_futures::select::Either::First(ButtonPressedMessage::PressedB) => {
                state.lolly_weight_g -= 0.1;
                state.b_l_pressed = !state.b_l_pressed;
                draw(&state, &mut display);
            }
            embassy_futures::select::Either::First(ButtonPressedMessage::PressedX) => {
                state.lolly_count_change = 0;
                state.t_r_pressed = !state.t_r_pressed;
                draw(&state, &mut display);
            }
            embassy_futures::select::Either::First(ButtonPressedMessage::PressedY) => {
                state.scale_weight_g = 0.0;
                state.b_r_pressed = !state.b_r_pressed;
                draw(&state, &mut display);
            }
            embassy_futures::select::Either::Second(ButtonTimeoutMessage::A) => {
                state.t_l_pressed = false;
                draw(&state, &mut display);
            }
        }
    }
}

#[embassy_executor::task]
async fn lights_manager(
    mut led: DisplayRgbLedController<'static>,
    rx: Receiver<'static, ThreadModeRawMutex, ButtonPressedMessage, 8>,
) {
    let mut red_on_state = false;
    let mut green_on_state = false;
    let mut blue_on_state = false;
    loop {
        match rx.receive().await {
            ButtonPressedMessage::PressedA => match red_on_state {
                false => {
                    led.red_on();
                    red_on_state = true;
                }
                true => {
                    led.red_off();
                    red_on_state = false;
                }
            },
            ButtonPressedMessage::PressedB => match green_on_state {
                false => {
                    led.green_on();
                    green_on_state = true;
                }
                true => {
                    led.green_off();
                    green_on_state = false;
                }
            },
            ButtonPressedMessage::PressedX => match blue_on_state {
                false => {
                    led.blue_on();
                    blue_on_state = true;
                }
                true => {
                    led.blue_off();
                    blue_on_state = false;
                }
            },
            ButtonPressedMessage::PressedY => info!("Pressed Y"),
        }
    }
}

#[embassy_executor::task]
async fn input_manager(
    mut button_a: Input<'static>,
    mut button_b: Input<'static>,
    mut button_x: Input<'static>,
    mut button_y: Input<'static>,
    tx: Sender<'static, ThreadModeRawMutex, ButtonPressedMessage, 8>,
    tx2: Sender<'static, ThreadModeRawMutex, ButtonPressedMessage, 8>,
) {
    loop {
        let result = select4(
            button_a.wait_for_low(),
            button_b.wait_for_low(),
            button_x.wait_for_low(),
            button_y.wait_for_low(),
        )
        .await;
        match result {
            Either4::First(_) => {
                tx2.send(ButtonPressedMessage::PressedA).await;
                button_a.wait_for_high().await;
            }
            Either4::Second(_) => {
                tx2.send(ButtonPressedMessage::PressedB).await;
                button_b.wait_for_high().await;
            }
            Either4::Third(_) => {
                tx2.send(ButtonPressedMessage::PressedX).await;
                button_x.wait_for_high().await;
            }
            Either4::Fourth(_) => {
                tx.send(ButtonPressedMessage::PressedY).await;
                tx2.send(ButtonPressedMessage::PressedY).await;
                button_y.wait_for_high().await;
            }
        }
    }
}

#[embassy_executor::task]
async fn pico_display_button_a_manager(
    pin12: Peri<'static, PIN_12>,
    tx: Sender<'static, ThreadModeRawMutex, (), 8>,
) {
    let button_a = Input::new(pin12, Pull::Up);
    manage_button(button_a, (), tx).await;
}
#[embassy_executor::task]
async fn pico_display_button_b_manager(
    pin13: Peri<'static, PIN_13>,
    tx: Sender<'static, ThreadModeRawMutex, (), 8>,
) {
    let button_b = Input::new(pin13, Pull::Up);
    manage_button(button_b, (), tx).await;
}
#[embassy_executor::task]
async fn pico_display_button_x_manager(
    pin14: Peri<'static, PIN_14>,
    tx: Sender<'static, ThreadModeRawMutex, (), 8>,
) {
    let button_x = Input::new(pin14, Pull::Up);
    manage_button(button_x, (), tx).await;
}
#[embassy_executor::task]
async fn pico_display_button_y_manager(
    pin15: Peri<'static, PIN_15>,
    tx: Sender<'static, ThreadModeRawMutex, (), 8>,
) {
    let button_y = Input::new(pin15, Pull::Up);
    manage_button(button_y, (), tx).await;
}

async fn manage_button<'a, M, Mutex, const CHANNEL_SIZE: usize>(
    mut button: Input<'static>,
    pressed_message: M,
    tx: Sender<'a, Mutex, M, CHANNEL_SIZE>,
) where
    M: Copy,
    Mutex: RawMutex,
{
    loop {
        button.wait_for_low().await;
        tx.send(pressed_message).await;
        button.wait_for_high().await;
    }
}
