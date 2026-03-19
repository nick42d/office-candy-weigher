#![no_std]
#![no_main]

use crate::config_consts::{
    MAX_LED_ON_TIME, MAX_MOMENTARY_BUTTON_ON_TIME, TOTAL_LED_FADEOUT_STEPS,
};
use crate::pimoroni_display::PimoroniDisplayController;
use crate::pimoroni_display_leds::{Percentage, PimoroniDisplayRgbLedController};
use crate::round_robin_select::{
    round_robin_select3, round_robin_select_array, PollFirst2, PollFirst3,
};
use crate::state::{output_state, LedState, MomentaryButtonState, State};
use crate::tasks::{hx710_load_cell_manager, pico_display_button_a_manager};
use crate::tasks::{
    hx710_load_cell_manager_rotary_encoder, pico_display_button_b_manager,
    pico_display_button_x_manager, pico_display_button_y_manager,
};
use core::cell::RefCell;
use core::convert::identity;
use defmt::*;
use embassy_executor::Spawner;
use embassy_futures::select::{select4, Either};
use embassy_rp::spi::{Config, Spi};
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::blocking_mutex::Mutex;
use embassy_sync::channel::Channel;
use embassy_time::Timer;
use futures::FutureExt;

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
    // SIMULATION CODE
    // spawner
    //     .spawn(hx710_load_cell_manager_rotary_encoder(
    //         peripherals.PIN_26,
    //         peripherals.PIN_27,
    //         peripherals.PIO0,
    //         CHANNEL.sender(),
    //     ))
    //     .unwrap();
    spawner
        .spawn(hx710_load_cell_manager(
            peripherals.PIN_10,
            peripherals.PIN_11,
            peripherals.PIO1,
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
    let mut poll_first_1 = PollFirst2::A;
    let mut poll_first_2 = 0;
    loop {
        // Interleave state transitions
        let state_transitions_futures = state.get_transitions().map(|x| match x {
            Some((t, f)) => futures::future::Either::Right(Timer::at(t).map(move |_| f)),
            None => futures::future::Either::Left(core::future::pending()),
        });
        // let result = round_robin_select::round_robin_select(
        //     &mut poll_first_1,
        //     rx.receive(),
        //     round_robin_select_array(&mut poll_first_2, state_transitions_futures),
        // )
        // .await;
        let result = round_robin_select::round_robin_select(
            &mut poll_first_1,
            rx.receive(),
            embassy_futures::select::select_array(state_transitions_futures),
        )
        .await;
        match result {
            Either::First(message) => state.handle_message(message),
            Either::Second((transition, _)) => transition(&mut state),
        }
        output_state(
            &mut state,
            &mut display_controller,
            &mut display_led_controller,
        );
    }
}
