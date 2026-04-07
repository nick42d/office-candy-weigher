use core::cell::RefCell;

use crate::config_consts::{
    scale_raw_1g_step, BUTTON_LONG_PRESS_THRESHOLD, BUTTON_REPEAT_THRESHOLD,
    LOW_BACKLIGHT_PERCENTAGE,
};
use crate::hardware_controllers::pimoroni_display_leds::Percentage;
use crate::hardware_controllers::PimoroniDisplayController;
use crate::{candy_weigher_ui, Irqs, StateEffect, CORE1_SIGNAL, MESSAGE_CHANNEL_SIZE};
use defmt::info;
use embassy_futures::select::{select, Either};
use embassy_rp::gpio::{Input, Pull};
use embassy_rp::peripherals::{
    DMA_CH0, PIN_10, PIN_11, PIN_12, PIN_13, PIN_14, PIN_15, PIN_16, PIN_17, PIN_18, PIN_19,
    PIN_20, PIO1, PWM_SLICE2, SPI0,
};
use embassy_rp::pio::Pio;
use embassy_rp::spi::{self, Spi};
use embassy_rp::Peri;
use embassy_sync::blocking_mutex::raw::{RawMutex, ThreadModeRawMutex};
use embassy_sync::blocking_mutex::Mutex;
use embassy_sync::channel::Sender;
use embassy_sync::signal::Signal;
use embassy_time::{Duration, Timer};
use hx71x_pio::{PioHX710, PioHX710Program};

#[embassy_executor::task]
pub async fn display_manager(
    pin16: Peri<'static, PIN_16>,
    pin17: Peri<'static, PIN_17>,
    pin18: Peri<'static, PIN_18>,
    pin19: Peri<'static, PIN_19>,
    pin20: Peri<'static, PIN_20>,
    slice2: Peri<'static, PWM_SLICE2>,
    spi0: Peri<'static, SPI0>,
    dma0: Peri<'static, DMA_CH0>,
) {
    // TODO: Consider if interrupt handler needs to be set up for DMA_CH0
    let spi = Spi::new_txonly(spi0, pin18, pin19, dma0, Irqs, spi::Config::default());
    let spi_bus = Mutex::new(RefCell::new(spi));
    let mut display_buffer = [0u8; 512];
    let mut display =
        PimoroniDisplayController::new(pin16, pin17, pin20, slice2, &spi_bus, &mut display_buffer);
    info!("Display controller initialised");

    loop {
        let (next_frame, next_backlight) = CORE1_SIGNAL.wait().await;
        display.draw_via_framebuffer(|display| candy_weigher_ui::draw(&next_frame, display));
        match next_backlight {
            crate::state::DisplayBacklightState::Off => display.turn_off_display(),
            crate::state::DisplayBacklightState::LowPower { .. } => {
                display.turn_on_display(LOW_BACKLIGHT_PERCENTAGE)
            }
            crate::state::DisplayBacklightState::On { .. } => {
                display.turn_on_display(Percentage(100))
            }
        }
    }
}

#[embassy_executor::task]
pub async fn pico_display_button_a_manager(
    pin12: Peri<'static, PIN_12>,
    tx: Sender<'static, ThreadModeRawMutex, StateEffect, MESSAGE_CHANNEL_SIZE>,
) {
    let button_a = Input::new(pin12, Pull::Up);
    manage_repeating_button(
        button_a,
        StateEffect::ButtonAPressed,
        StateEffect::ButtonARepeated,
        StateEffect::ButtonAReleased,
        BUTTON_LONG_PRESS_THRESHOLD,
        BUTTON_REPEAT_THRESHOLD,
        tx,
    )
    .await;
}
#[embassy_executor::task]
pub async fn pico_display_button_b_manager(
    pin13: Peri<'static, PIN_13>,
    tx: Sender<'static, ThreadModeRawMutex, StateEffect, MESSAGE_CHANNEL_SIZE>,
) {
    let button_b = Input::new(pin13, Pull::Up);
    manage_repeating_button(
        button_b,
        StateEffect::ButtonBPressed,
        StateEffect::ButtonBRepeated,
        StateEffect::ButtonBReleased,
        BUTTON_LONG_PRESS_THRESHOLD,
        BUTTON_REPEAT_THRESHOLD,
        tx,
    )
    .await;
}
#[embassy_executor::task]
pub async fn pico_display_button_x_manager(
    pin14: Peri<'static, PIN_14>,
    tx: Sender<'static, ThreadModeRawMutex, StateEffect, MESSAGE_CHANNEL_SIZE>,
) {
    let button_x = Input::new(pin14, Pull::Up);
    manage_holdable_button(
        button_x,
        StateEffect::ButtonXPressed,
        StateEffect::ButtonXHeld,
        StateEffect::ButtonXReleased,
        BUTTON_LONG_PRESS_THRESHOLD,
        tx,
    )
    .await;
}
#[embassy_executor::task]
pub async fn pico_display_button_y_manager(
    pin15: Peri<'static, PIN_15>,
    tx: Sender<'static, ThreadModeRawMutex, StateEffect, MESSAGE_CHANNEL_SIZE>,
) {
    let button_y = Input::new(pin15, Pull::Up);
    manage_holdable_button(
        button_y,
        StateEffect::ButtonYPressed,
        StateEffect::ButtonYHeld,
        StateEffect::ButtonYReleased,
        BUTTON_LONG_PRESS_THRESHOLD,
        tx,
    )
    .await;
}

