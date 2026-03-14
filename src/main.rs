#![no_std]
#![no_main]

use crate::pimori_display::PimoriDisplayController;
use crate::pimori_display_leds::{Percentage, PimoriDisplayRgbLedController};
use crate::tasks::{
    hx710_load_cell_manager, pico_display_button_b_manager, pico_display_button_x_manager,
    pico_display_button_y_manager,
};
use crate::{candy_weigher_ui::DisplayState, tasks::pico_display_button_a_manager};
use core::cell::RefCell;
use defmt::*;
use embassy_executor::Spawner;
use embassy_futures::select::{select, select3, Either, Either3};
use embassy_rp::spi::{Config, Spi};
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::blocking_mutex::Mutex;
use embassy_sync::channel::Channel;
use embassy_time::{Duration, Instant, Timer};
use futures::future::Either as EitherFuture;

use {defmt_rtt as _, panic_probe as _};

const CHANNEL_SIZE: usize = 16;
const DEFAULT_LOLLY_WEIGHT: f32 = 25.0;

static CHANNEL: Channel<ThreadModeRawMutex, Message, CHANNEL_SIZE> = Channel::new();

mod candy_weigher_ui;
mod pimori_display;
mod pimori_display_leds;
mod tasks;

#[derive(Copy, Clone, Debug, defmt::Format)]
enum Message {
    ButtonAPressed,
    ButtonBPressed,
    ButtonXPressed,
    ButtonYPressed,
    WeightUpdate(f32),
}

struct State {
    pub tare_weight_g: f32,
    pub scale_weight_g: f32,
    pub saved_tared_scale_weight_g: f32,
    pub lolly_weight_g: f32,
    pub t_l_pressed: ButtonState,
    pub b_l_pressed: bool,
    pub t_r_pressed: bool,
    pub b_r_pressed: bool,
    pub led_state: LedState,
}

enum ButtonState {
    Off,
    PressedRecently { on_at: Instant },
}

enum LedState {
    Off,
    RedFull { on_at: Instant },
    RedHalf { half_at: Instant },
    Blue { on_at: Instant },
}

impl Default for State {
    fn default() -> Self {
        Self {
            tare_weight_g: Default::default(),
            scale_weight_g: Default::default(),
            saved_tared_scale_weight_g: Default::default(),
            lolly_weight_g: DEFAULT_LOLLY_WEIGHT,
            t_l_pressed: ButtonState::Off,
            b_l_pressed: Default::default(),
            t_r_pressed: Default::default(),
            b_r_pressed: Default::default(),
            led_state: LedState::Off,
        }
    }
}

