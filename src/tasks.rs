use core::cell::RefCell;

use crate::config_consts::{SCALE_RAW_1G_STEP, SCALE_RAW_TARE};
use crate::hx710::{PioHX710, PioHX710Program};
use crate::pimoroni_display::PimoroniDisplayController;
use crate::{candy_weigher_ui, Message, CHANNEL_SIZE, CORE1_SIGNAL};
use defmt::info;
use embassy_futures::select::{select, Either};
use embassy_rp::gpio::{Input, Pull};
use embassy_rp::peripherals::{
    DMA_CH0, PIN_10, PIN_11, PIN_12, PIN_13, PIN_14, PIN_15, PIN_16, PIN_17, PIN_18, PIN_19, PIO1,
    SPI0,
};
use embassy_rp::pio::{self, InterruptHandler, Pio, ShiftDirection};
use embassy_rp::spi::{self, Spi};
use embassy_rp::{bind_interrupts, Peri};
use embassy_sync::blocking_mutex::raw::{RawMutex, ThreadModeRawMutex};
use embassy_sync::blocking_mutex::Mutex;
use embassy_sync::channel::Sender;
use embassy_time::{Duration, Timer};

#[cfg(feature = "hardware-sim")]
bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => InterruptHandler<embassy_rp::peripherals::PIO0>;
    PIO1_IRQ_0 => InterruptHandler<PIO1>;
});

#[cfg(not(feature = "hardware-sim"))]
bind_interrupts!(struct Irqs {
    PIO1_IRQ_0 => InterruptHandler<PIO1>;
});

#[embassy_executor::task]
pub async fn display_manager(
    pin16: Peri<'static, PIN_16>,
    pin17: Peri<'static, PIN_17>,
    pin18: Peri<'static, PIN_18>,
    pin19: Peri<'static, PIN_19>,
    spi0: Peri<'static, SPI0>,
    dma0: Peri<'static, DMA_CH0>,
) {
    // TODO: Consider if interrupt handler needs to be set up for DMA_CH0
    let spi = Spi::new_txonly(spi0, pin18, pin19, dma0, spi::Config::default());
    let spi_bus = Mutex::new(RefCell::new(spi));
    let mut display_buffer = [0u8; 512];
    let mut display = PimoroniDisplayController::new(pin16, pin17, &spi_bus, &mut display_buffer);
    info!("Display controller initialised");

    loop {
        let next_frame = CORE1_SIGNAL.wait().await;
        display.draw_via_framebuffer(|display| candy_weigher_ui::draw(&next_frame, display));
    }
}

#[embassy_executor::task]
pub async fn pico_display_button_a_manager(
    pin12: Peri<'static, PIN_12>,
    tx: Sender<'static, ThreadModeRawMutex, Message, CHANNEL_SIZE>,
) {
    let button_a = Input::new(pin12, Pull::Up);
    manage_holdable_button(
        button_a,
        Message::ButtonAPressed,
        Message::ButtonAHeld,
        Message::ButtonAHoldCancelled,
        tx,
    )
    .await;
}
#[embassy_executor::task]
pub async fn pico_display_button_b_manager(
    pin13: Peri<'static, PIN_13>,
    tx: Sender<'static, ThreadModeRawMutex, Message, CHANNEL_SIZE>,
) {
    let button_b = Input::new(pin13, Pull::Up);
    manage_holdable_button(
        button_b,
        Message::ButtonBPressed,
        Message::ButtonBHeld,
        Message::ButtonBHoldCancelled,
        tx,
    )
    .await;
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

async fn manage_holdable_button<'a, M, Mutex, const BUTTON_CHANNEL_SIZE: usize>(
    mut button: Input<'static>,
    pressed_message: M,
    held_message: M,
    finished_holding_message: M,
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
            embassy_time::Timer::after_millis(500),
        )
        .await
        {
            continue;
        }
        tx.send(held_message).await;
        tx.send(pressed_message).await;
        while let Either::Second(_) = select(
            button.wait_for_high(),
            embassy_time::Timer::after_millis(100),
        )
        .await
        {
            tx.send(pressed_message).await;
        }
        tx.send(finished_holding_message).await;
    }
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