async fn manage_repeating_button<'a, M, Mutex, const BUTTON_CHANNEL_SIZE: usize>(
    mut button: Input<'static>,
    pressed_message: M,
    repeat_message: M,
    released_message: M,
    first_repeat_threshold: Duration,
    subsequent_repeat_threshold: Duration,
    tx: Sender<'a, Mutex, M, BUTTON_CHANNEL_SIZE>,
) where
    M: Copy,
    Mutex: RawMutex,
{
    loop {
        button.wait_for_low().await;
        tx.send(pressed_message).await;
        // Wait for long press
        if let Either::First(_) = select(
            button.wait_for_high(),
            //500ms
            embassy_time::Timer::after(first_repeat_threshold),
        )
        .await
        {
            tx.send(released_message).await;
            continue;
        }
        tx.send(repeat_message).await;
        while let Either::Second(_) = select(
            button.wait_for_high(),
            //100ms
            embassy_time::Timer::after(subsequent_repeat_threshold),
        )
        .await
        {
            tx.send(repeat_message).await;
        }
        tx.send(released_message).await;
    }
}

async fn manage_holdable_button<'a, M, Mutex, const BUTTON_CHANNEL_SIZE: usize>(
    mut button: Input<'static>,
    pressed_message: M,
    held_message: M,
    released_message: M,
    held_threshold: Duration,
    tx: Sender<'a, Mutex, M, BUTTON_CHANNEL_SIZE>,
) where
    M: Copy,
    Mutex: RawMutex,
{
    loop {
        button.wait_for_low().await;
        tx.send(pressed_message).await;
        // Wait for long press
        if let Either::Second(_) = select(
            button.wait_for_high(),
            embassy_time::Timer::after(held_threshold),
        )
        .await
        {
            tx.send(held_message).await;
        }
        button.wait_for_high().await;
        tx.send(released_message).await;
    }
}

#[cfg(feature = "software-sim")]
#[embassy_executor::task]
pub async fn hx710_load_cell_manager_simulated(
    tx: Sender<'static, ThreadModeRawMutex, StateEffect, MESSAGE_CHANNEL_SIZE>,
) {
    const TEST_WEIGHT_DATA: &[(f32, Duration)] = &[
        (00000.0, Duration::from_secs(5)),
        (10000.0, Duration::from_millis(300)),
        (50000.0, Duration::from_millis(300)),
        (100000.0, Duration::from_millis(300)),
        (500000.0, Duration::from_millis(300)),
        (1500000.0, Duration::from_millis(300)),
        (3000000.0, Duration::from_secs(10)),
        (2950000.0, Duration::from_millis(300)),
        (2850000.0, Duration::from_millis(300)),
        (2750000.0, Duration::from_secs(5)),
        (2700000.0, Duration::from_millis(300)),
        (2600000.0, Duration::from_millis(300)),
        (2500000.0, Duration::from_secs(5)),
    ];
    for (weight, duration) in TEST_WEIGHT_DATA.iter().cycle() {
        tx.send(StateEffect::WeightUpdate(ScaleRawWeight(*weight)))
            .await;
        embassy_time::Timer::after(*duration).await;
    }
}

