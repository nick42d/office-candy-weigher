#![no_std]
#![no_main]

use crate::candy_weigher_ui::DisplayState;
use crate::config_consts::assign_peripherals;
use crate::hardware_controllers::{
    FlashController, LoadCellController, PimoroniDisplayRgbLedController, flash::Config,
};
use crate::round_robin_select::{PollFirst2, unbiased_select_slice};
use crate::state::effect::{Event, TimerEvent};
use crate::state::{DisplayBacklightState, State, output_state};
use crate::tasks::{display_manager, hx710_load_cell_manager, pico_display_button_a_manager};
use crate::tasks::{
    pico_display_button_b_manager, pico_display_button_x_manager, pico_display_button_y_manager,
};
use crate::utils::{TimerFuture, timer_future_at, timer_future_in};
use defmt::*;
use effect_lite::Effect;
use embassy_executor::{Executor, Spawner};
use embassy_futures::select::Either;
use embassy_rp::bind_interrupts;
use embassy_rp::clocks::RoscRng;
use embassy_rp::dma;
use embassy_rp::multicore::{Stack, spawn_core1};
use embassy_rp::peripherals::PIO1;
use embassy_rp::peripherals::{DMA_CH0, DMA_CH1};
use embassy_rp::pio;
use embassy_sync::blocking_mutex::raw::{CriticalSectionRawMutex, ThreadModeRawMutex};
use embassy_sync::channel::Channel;
use embassy_sync::signal::Signal;
use embassy_time::{Duration, Instant};
use static_cell::StaticCell;

use {defmt_rtt as _, panic_probe as _};

const MESSAGE_CHANNEL_SIZE: usize = 16;
// STORAGE block in memory.x starts at 2040KB.
pub const FLASH_STORAGE_OFFSET_BYTES: u32 = 2040 * 1024;
// STORAGE in memory.x is 8KB.
const FLASH_STORAGE_SIZE_BYTES: u32 = 8 * 1024;

static MESSAGE_CHANNEL: Channel<ThreadModeRawMutex, Event, MESSAGE_CHANNEL_SIZE> = Channel::new();
// Give core1 (second core) it's own stack.
static CORE1_STACK: StaticCell<Stack<4096>> = StaticCell::new();
static CORE1_EXECUTOR: StaticCell<Executor> = StaticCell::new();
static CORE1_SIGNAL: Signal<CriticalSectionRawMutex, (DisplayState, DisplayBacklightState)> =
    Signal::new();

#[cfg(feature = "hardware-sim")]
bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => pio::InterruptHandler<embassy_rp::peripherals::PIO0>;
    PIO1_IRQ_0 => pio::InterruptHandler<PIO1>;
    DMA_IRQ_0 => dma::InterruptHandler<DMA_CH0>, dma::InterruptHandler<DMA_CH1>;
});

#[cfg(not(feature = "hardware-sim"))]
bind_interrupts!(struct Irqs {
    PIO1_IRQ_0 => pio::InterruptHandler<PIO1>;
    DMA_IRQ_0 => dma::InterruptHandler<DMA_CH0>, dma::InterruptHandler<DMA_CH1>;
});

