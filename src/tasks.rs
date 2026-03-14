use crate::{CHANNEL_SIZE, Message};
use embassy_rp::Peri;
use embassy_rp::gpio::{Input, Pull};
use embassy_rp::peripherals::{PIN_12, PIN_13, PIN_14, PIN_15};
use embassy_sync::blocking_mutex::raw::{RawMutex, ThreadModeRawMutex};
use embassy_sync::channel::Sender;

#[embassy_executor::task]
pub async fn pico_display_button_a_manager(
    pin12: Peri<'static, PIN_12>,
    tx: Sender<'static, ThreadModeRawMutex, Message, CHANNEL_SIZE>,
) {
    let button_a = Input::new(pin12, Pull::Up);
    manage_button(button_a, Message::ButtonAPressed, tx).await;
}
#[embassy_executor::task]
pub async fn pico_display_button_b_manager(
    pin13: Peri<'static, PIN_13>,
    tx: Sender<'static, ThreadModeRawMutex, Message, CHANNEL_SIZE>,
) {
    let button_b = Input::new(pin13, Pull::Up);
    manage_button(button_b, Message::ButtonBPressed, tx).await;
}
#[embassy_executor::task]
pub async fn pico_display_button_x_manager(
    pin14: Peri<'static, PIN_14>,
    tx: Sender<'static, ThreadModeRawMutex, Message, CHANNEL_SIZE>,
) {
    let button_x = Input::new(pin14, Pull::Up);
    manage_button(button_x, Message::ButtonXPressed, tx).await;
}
#[embassy_executor::task]
pub async fn pico_display_button_y_manager(
    pin15: Peri<'static, PIN_15>,
    tx: Sender<'static, ThreadModeRawMutex, Message, CHANNEL_SIZE>,
) {
    let button_y = Input::new(pin15, Pull::Up);
    manage_button(button_y, Message::ButtonYPressed, tx).await;
}

async fn manage_button<'a, M, Mutex, const BUTTON_CHANNEL_SIZE: usize>(
    mut button: Input<'static>,
    pressed_message: M,
    tx: Sender<'a, Mutex, M, BUTTON_CHANNEL_SIZE>,
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