#[cfg(feature = "software-sim")]
#[embassy_executor::task]
pub async fn hx710_load_cell_manager_simulated(
    tx: Sender<'static, ThreadModeRawMutex, Message, CHANNEL_SIZE>,
) {
    const TEST_WEIGHT_DATA: &[(f32, Duration)] = &[
        (0.0, Duration::from_secs(5)),
        (1.0, Duration::from_millis(300)),
        (5.0, Duration::from_millis(300)),
        (10.0, Duration::from_millis(300)),
        (50.0, Duration::from_millis(300)),
        (150.0, Duration::from_millis(300)),
        (300.0, Duration::from_secs(10)),
        (295.0, Duration::from_millis(300)),
        (285.0, Duration::from_millis(300)),
        (275.0, Duration::from_secs(5)),
        (270.0, Duration::from_millis(300)),
        (260.0, Duration::from_millis(300)),
        (250.0, Duration::from_secs(5)),
    ];
    for (weight, duration) in TEST_WEIGHT_DATA.iter().cycle() {
        tx.send(Message::WeightUpdate(*weight)).await;
        embassy_time::Timer::after(*duration).await;
    }
}

#[cfg(feature = "hardware-sim")]
#[embassy_executor::task]
pub async fn hx710_load_cell_manager_rotary_encoder(
    pin26: Peri<'static, embassy_rp::peripherals::PIN_26>,
    pin27: Peri<'static, embassy_rp::peripherals::PIN_27>,
    // Button: vvv
    // pin28: Peri<'static, PIN_28>,
    pio0: Peri<'static, embassy_rp::peripherals::PIO0>,
    tx: Sender<'static, ThreadModeRawMutex, Message, CHANNEL_SIZE>,
) {
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
            Direction::Clockwise => base_weight += 2.5,
            Direction::CounterClockwise => base_weight -= 2.5,
        }
        tx.send(Message::WeightUpdate(base_weight)).await;
    }
}

#[embassy_executor::task]
pub async fn hx710_load_cell_manager(
    pin10: Peri<'static, PIN_10>,
    pin11: Peri<'static, PIN_11>,
    pio1: Peri<'static, PIO1>,
    tx: Sender<'static, ThreadModeRawMutex, Message, CHANNEL_SIZE>,
) {
    let Pio {
        mut common, sm0, ..
    } = Pio::new(pio1, Irqs);
    let program = PioHX710Program::new(&mut common);
    let mut load_cell = PioHX710::new(&mut common, sm0, pin11, pin10, &program);
    info!("HX710 PIO Task Started on PIO1");

    // Exponential moving average - to smooth readings.
    const EMA_FILTER_ALPHA: f32 = 0.2;
    let mut ema_weight_g = 0.0;
    let mut last_sent_g = 0.0;
    tx.send(Message::WeightUpdate(last_sent_g)).await;

    loop {
        let raw_val = load_cell.read().await;

        let calc_as_grams = (raw_val as f32 - SCALE_RAW_TARE) / SCALE_RAW_1G_STEP;
        ema_weight_g =
            (calc_as_grams * EMA_FILTER_ALPHA) + (ema_weight_g * (1.0 - EMA_FILTER_ALPHA));

        if (ema_weight_g - last_sent_g).abs() > 0.1 {
            last_sent_g = ema_weight_g;
            tx.send(Message::WeightUpdate(ema_weight_g)).await;
        }

        // Small delay to prevent flooding logs, though PIO will
        // naturally throttle based on the HX710 sample rate (10-40Hz).
        Timer::after(Duration::from_millis(100)).await;
    }
}
