use crate::{Message, CHANNEL_SIZE};
use defmt::info;
use embassy_futures::select::{select, Either};
use embassy_rp::gpio::{Input, Pull};
use embassy_rp::peripherals::{
    DMA_CH0, PIN_10, PIN_11, PIN_12, PIN_13, PIN_14, PIN_15, PIN_20, PIN_21, PIN_26, PIN_27,
    PIN_28, PIO0, PIO1,
};
use embassy_rp::pio::{self, InterruptHandler, Pio, ShiftDirection};
use embassy_rp::pio_programs::rotary_encoder::{Direction, PioEncoder, PioEncoderProgram};
use embassy_rp::{bind_interrupts, Peri};
use embassy_sync::blocking_mutex::raw::{RawMutex, ThreadModeRawMutex};
use embassy_sync::channel::Sender;
use embassy_time::{Duration, Timer};

#[cfg(feature = "hardware-sim")]
bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => InterruptHandler<PIO0>;
    PIO1_IRQ_0 => InterruptHandler<PIO1>;
});

#[cfg(not(feature = "hardware-sim"))]
bind_interrupts!(struct Irqs {
    PIO1_IRQ_0 => InterruptHandler<PIO1>;
});

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
        // Wait for long press
        if let Either::First(_) = select(
            button.wait_for_high(),
            embassy_time::Timer::after_millis(500),
        )
        .await
        {
            continue;
        }
        tx.send(pressed_message).await;
        while let Either::Second(_) = select(
            button.wait_for_high(),
            embassy_time::Timer::after_millis(50),
        )
        .await
        {
            tx.send(pressed_message).await;
        }
    }
}

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
    pin26: Peri<'static, PIN_26>,
    pin27: Peri<'static, PIN_27>,
    // Button: vvv
    // pin28: Peri<'static, PIN_28>,
    pio0: Peri<'static, PIO0>,
    tx: Sender<'static, ThreadModeRawMutex, Message, CHANNEL_SIZE>,
) {
    let Pio {
        mut common, sm0, ..
    } = Pio::new(pio0, Irqs);
    let program = PioEncoderProgram::new(&mut common);
    let mut encoder = PioEncoder::new(&mut common, sm0, pin26, pin27, &program);

    let mut base_weight = 0.0;
    loop {
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
    let mut asm = pio::program::Assembler::<{ pio::program::RP2040_MAX_PROGRAM_SIZE }>::new();
    // Panics without this
    asm.side_set = pio::program::SideSet::new(false, 1, false);

    let mut label_start = asm.label();
    let mut label_bitloop = asm.label();

    // Side-set is configured for the Clock (SCK) pin
    asm.bind(&mut label_start);
    asm.wait_with_side_set(0, pio::program::WaitSource::PIN, 0, false, 0); // Wait for DOUT (pin 0) to go low
    asm.set_with_side_set(pio::program::SetDestination::X, 23, 0); // Prepare to read 24 bits

    asm.bind(&mut label_bitloop);
    // Clock High (side-set 1), delay 1
    asm.nop_with_delay_and_side_set(1, 1);
    // Sample DOUT, Clock Low (side-set 0), delay 1
    asm.in_with_delay_and_side_set(pio::program::InSource::PINS, 1, 1, 0);
    asm.jmp_with_side_set(
        pio::program::JmpCondition::XDecNonZero,
        &mut label_bitloop,
        0,
    );

    // 25th pulse for Gain 128 (HX710 default)
    asm.nop_with_delay_and_side_set(1, 1);
    asm.nop_with_delay_and_side_set(0, 0);

    // Push 24-bit result to FIFO, no block (it's already ready)
    asm.push_with_side_set(false, false, 0);

    let prg = asm.assemble_program();

    let Pio {
        mut common,
        mut sm0,
        ..
    } = Pio::new(pio1, Irqs);

    let installed = common.load_program(&prg);
    let sclk = common.make_pio_pin(pin11);
    let dout = common.make_pio_pin(pin10);

    let mut cfg = pio::Config::default();
    cfg.use_program(&installed, &[&sclk]); // Set SCLK as the side-set pin
    cfg.set_in_pins(&[&dout]); // Set DOUT as the IN pin (index 0 for the 'wait' and 'in' instructions)
    cfg.set_set_pins(&[&sclk]);
    cfg.set_out_pins(&[&sclk]);

    // 1. EXPLICITLY SET PIN DIRECTIONS
    sm0.set_pin_dirs(embassy_rp::pio::Direction::Out, &[&sclk]);
    sm0.set_pin_dirs(embassy_rp::pio::Direction::In, &[&dout]);

    // 2. SET INITIAL CLOCK STATE TO LOW (Wakes up HX710)
    sm0.set_pins(embassy_rp::gpio::Level::Low, &[&sclk]);

    cfg.clock_divider = 125u16.into();
    cfg.shift_in.direction = ShiftDirection::Left;
    cfg.shift_in.auto_fill = false;
    cfg.shift_in.threshold = 32;

    sm0.set_config(&cfg);
    sm0.set_enable(true);

    info!("HX710 PIO Task Started on PIO1");

    let mut ema_weight_g = 0.0;
    let mut last_sent_g = 0.0;
    tx.send(Message::WeightUpdate(last_sent_g)).await;
    // Exponention moving average
    const EMA_FILTER_ALPHA: f32 = 0.2;

    loop {
        // Wait for the PIO to push a 24-bit value into the RX FIFO
        let mut raw_val = sm0.rx().wait_pull().await;

        // Sign extend from 24-bit to 32-bit i32
        if (raw_val & 0x800000) != 0 {
            raw_val |= 0xFF000000;
        }
        let signed_val = raw_val as i32;

        const CALIB_TARE: f32 = 8192.0;
        const CALIB_SCALE: f32 = 1416.02;

        let calc_as_grams = (signed_val as f32 - CALIB_TARE) / CALIB_SCALE;
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
