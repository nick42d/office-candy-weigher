#![no_std]
#![no_main]

use crate::pimori_display::PimoriDisplayController;
use crate::pimori_display_leds::PimoriDisplayRgbLedController;
use crate::tasks::{
    pico_display_button_b_manager, pico_display_button_x_manager, pico_display_button_y_manager,
};
use crate::{candy_weigher_ui::DisplayState, tasks::pico_display_button_a_manager};
use core::cell::RefCell;
use defmt::*;
use embassy_executor::Spawner;
use embassy_rp::spi::{Config, Spi};
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::blocking_mutex::Mutex;
use embassy_sync::channel::Channel;

use {defmt_rtt as _, panic_probe as _};

const CHANNEL_SIZE: usize = 16;
static CHANNEL: Channel<ThreadModeRawMutex, Message, CHANNEL_SIZE> = Channel::new();

mod candy_weigher_ui;
mod pimori_display;
mod pimori_display_leds;
mod tasks;

#[derive(Copy, Clone)]
enum Message {
    ButtonAPressed,
    ButtonBPressed,
    ButtonXPressed,
    ButtonYPressed,
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

    let mut display_controller = PimoriDisplayController::new(
        peripherals.PIN_16,
        peripherals.PIN_17,
        peripherals.PIN_20,
        &spi_bus,
        &mut display_buffer,
    );

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
    display_controller.draw(|display| candy_weigher_ui::draw(&state, display));
    let rx = CHANNEL.receiver();
    loop {
        let result = rx.receive().await;
        match result {
            Message::ButtonAPressed => {
                state.lolly_weight_g += 0.1;
                state.t_l_pressed = true;
                display_controller.draw(|display| candy_weigher_ui::draw(&state, display));
            }
            Message::ButtonBPressed => {
                state.lolly_weight_g -= 0.1;
                state.b_l_pressed = !state.b_l_pressed;
                display_controller.draw(|display| candy_weigher_ui::draw(&state, display));
            }
            Message::ButtonXPressed => {
                state.lolly_count_change = 0;
                state.t_r_pressed = !state.t_r_pressed;
                display_controller.draw(|display| candy_weigher_ui::draw(&state, display));
            }
            Message::ButtonYPressed => {
                state.scale_weight_g = 0.0;
                state.b_r_pressed = !state.b_r_pressed;
                display_controller.draw(|display| candy_weigher_ui::draw(&state, display));
            }
        }
    }
}
