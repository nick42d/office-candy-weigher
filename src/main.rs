#![no_std]
#![no_main]

use crate::pimoroni_display::PimoroniDisplayController;
use crate::pimoroni_display_leds::PimoroniDisplayRgbLedController;
use crate::round_robin_select::PollFirst2;
use crate::state::{output_state, State};
use crate::tasks::{hx710_load_cell_manager, pico_display_button_a_manager};
use crate::tasks::{
    pico_display_button_b_manager, pico_display_button_x_manager, pico_display_button_y_manager,
};
use core::cell::RefCell;
use defmt::*;
use embassy_executor::Spawner;
use embassy_futures::select::Either;
use embassy_rp::spi::{Config, Spi};
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::blocking_mutex::Mutex;
use embassy_sync::channel::Channel;
use embassy_time::Timer;
use futures::FutureExt;

use {defmt_rtt as _, panic_probe as _};

const CHANNEL_SIZE: usize = 16;

static CHANNEL: Channel<ThreadModeRawMutex, Message, CHANNEL_SIZE> = Channel::new();

mod candy_weigher_ui;
mod config_consts;
mod pimoroni_display;
mod pimoroni_display_leds;
mod round_robin_select;
mod state;
mod tasks;

#[derive(Copy, Clone, Debug, defmt::Format)]
enum Message {
    ButtonAPressed,
    ButtonBPressed,
    ButtonAHeld,
    ButtonBHeld,
    ButtonAHoldCancelled,
    ButtonBHoldCancelled,
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
        .spawn(hx710_load_cell_manager(
            peripherals.PIN_10,
            peripherals.PIN_11,
            peripherals.PIO1,
            CHANNEL.sender(),
        ))
        .unwrap();
    #[cfg(feature = "hardware-sim")]
    spawner
        .spawn(tasks::hx710_load_cell_manager_rotary_encoder(
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

    let rx = CHANNEL.receiver();
    info!("Initial UI drawn, entering event loop");
    let mut poll_first_1 = PollFirst2::A;
    loop {
        // Interleave state transitions
        let state_transitions_future = match state.get_next_transitions() {
            Some((t, f)) => futures::future::Either::Right(Timer::at(t).map(move |_| f)),
            None => futures::future::Either::Left(core::future::pending()),
        };
        let result = round_robin_select::round_robin_select(
            &mut poll_first_1,
            rx.receive(),
            state_transitions_future,
        )
        .await;
        match result {
            Either::First(message) => state.handle_message(message),
            Either::Second(transitions) => {
                debug!("State transitioning");
                for transition in transitions {
                    transition(&mut state)
                }
            }
        }
        output_state(
            &mut state,
            &mut display_controller,
            &mut display_led_controller,
        );
    }
}