mod candy_weigher_ui;
mod config_consts;
mod hardware_controllers;
mod round_robin_select;
mod state;
mod tasks;
mod utils;

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let peripherals = assign_peripherals(embassy_rp::init(Default::default()));
    info!("Peripherals initialised");

    let mut display_led_controller = PimoroniDisplayRgbLedController::new(
        peripherals.display_led_controller_rg_pwm_slice,
        peripherals.display_led_controller_b_pwm_slice,
        peripherals.display_led_controller_r_pin,
        peripherals.display_led_controller_g_pin,
        peripherals.display_led_controller_b_pin,
    );
    info!("LED controller initialised");

    let mut flash_controller: FlashController<'_> = FlashController::new(
        peripherals.flash,
        peripherals.flash_dma,
        FLASH_STORAGE_OFFSET_BYTES,
    );
    info!("Flash controller initialised");

    let load_cell_controller = LoadCellController;
    info!("Load cell controller initialised");

    let cfg: Config = flash_controller
        .read::<_, 256>()
        .await
        .inspect_err(|e| warn!("Failed to read config from flash. Was one stored? e: {}", e))
        .unwrap_or_default();
    info!("Loaded config: {}", cfg);

    spawn_core1(
        peripherals.core_1,
        CORE1_STACK.init(Stack::new()),
        move || {
            let core1_executor = CORE1_EXECUTOR.init(Executor::new());
            core1_executor.run(|spawner| {
                spawner.spawn(
                    display_manager(
                        peripherals.display_manager_dcx_pin,
                        peripherals.display_manager_spi_cs_pin,
                        peripherals.display_manager_spi_clk_pin,
                        peripherals.display_manager_spi_mosi_pin,
                        peripherals.display_manager_backlight_pin,
                        peripherals.display_manager_pwm_slice,
                        peripherals.display_manager_spi,
                        peripherals.display_manager_dma,
                    )
                    .unwrap(),
                );
                info!("Core1 tasks spawned");
            });
        },
    );
    spawner.spawn(
        pico_display_button_a_manager(peripherals.button_a_pin, MESSAGE_CHANNEL.sender()).unwrap(),
    );
    spawner.spawn(
        pico_display_button_b_manager(peripherals.button_b_pin, MESSAGE_CHANNEL.sender()).unwrap(),
    );
    spawner.spawn(
        pico_display_button_x_manager(peripherals.button_x_pin, MESSAGE_CHANNEL.sender()).unwrap(),
    );
    spawner.spawn(
        pico_display_button_y_manager(peripherals.button_y_pin, MESSAGE_CHANNEL.sender()).unwrap(),
    );
    spawner.spawn(
        hx710_load_cell_manager(
            peripherals.hx710_sclk_pin,
            peripherals.hx710_dout_pin,
            peripherals.hx710_pio,
            MESSAGE_CHANNEL.sender(),
            load_cell_controller.get_signal(),
        )
        .unwrap(),
    );
    #[cfg(feature = "hardware-sim")]
    spawner.spawn(
        tasks::hx710_load_cell_manager_rotary_encoder(
            peripherals.rotary_encoder_sclk_pin,
            peripherals.rotary_encoder_dout_pin,
            peripherals.rotary_encoder_pio,
            MESSAGE_CHANNEL.sender(),
        )
        .unwrap(),
    );
    #[cfg(feature = "software-sim")]
    spawner.spawn(tasks::hx710_load_cell_manager_simulated(MESSAGE_CHANNEL.sender()).unwrap());

    info!("Core0 tasks spawned");

    let mut state = State {
        lolly_weight_g: (cfg.lolly_weight_dg as f32) / 10.0,
        tare_weight_g: (cfg.tare_weight_dg as f32) / 10.0,
        saved_tared_scale_weight_g: (cfg.saved_tared_scale_weight as f32) / 10.0,
        scale_raw_tare: cfg.scale_raw_tare,
        scale_raw_50g: cfg.scale_raw_50g,
        ..Default::default()
    };

    output_state(&mut state, &mut display_led_controller);

    let rx = MESSAGE_CHANNEL.receiver();
    info!("Initial UI drawn, entering event loop");
    let mut poll_first_1 = PollFirst2::A;
    const MAX_EFFECTS: usize = 100;
    let mut futures_executor = heapless::Vec::<TimerFuture<Event>, MAX_EFFECTS>::new();
    let mut rng = RoscRng;

    loop {
        let result = round_robin_select::round_robin_select(
            &mut poll_first_1,
            rx.receive(),
            unbiased_select_slice(&mut rng, core::pin::pin!(&mut futures_executor)),
        )
        .await;
        let event = match result {
            Either::First(event_from_hardware) => {
                debug!("received event from hardware {:?}", event_from_hardware);
                event_from_hardware
            }
            Either::Second((event_from_executor, idx)) => {
                debug!("received event from executor {:?}", event_from_executor);
                futures_executor.remove(idx);
                event_from_executor
            }
        };
        let (e1, e2, e3, e4) = event.resolve(&mut state);
        if let Some(e1) = e1 {
            e1.resolve(&mut flash_controller)
        }
        if let Some(e2) = e2 {
            e2.resolve(&load_cell_controller)
        }
        if let Some(e3) = e3 {
            // Remove all other LED timers
            futures_executor.retain(|fut| {
                fut.inspect_t()
                    .is_some_and(|t| !matches!(t, Event::Timer(TimerEvent::FadeoutLEDs { .. })))
            });
            let Ok(()) = futures_executor.push(e3.resolve(())) else {
                crate::panic!("Ran out of space in futures executor");
            };
        }
        if let Some(e4) = e4 {
            // Remove all other Display timers
            futures_executor.retain(|fut| {
                fut.inspect_t().is_some_and(|t| {
                    !matches!(t, Event::Timer(TimerEvent::DimOrSleepDisplay { .. }))
                })
            });
            let Ok(()) = futures_executor.push(e4.resolve(())) else {
                crate::panic!("Ran out of space in futures executor");
            };
        }
        output_state(&mut state, &mut display_led_controller);
    }
}

#[must_use]
#[derive(Debug, defmt::Format)]
pub struct WriteConfig(Config);
#[must_use]
#[derive(Debug, defmt::Format)]
pub struct EnterOrProgressCalibrationMode;
#[must_use]
#[derive(Debug, defmt::Format)]
pub struct StartDimOrSleepDisplayTimer {
    start_time: Instant,
    in_dur: Duration,
}
#[must_use]
#[derive(Debug, defmt::Format)]
pub struct StartLEDTimer {
    start_time: Instant,
    next_at: Instant,
}

impl<'a> Effect<&mut FlashController<'a>> for WriteConfig {
    type Output = ();
    fn resolve(self, flash_controller: &mut FlashController<'a>) -> Self::Output {
        flash_controller.write::<_, 4096>(&self.0);
    }
}
impl Effect<&LoadCellController> for EnterOrProgressCalibrationMode {
    type Output = ();
    fn resolve(self, hx710_controller: &LoadCellController) -> Self::Output {
        hx710_controller.enter_or_progress_calibration_mode()
    }
}
impl Effect<()> for StartDimOrSleepDisplayTimer {
    type Output = TimerFuture<Event>;
    fn resolve(self, _: ()) -> Self::Output {
        let StartDimOrSleepDisplayTimer { start_time, in_dur } = self;
        timer_future_in(
            Event::Timer(TimerEvent::DimOrSleepDisplay { start_time }),
            in_dur,
        )
    }
}
impl Effect<()> for StartLEDTimer {
    type Output = TimerFuture<Event>;
    fn resolve(self, _: ()) -> Self::Output {
        let StartLEDTimer {
            start_time,
            next_at,
        } = self;
        timer_future_at(
            Event::Timer(TimerEvent::FadeoutLEDs { start_time }),
            next_at,
        )
    }
}
