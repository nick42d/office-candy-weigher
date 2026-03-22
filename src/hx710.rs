//! Implementation of HX710 (specifically HX710C but could be further
//! generalised) for RP2040 using PIO.
use embassy_rp::{
    Peri,
    pio::{self, Common, Instance, LoadedProgram, PioPin, ShiftDirection, StateMachine},
};

pub struct PioHX710Program<'a, PIO: Instance>(LoadedProgram<'a, PIO>);

impl<'a, PIO: Instance> PioHX710Program<'a, PIO> {
    pub fn new(common: &mut Common<'a, PIO>) -> Self {
        // This is a rough translation of this https://github.com/robert-hh/hx710/blob/master/hx710_pio.py
        // Datasheet is located here: https://www.micros.com.pl/mediaserver/info-uphx710c%20smd.pdf
        let mut asm = pio::program::Assembler::<{ pio::program::RP2040_MAX_PROGRAM_SIZE }>::new();
        // Configure side set - one bit (output for Sclk).
        asm.side_set = pio::program::SideSet::new(false, 1, false);

        let mut label_start = asm.label();
        let mut label_bitloop = asm.label();

        asm.bind(&mut label_start);
        // Wait for DOUT (pin 0) to go low
        asm.wait_with_side_set(0, pio::program::WaitSource::PIN, 0, false, 0);
        // Prepare to read 24 bits
        asm.set_with_side_set(pio::program::SetDestination::X, 23, 0);
        asm.bind(&mut label_bitloop);
        // Clock High (side-set 1), delay 1
        asm.nop_with_delay_and_side_set(1, 1);
        // Sample DOUT, Clock Low (side-set 0), delay 1
        asm.in_with_delay_and_side_set(pio::program::InSource::PINS, 1, 1, 0);
        // Test for more bits.
        asm.jmp_with_side_set(
            pio::program::JmpCondition::XDecNonZero,
            &mut label_bitloop,
            0,
        );
        // 25th pulse for Gain 128 (HX710C default - 10Hz).
        asm.nop_with_delay_and_side_set(1, 1);
        asm.nop_with_delay_and_side_set(0, 0);

        // Push 24-bit result to FIFO, no block (it's already ready)
        asm.push_with_side_set(false, false, 0);

        let prg = asm.assemble_program();
        Self(common.load_program(&prg))
    }
}
pub struct PioHX710<'a, PIO: Instance, const SM: usize>(StateMachine<'a, PIO, SM>);

impl<'a, PIO: Instance, const SM: usize> PioHX710<'a, PIO, SM> {
    pub fn new(
        common: &mut Common<'a, PIO>,
        mut sm: StateMachine<'a, PIO, SM>,
        sclk_pin: Peri<'a, impl PioPin>,
        dout_pin: Peri<'a, impl PioPin>,
        program: &PioHX710Program<'a, PIO>,
    ) -> Self {
        let sclk = common.make_pio_pin(sclk_pin);
        let dout = common.make_pio_pin(dout_pin);

        sm.set_pin_dirs(embassy_rp::pio::Direction::Out, &[&sclk]);
        sm.set_pin_dirs(embassy_rp::pio::Direction::In, &[&dout]);

        // Set initial sclk to low (Wakes up HX710)
        sm.set_pins(embassy_rp::gpio::Level::Low, &[&sclk]);

        let mut cfg = pio::Config::default();

        // Set SCLK as the side-set pin
        cfg.use_program(&program.0, &[&sclk]);
        cfg.set_set_pins(&[&sclk]);

        cfg.set_in_pins(&[&dout]);
        cfg.set_out_pins(&[&sclk]);

        // Standard clock speed is 133 Mhz.
        // Typical clock period of HX710C is 2 microseconds (500kHz)
        cfg.clock_divider = (133u16 * (1000 / 500)).into();
        cfg.shift_in.direction = ShiftDirection::Left;
        cfg.shift_in.auto_fill = false;
        cfg.shift_in.threshold = 32;

        sm.set_config(&cfg);
        sm.set_enable(true);

        Self(sm)
    }
    // Read the next raw value when it's available.
    pub async fn read(&mut self) -> i32 {
        // Wait for the PIO to push a 24-bit value into the RX FIFO
        let mut raw_val = self.0.rx().wait_pull().await;

        // Sign extend from 24-bit to 32-bit i32
        if (raw_val & 0x800000) != 0 {
            raw_val |= 0xFF000000;
        }
        raw_val as i32
    }
}