#[cfg(feature = "hardware-sim")]
#[embassy_executor::task]
pub async fn hx710_load_cell_manager_rotary_encoder(
    pin26: Peri<'static, embassy_rp::peripherals::PIN_26>,
    pin27: Peri<'static, embassy_rp::peripherals::PIN_27>,
    pio0: Peri<'static, embassy_rp::peripherals::PIO0>,
    tx: Sender<'static, ThreadModeRawMutex, StateEffect, MESSAGE_CHANNEL_SIZE>,
) {
    use crate::Irqs;

    let Pio {
        mut common, sm0, ..
    } = Pio::new(pio0, Irqs);
    let program = embassy_rp::pio_programs::rotary_encoder::PioEncoderProgram::new(&mut common);
    let mut encoder = embassy_rp::pio_programs::rotary_encoder::PioEncoder::new(
        &mut common,
        sm0,
        pin26,
        pin27,
        &program,
    );

    let mut base_weight = 0.0;
    loop {
        use embassy_rp::pio_programs::rotary_encoder::Direction;

        let direction = encoder.read().await;
        match direction {
            Direction::Clockwise => base_weight += 25000.0,
            Direction::CounterClockwise => base_weight -= 25000.0,
        }
        tx.send(StateEffect::WeightUpdate(ScaleRawWeight(base_weight)))
            .await;
    }
}

#[derive(defmt::Format, Debug, Copy, Clone)]
pub struct ScaleRawWeight(f32);
impl ScaleRawWeight {
    pub const fn to_grams(self, scale_raw_tare: f32, scale_raw_50g: f32) -> f32 {
        (self.0 - scale_raw_tare) / scale_raw_1g_step(scale_raw_tare, scale_raw_50g)
    }
    pub const fn get_raw(self) -> f32 {
        self.0
    }
}

#[embassy_executor::task]
pub async fn hx710_load_cell_manager(
    pin10: Peri<'static, PIN_10>,
    pin11: Peri<'static, PIN_11>,
    pio1: Peri<'static, PIO1>,
    tx: Sender<'static, ThreadModeRawMutex, StateEffect, MESSAGE_CHANNEL_SIZE>,
    calibration_mode_signal: &'static Signal<ThreadModeRawMutex, ()>,
) {
    let Pio {
        mut common, sm0, ..
    } = Pio::new(pio1, Irqs);
    let program = PioHX710Program::new(&mut common);
    let mut load_cell = PioHX710::new(&mut common, sm0, pin11, pin10, &program);

    // Exponential moving average - to smooth readings.
    const EMA_FILTER_ALPHA: f32 = 0.2;
    // Minimum change of value to be sent.
    const MIN_RAW_CHANGE_TOLERANCE: f32 = 10.0;
    let mut ema_weight_raw: Option<f32> = None;

    loop {
        if calibration_mode_signal.signaled() {
            // Proposed workflow:
            // Display - remove all weight from scale then press X/Y (or any key).
            // Display - calibrating tare value
            // Take 50 polls and disccard, then average of 200.
            // Display - Tare weight calibrated. Add 50g weight, then press X/Y (or any
            // key). Display - calibrating 50g value
            // Send new calibration values
            // Display - calibration complete - press X/Y (or any key) to continue.
            calibration_mode_signal.reset();
            let mut ma_100 = [0f32; 100];
            for i in 0..=500 {
                let raw_val = load_cell.read().await;
                ma_100[i % 100] = raw_val as f32;
                if i >= 99 {
                    tx.send(StateEffect::CalibWeightUpdate(ScaleRawWeight(
                        ma_100.iter().sum::<f32>() / 100.0,
                    )))
                    .await
                }
                // Small delay to prevent flooding logs, though PIO will
                // naturally throttle based on the HX710 sample rate (10-40Hz).
                //
                // Delay is smaller here as chewing battery not a concern in calibration mode.
                Timer::after(Duration::from_millis(10)).await;
            }
            tx.send(StateEffect::CalibModeComplete).await;
        }
        let raw_val = load_cell.read().await;

        if let Some(ref mut ema_weight_raw) = ema_weight_raw {
            let next_ema_weight_raw =
                (raw_val as f32 * EMA_FILTER_ALPHA) + (*ema_weight_raw * (1.0 - EMA_FILTER_ALPHA));
            if (next_ema_weight_raw - *ema_weight_raw).abs() > MIN_RAW_CHANGE_TOLERANCE {
                tx.send(StateEffect::WeightUpdate(ScaleRawWeight(
                    next_ema_weight_raw,
                )))
                .await;
            }
            *ema_weight_raw = next_ema_weight_raw;
        } else {
            tx.send(StateEffect::WeightUpdate(ScaleRawWeight(raw_val as f32)))
                .await;
            ema_weight_raw = Some(raw_val as f32)
        }

        // Small delay to prevent flooding logs and chewing battery, though PIO will
        // naturally throttle based on the HX710 sample rate (10-40Hz).
        Timer::after(Duration::from_millis(100)).await;
    }
}