impl State {
    fn to_display_state(&self) -> DisplayState {
        // Addition of 0.5 is a neat hack to round positive float to integer.
        let tared_scale_weight_g = self.scale_weight_g - self.tare_weight_g;
        let lolly_count = (tared_scale_weight_g / self.lolly_weight_g + 0.5) as u32;
        let prev_lolly_count = (self.saved_tared_scale_weight_g / self.lolly_weight_g + 0.5) as u32;
        DisplayState {
            scale_weight_g: self.scale_weight_g - self.tare_weight_g,
            lolly_weight_g: self.lolly_weight_g,
            lolly_count,
            lolly_count_change: lolly_count as i32 - prev_lolly_count as i32,
            t_l_pressed: matches!(self.t_l_pressed, ButtonState::PressedRecently { .. }),
            b_l_pressed: self.b_l_pressed,
            t_r_pressed: self.t_r_pressed,
            b_r_pressed: self.b_r_pressed,
        }
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let peripherals = embassy_rp::init(Default::default());
    info!("Peripherals initialised");

    let spi = Spi::new_blocking_txonly(
        peripherals.SPI0,
        peripherals.PIN_18,
        peripherals.PIN_19,
        Config::default(),
    );
    let spi_bus = Mutex::new(RefCell::new(spi));
    let mut display_buffer = [0u8; 512];

    let mut display_led_controller = PimoriDisplayRgbLedController::new(
        peripherals.PWM_SLICE3,
        peripherals.PWM_SLICE4,
        peripherals.PIN_6,
        peripherals.PIN_7,
        peripherals.PIN_8,
    );
    info!("LED controller initialised");

    let mut display_controller = PimoriDisplayController::new(
        peripherals.PIN_16,
        peripherals.PIN_17,
        peripherals.PIN_20,
        &spi_bus,
        &mut display_buffer,
    );
    info!("Display controller initialised");

    spawner
        .spawn(pico_display_button_a_manager(
            peripherals.PIN_12,
            CHANNEL.sender(),
        ))
        .unwrap();
    spawner
        .spawn(pico_display_button_b_manager(
            peripherals.PIN_13,
            CHANNEL.sender(),
        ))
        .unwrap();
    spawner
        .spawn(pico_display_button_x_manager(
            peripherals.PIN_14,
            CHANNEL.sender(),
        ))
        .unwrap();
    spawner
        .spawn(pico_display_button_y_manager(
            peripherals.PIN_15,
            CHANNEL.sender(),
        ))
        .unwrap();
    spawner
        .spawn(hx710_load_cell_manager(CHANNEL.sender()))
        .unwrap();
    info!("Tasks spawned");

    let mut state = State::default();

    display_controller
        .draw_to_framebuffer(|display| candy_weigher_ui::draw(&state.to_display_state(), display));
    display_controller.flush_buffer_to_screen();
    let rx = CHANNEL.receiver();
    info!("Initial UI drawn, entering event loop");
    loop {
        // Interleave LED state updates.
        let led_fut = match state.led_state {
            LedState::Off => EitherFuture::Left(core::future::pending()),
            LedState::RedFull { on_at } => {
                const MAX_ON_TIME: Duration = Duration::from_millis(250);
                let on_for = Instant::now() - on_at;
                let rem_full = MAX_ON_TIME
                    .checked_sub(on_for)
                    .unwrap_or(Duration::from_millis(0));
                EitherFuture::Right(Timer::after(rem_full))
            }
            LedState::RedHalf { half_at } => {
                const MAX_ON_TIME: Duration = Duration::from_millis(250);
                let half_for = Instant::now() - half_at;
                let rem_on = MAX_ON_TIME
                    .checked_sub(half_for)
                    .unwrap_or(Duration::from_millis(0));
                EitherFuture::Right(Timer::after(rem_on))
            }
            LedState::Blue { on_at } => {
                const MAX_ON_TIME: Duration = Duration::from_millis(500);
                let on_for = Instant::now() - on_at;
                let rem_on = MAX_ON_TIME
                    .checked_sub(on_for)
                    .unwrap_or(Duration::from_millis(0));
                EitherFuture::Right(Timer::after(rem_on))
            }
        };
        // Interleave button state updates.
        let t_l_button_fut = match state.t_l_pressed {
            ButtonState::Off => EitherFuture::Left(core::future::pending()),
            ButtonState::PressedRecently { on_at } => {
                const MAX_ON_TIME: Duration = Duration::from_millis(100);
                let on_for = Instant::now() - on_at;
                let rem_on = MAX_ON_TIME
                    .checked_sub(on_for)
                    .unwrap_or(Duration::from_millis(0));
                EitherFuture::Right(Timer::after(rem_on))
            }
        };
        let result = select3(rx.receive(), led_fut, t_l_button_fut).await;
        match result {
            Either3::First(Message::ButtonAPressed) => {
                state.lolly_weight_g += 0.1;
                state.t_l_pressed = ButtonState::PressedRecently {
                    on_at: Instant::now(),
                };
            }
            Either3::First(Message::ButtonBPressed) => {
                state.lolly_weight_g -= 0.1;
                state.b_l_pressed = !state.b_l_pressed;
            }
            Either3::First(Message::ButtonXPressed) => {
                state.saved_tared_scale_weight_g = state.scale_weight_g - state.tare_weight_g;
                state.t_r_pressed = !state.t_r_pressed;
            }
            Either3::First(Message::ButtonYPressed) => {
                state.tare_weight_g = state.scale_weight_g;
                state.b_r_pressed = !state.b_r_pressed;
            }
            Either3::First(Message::WeightUpdate(w)) => {
                if w < state.scale_weight_g {
                    state.led_state = LedState::RedFull {
                        on_at: Instant::now(),
                    }
                }
                if w > state.scale_weight_g {
                    state.led_state = LedState::Blue {
                        on_at: Instant::now(),
                    }
                }
                state.scale_weight_g = w
            }
            Either3::Second(_) => match state.led_state {
                LedState::Off => core::unreachable!(),
                LedState::RedFull { .. } => {
                    state.led_state = LedState::RedHalf {
                        half_at: Instant::now(),
                    }
                }
                LedState::RedHalf { .. } => state.led_state = LedState::Off,
                LedState::Blue { .. } => state.led_state = LedState::Off,
            },
            Either3::Third(_) => state.t_l_pressed = ButtonState::Off,
        }
        match state.led_state {
            LedState::Off => display_led_controller.all_off(),
            LedState::RedFull { .. } => display_led_controller.set_red(Percentage(100)),
            LedState::RedHalf { .. } => display_led_controller.set_red(Percentage(50)),
            LedState::Blue { .. } => display_led_controller.set_blue(Percentage(50)),
        };
        display_controller.draw_to_framebuffer(|display| {
            candy_weigher_ui::draw(&state.to_display_state(), display)
        });
        display_controller.flush_buffer_to_screen();
    }
}
