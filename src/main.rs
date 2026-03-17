#![no_std]
#![no_main]

use crate::config_consts::{
    MAX_LED_ON_TIME, MAX_MOMENTARY_BUTTON_ON_TIME, TOTAL_LED_FADEOUT_STEPS,
};
use crate::pimoroni_display::PimoroniDisplayController;
use crate::pimoroni_display_leds::{Percentage, PimoroniDisplayRgbLedController};
use crate::round_robin_select::{round_robin_select3, PollFirst3};
use crate::state::{output_state, LedState, MomentaryButtonState, State};
use crate::tasks::pico_display_button_a_manager;
use crate::tasks::{
    hx710_load_cell_manager_rotary_encoder, pico_display_button_b_manager,
    pico_display_button_x_manager, pico_display_button_y_manager,
};
use core::cell::RefCell;
use defmt::*;
use embassy_executor::Spawner;
use embassy_futures::select::{select4, Either3};
use embassy_rp::spi::{Config, Spi};
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::blocking_mutex::Mutex;
use embassy_sync::channel::Channel;

use {defmt_rtt as _, panic_probe as _};

const CHANNEL_SIZE: usize = 16;

static CHANNEL: Channel<ThreadModeRawMutex, Message, CHANNEL_SIZE> = Channel::new();

mod config_consts {
    use embassy_time::Duration;
    use embedded_graphics::{pixelcolor::Rgb565, prelude::RgbColor};

    pub const DEFAULT_LOLLY_WEIGHT: f32 = 25.0;
    pub const TOTAL_LED_FADEOUT_STEPS: u16 = 8;
    pub const MAX_MOMENTARY_BUTTON_ON_TIME: Duration = Duration::from_millis(100);
    pub const MAX_LED_ON_TIME: Duration = Duration::from_millis(500);
    pub const BUTTON_TOOLTIP_COLOUR: Rgb565 = Rgb565::GREEN;
    pub const BUTTON_SEMICIRCLE_COLOUR: Rgb565 = Rgb565::WHITE;
    pub const SEMICIRCLE_DIAMETER: u32 = 44;
}
mod candy_weigher_ui;
mod pimoroni_display;
mod pimoroni_display_leds;
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

    // TODO: Consider if interrupt handler needs to be set up for DMA_CH0
    let spi = Spi::new_txonly(
        peripherals.SPI0,
        peripherals.PIN_18,
        peripherals.PIN_19,
        peripherals.DMA_CH0,
        Config::default(),
    );
    let spi_bus = Mutex::new(RefCell::new(spi));
    let mut display_buffer = [0u8; 512];

    let mut display_led_controller = PimoroniDisplayRgbLedController::new(
        peripherals.PWM_SLICE3,
        peripherals.PWM_SLICE4,
        peripherals.PIN_6,
        peripherals.PIN_7,
        peripherals.PIN_8,
    );
    info!("LED controller initialised");

    let mut display_controller = PimoroniDisplayController::new(
        peripherals.PIN_16,
        peripherals.PIN_17,
        peripherals.PIN_20,
        peripherals.PWM_SLICE2,
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
    output_state(
        &mut state,
        &mut display_controller,
        &mut display_led_controller,
    );
    display_controller.turn_on_display(Percentage(100));

    let rx = CHANNEL.receiver();
    info!("Initial UI drawn, entering event loop");
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
            &mut poll_first,
            rx.receive(),
            led_animation_future,
            momentary_button_animation_future,
        )
        .await;
        match result {
            Either3::First(message) => state.handle_message(message),
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
        output_state(
            &mut state,
            &mut display_controller,
            &mut display_led_controller,
        );
    }
}
