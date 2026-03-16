use embassy_rp::peripherals::{PIN_6, PIN_7, PIN_8, PWM_SLICE3, PWM_SLICE4};
use embassy_rp::pwm::{self, Pwm};
use embassy_rp::Peri;

pub struct Percentage(pub u16);

pub struct PimoroniDisplayRgbLedController<'a> {
    // red and green output shares this slice
    rg_pwm_slice: Pwm<'a>,
    b_pwm_slice: Pwm<'a>,
    rg_conf: pwm::Config,
    b_conf: pwm::Config,
}

impl<'a> PimoroniDisplayRgbLedController<'a> {
    pub fn new(
        slice_3: Peri<'a, PWM_SLICE3>,
        slice_4: Peri<'a, PWM_SLICE4>,
        pin_6: Peri<'a, PIN_6>,
        pin_7: Peri<'a, PIN_7>,
        pin_8: Peri<'a, PIN_8>,
    ) -> PimoroniDisplayRgbLedController<'a> {
        let mut pwm_config = pwm::Config::default();
        // high is off
        pwm_config.invert_a = true;
        pwm_config.invert_b = true;
        // max period per datasheet
        pwm_config.top = 65535;
        let rg_pwm_slice = Pwm::new_output_ab(slice_3, pin_6, pin_7, pwm_config.clone());
        let b_pwm_slice = Pwm::new_output_a(slice_4, pin_8, pwm_config.clone());
        Self {
            rg_pwm_slice,
            b_pwm_slice,
            rg_conf: pwm_config.clone(),
            b_conf: pwm_config,
        }
    }
    pub fn set_red(&mut self, brightness: Percentage) {
        self.rg_conf.compare_a = (0xffff * brightness.0 as usize / 100)
            .try_into()
            .unwrap_or(0xffff);
        self.rg_pwm_slice.set_config(&self.rg_conf);
    }
    pub fn red_on(&mut self) {
        self.rg_conf.compare_a = 0xffff;
        self.rg_pwm_slice.set_config(&self.rg_conf);
    }
    pub fn red_off(&mut self) {
        self.rg_conf.compare_a = 0x0000;
        self.rg_pwm_slice.set_config(&self.rg_conf);
    }
    pub fn set_green(&mut self, brightness: Percentage) {
        self.rg_conf.compare_b = (0xffff * brightness.0 as usize / 100)
            .try_into()
            .unwrap_or(0xffff);
        self.rg_pwm_slice.set_config(&self.rg_conf);
    }
    pub fn green_on(&mut self) {
        self.rg_conf.compare_b = 0xffff;
        self.rg_pwm_slice.set_config(&self.rg_conf);
    }
    pub fn green_off(&mut self) {
        self.rg_conf.compare_b = 0x0000;
        self.rg_pwm_slice.set_config(&self.rg_conf);
    }
    pub fn set_blue(&mut self, brightness: Percentage) {
        self.b_conf.compare_a = (0xffff * brightness.0 as usize / 100)
            .try_into()
            .unwrap_or(0xffff);
        self.b_pwm_slice.set_config(&self.b_conf);
    }
    pub fn blue_on(&mut self) {
        self.b_conf.compare_a = 0xffff;
        self.b_pwm_slice.set_config(&self.b_conf);
    }
    pub fn blue_off(&mut self) {
        self.b_conf.compare_a = 0x0000;
        self.b_pwm_slice.set_config(&self.b_conf);
    }
    pub fn all_off(&mut self) {
        self.red_off();
        self.blue_off();
        self.green_off();
    }
}
