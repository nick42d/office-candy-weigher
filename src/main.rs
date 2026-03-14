#![no_std]
#![no_main]

use crate::pimori_display::PimoriDisplayController;
use crate::pimori_display_leds::{Percentage, PimoriDisplayRgbLedController};
use crate::round_robin_select::{round_robin_select3, PollFirst3};
use crate::state::{LedState, MomentaryButtonState, State};
use crate::tasks::{
    hx710_load_cell_manager_rotary_encoder, pico_display_button_b_manager,
    pico_display_button_x_manager, pico_display_button_y_manager,
};
use crate::{candy_weigher_ui::DisplayState, tasks::pico_display_button_a_manager};
use core::cell::RefCell;
use defmt::*;
use embassy_executor::Spawner;
use embassy_futures::select::{select4, Either3};
use embassy_rp::spi::{Config, Spi};
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::blocking_mutex::Mutex;
use embassy_sync::channel::Channel;
use embassy_time::{Duration, Instant, Timer};

use {defmt_rtt as _, panic_probe as _};

const CHANNEL_SIZE: usize = 16;
const DEFAULT_LOLLY_WEIGHT: f32 = 25.0;

static CHANNEL: Channel<ThreadModeRawMutex, Message, CHANNEL_SIZE> = Channel::new();

mod candy_weigher_ui;
mod pimori_display;
mod pimori_display_leds;
mod round_robin_select;
mod state;
mod tasks;

#[derive(Copy, Clone, Debug, defmt::Format)]
enum Message {
    ButtonAPressed,
    ButtonBPressed,
    ButtonXPressed,
    ButtonYPressed,
    WeightUpdate(f32),
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
        .spawn(hx710_load_cell_manager_rotary_encoder(
            peripherals.PIN_26,
            peripherals.PIN_27,
            peripherals.PIO0,
            CHANNEL.sender(),
        ))
        .unwrap();
    info!("Tasks spawned");

    let mut state = State::default();

    display_controller
        .draw_to_framebuffer(|display| candy_weigher_ui::draw(&state.to_display_state(), display));
    display_controller.flush_buffer_to_screen();
    let rx = CHANNEL.receiver();
    info!("Initial UI drawn, entering event loop");
    const MAX_MOMENTARY_BUTTON_ON_TIME: Duration = Duration::from_millis(100);
    const MAX_LED_ON_TIME: Duration = Duration::from_millis(500);
    let mut poll_first = PollFirst3::A;
    loop {
        // Interleave LED state updates.
        let led_animation_future = state.led_state.next_timer(MAX_LED_ON_TIME);
        // Interleave button state updates.
        let momentary_button_animation_future = select4(
            state.t_l_pressed.next_timer(MAX_MOMENTARY_BUTTON_ON_TIME),
            state.t_r_pressed.next_timer(MAX_MOMENTARY_BUTTON_ON_TIME),
            state.b_l_pressed.next_timer(MAX_MOMENTARY_BUTTON_ON_TIME),
            state.b_r_pressed.next_timer(MAX_MOMENTARY_BUTTON_ON_TIME),
        );
        let result = round_robin_select3(
            poll_first,
            rx.receive(),
            led_animation_future,
            momentary_button_animation_future,
        )
        .await;
        match result {
            Either3::First(Message::ButtonAPressed) => {
                state.lolly_weight_g += 0.1;
                state.t_l_pressed = MomentaryButtonState::PressedRecently {
                    on_at: Instant::now(),
                };
            }
            Either3::First(Message::ButtonBPressed) => {
                state.lolly_weight_g -= 0.1;
                state.b_l_pressed = MomentaryButtonState::PressedRecently {
                    on_at: Instant::now(),
                };
            }
            Either3::First(Message::ButtonXPressed) => {
                state.saved_tared_scale_weight_g = state.scale_weight_g - state.tare_weight_g;
                state.t_r_pressed = MomentaryButtonState::PressedRecently {
                    on_at: Instant::now(),
                };
            }
            Either3::First(Message::ButtonYPressed) => {
                state.tare_weight_g = state.scale_weight_g;
                state.b_r_pressed = MomentaryButtonState::PressedRecently {
                    on_at: Instant::now(),
                };
            }
            Either3::First(Message::WeightUpdate(w)) => {
                if w < state.scale_weight_g {
                    state.led_state = LedState::Red {
                        total_steps: 4,
                        current_step: 0,
                        current_step_at: Instant::now(),
                    };
                }
                if w > state.scale_weight_g {
                    state.led_state = LedState::Blue {
                        total_steps: 4,
                        current_step: 0,
                        current_step_at: Instant::now(),
                    };
                }
                state.scale_weight_g = w;
            }
            Either3::Second(_) => state.led_state = state.led_state.next(),
            Either3::Third(s) => match s {
                embassy_futures::select::Either4::First(_) => {
                    state.t_l_pressed = state.t_l_pressed.next()
                }
                embassy_futures::select::Either4::Second(_) => {
                    state.t_r_pressed = state.t_r_pressed.next()
                }
                embassy_futures::select::Either4::Third(_) => {
                    state.b_l_pressed = state.b_l_pressed.next()
                }
                embassy_futures::select::Either4::Fourth(_) => {
                    state.b_r_pressed = state.b_r_pressed.next()
                }
            },
        }
        match state.led_state {
            LedState::Off => display_led_controller.all_off(),
            LedState::Red {
                total_steps,
                current_step,
                ..
            } => {
                display_led_controller.blue_off();
                display_led_controller.set_red(Percentage(
                    100 * total_steps.saturating_sub(current_step) / total_steps,
                ))
            }
            LedState::Blue {
                total_steps,
                current_step,
                ..
            } => {
                display_led_controller.red_off();
                display_led_controller.set_blue(Percentage(
                    100 * total_steps.saturating_sub(current_step) / total_steps,
                ))
            }
        };
        display_controller.draw_to_framebuffer(|display| {
            candy_weigher_ui::draw(&state.to_display_state(), display)
        });
        display_controller.flush_buffer_to_screen();
        poll_first.next();
    }
}
