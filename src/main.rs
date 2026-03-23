#![no_std]
#![no_main]

use crate::candy_weigher_ui::DisplayState;
use crate::flash::{Config, FlashController};
use crate::pimoroni_display::PimoroniDisplayBacklightController;
use crate::pimoroni_display_leds::PimoroniDisplayRgbLedController;
use crate::round_robin_select::PollFirst2;
use crate::state::{output_state, State};
use crate::tasks::{display_manager, hx710_load_cell_manager, pico_display_button_a_manager};
use crate::tasks::{
    pico_display_button_b_manager, pico_display_button_x_manager, pico_display_button_y_manager,
};
use defmt::*;
use embassy_executor::{Executor, Spawner};
use embassy_futures::select::Either;
use embassy_rp::multicore::{spawn_core1, Stack};
use embassy_sync::blocking_mutex::raw::{CriticalSectionRawMutex, ThreadModeRawMutex};
use embassy_sync::channel::Channel;
use embassy_sync::signal::Signal;
use embassy_time::Timer;
use futures::FutureExt;
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

const MESSAGE_CHANNEL_SIZE: usize = 16;
// STORAGE block in memory.x starts at 2040KB.
pub const FLASH_STORAGE_OFFSET_BYTES: u32 = 2040 * 1024;
// STORAGE in memory.x is 8KB.
const FLASH_STORAGE_SIZE_BYTES: u32 = 8 * 1024;

static MESSAGE_CHANNEL: Channel<ThreadModeRawMutex, Message, MESSAGE_CHANNEL_SIZE> = Channel::new();
// Give core1 (second core) it's own stack.
static CORE1_STACK: StaticCell<Stack<4096>> = StaticCell::new();
static CORE1_EXECUTOR: StaticCell<Executor> = StaticCell::new();
static CORE1_SIGNAL: Signal<CriticalSectionRawMutex, DisplayState> = Signal::new();

mod candy_weigher_ui;
mod config_consts;
mod flash;
mod hx710;
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
    ButtonXHeld,
    ButtonYHeld,
    ButtonXReleased,
    ButtonYReleased,
    WeightUpdate(f32),
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let peripherals = embassy_rp::init(Default::default());
    info!("Peripherals initialised");

    let mut display_led_controller = PimoroniDisplayRgbLedController::new(
        peripherals.PWM_SLICE3,
        peripherals.PWM_SLICE4,
        peripherals.PIN_6,
        peripherals.PIN_7,
        peripherals.PIN_8,
    );
    info!("LED controller initialised");

    let mut display_backlight_controller =
        PimoroniDisplayBacklightController::new(peripherals.PIN_20, peripherals.PWM_SLICE2);
    info!("Display backlight controller initialised");

    let mut flash_controller: FlashController<'_, FLASH_STORAGE_OFFSET_BYTES> =
        FlashController::new(peripherals.FLASH, peripherals.DMA_CH1);
    info!("Flash controller initialised");

    let cfg: Config = flash_controller
        .read::<_, 256>()
        .await
        .inspect_err(|e| warn!("Failed to read config from flash. Was one stored? e: {}", e))
        .unwrap_or_default();
    info!("Loaded config: {}", cfg);
    flash_controller.write::<_, 4096>(&Config::default());

    spawn_core1(
        peripherals.CORE1,
        CORE1_STACK.init(Stack::new()),
        move || {
            let core1_executor = CORE1_EXECUTOR.init(Executor::new());
            core1_executor.run(|spawner| {
                spawner
                    .spawn(display_manager(
                        peripherals.PIN_16,
                        peripherals.PIN_17,
                        peripherals.PIN_18,
                        peripherals.PIN_19,
                        peripherals.SPI0,
                        peripherals.DMA_CH0,
                    ))
                    .unwrap();
                info!("Core1 tasks spawned");
            });
        },
    );
    spawner
        .spawn(pico_display_button_a_manager(
            peripherals.PIN_12,
            MESSAGE_CHANNEL.sender(),
        ))
        .unwrap();
    spawner
        .spawn(pico_display_button_b_manager(
            peripherals.PIN_13,
            MESSAGE_CHANNEL.sender(),
        ))
        .unwrap();
    spawner
        .spawn(pico_display_button_x_manager(
            peripherals.PIN_14,
            MESSAGE_CHANNEL.sender(),
        ))
        .unwrap();
    spawner
        .spawn(pico_display_button_y_manager(
            peripherals.PIN_15,
            MESSAGE_CHANNEL.sender(),
        ))
        .unwrap();
    spawner
        .spawn(hx710_load_cell_manager(
            peripherals.PIN_10,
            peripherals.PIN_11,
            peripherals.PIO1,
            MESSAGE_CHANNEL.sender(),
        ))
        .unwrap();
    #[cfg(feature = "hardware-sim")]
    spawner
        .spawn(tasks::hx710_load_cell_manager_rotary_encoder(
            peripherals.PIN_26,
            peripherals.PIN_27,
            peripherals.PIO0,
            MESSAGE_CHANNEL.sender(),
        ))
        .unwrap();
    #[cfg(feature = "software-sim")]
    spawner
        .spawn(tasks::hx710_load_cell_manager_simulated(
            MESSAGE_CHANNEL.sender(),
        ))
        .unwrap();
    info!("Core0 tasks spawned");

    let mut state = State::default();
    state.lolly_weight_g = cfg.lolly_weight_g;
    state.tare_weight_g = cfg.tare_weight_g;

    output_state(
        &mut state,
        &mut display_backlight_controller,
        &mut display_led_controller,
    );

    let rx = MESSAGE_CHANNEL.receiver();
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
            Either::First(message) => state.handle_message(&mut flash_controller, message),
            Either::Second(transitions) => {
                debug!("State transitioning");
                for transition in transitions {
                    transition(&mut state)
                }
            }
        }
        output_state(
            &mut state,
            &mut display_backlight_controller,
            &mut display_led_controller,
        );
    }
}
