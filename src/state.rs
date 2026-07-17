use crate::{
    CORE1_SIGNAL, StartDimOrSleepDisplayTimer, StartLEDTimer,
    candy_weigher_ui::DisplayState,
    config_consts::{
        DEFAULT_LOLLY_WEIGHT, DEFAULT_SCALE_RAW_50G, DEFAULT_SCALE_RAW_TARE, MAX_LED_ON_TIME,
        TIME_FROM_BACKLIGHT_LOW_TO_OFF, TIME_TO_BACKLIGHT_LOW, TOTAL_LED_FADEOUT_STEPS,
    },
    hardware_controllers::pimoroni_display_leds::{Percentage, PimoroniDisplayRgbLedController},
    utils::{ScaleRawWeight, round_f32},
};
use core::ops::Mul;
use defmt::debug;
use embassy_time::Instant;

pub mod effect;

pub struct State {
    pub tare_weight_g: f32,
    pub scale_weight_g: f32,
    pub saved_tared_scale_weight_g: f32,
    pub lolly_weight_g: f32,
    pub scale_raw_tare: ScaleRawWeight,
    pub scale_raw_50g: ScaleRawWeight,
    pub y_pressed: ButtonState,
    pub x_pressed: ButtonState,
    pub b_pressed: ButtonState,
    pub a_pressed: ButtonState,
    pub led_state: LedState,
    pub backlight_state: DisplayBacklightState,
    pub last_backlight_state: Option<DisplayBacklightState>,
    pub last_display_state: Option<DisplayState>,
    pub last_led_state: Option<LedState>,
    pub screen_shown: ScreenShown,
    pub battery_state: BatteryState,
}

#[derive(PartialEq, Copy, Clone)]
pub enum CalibrationState {
    Loading,
    WaitingConfirmation,
    CalibratingTare {
        latest_tare_calib_value: ScaleRawWeight,
    },
    TareCalibrated {
        latest_tare_calib_value: ScaleRawWeight,
    },
    Calibrating50g {
        latest_tare_calib_value: ScaleRawWeight,
        latest_50g_calib_value: ScaleRawWeight,
    },
    Calibrated {
        latest_tare_calib_value: ScaleRawWeight,
        latest_50g_calib_value: ScaleRawWeight,
    },
}

#[derive(Default, PartialEq, Copy, Clone)]
pub enum ScreenShown {
    #[default]
    Main,
    Calibration(CalibrationState),
    SavingSettings,
}

#[derive(Eq, PartialEq, Copy, Clone)]
pub enum DisplayBacklightState {
    Off,
    LowPower { on_at: Instant },
    On { on_at: Instant },
}

#[derive(Default, PartialEq, Copy, Clone)]
pub enum BatteryState {
    #[default]
    Unknown,
    High,
    Medium,
    Low,
    Critical,
}

impl DisplayBacklightState {
    pub fn handle_transition(
        &mut self,
        prev_on_at: Instant,
    ) -> Option<StartDimOrSleepDisplayTimer> {
        match self {
            DisplayBacklightState::Off => {
                *self = DisplayBacklightState::Off;
                None
            }
            // Handle case where timer is no longer valid, ie, state was reset after timer
            // set.
            DisplayBacklightState::LowPower { on_at } if *on_at == prev_on_at => {
                *self = DisplayBacklightState::Off;
                None
            }
            // Handle case where timer is no longer valid, ie, state was reset after timer
            // set.
            DisplayBacklightState::On { on_at } if *on_at == prev_on_at => {
                *self = DisplayBacklightState::LowPower { on_at: *on_at };
                TIME_FROM_BACKLIGHT_LOW_TO_OFF.map(|in_dur| StartDimOrSleepDisplayTimer {
                    start_time: prev_on_at,
                    in_dur,
                })
            }
            _ => None,
        }
    }
    pub fn reset(&mut self) -> StartDimOrSleepDisplayTimer {
        let now = Instant::now();
        *self = DisplayBacklightState::On { on_at: now };
        StartDimOrSleepDisplayTimer {
            start_time: now,
            in_dur: TIME_TO_BACKLIGHT_LOW,
        }
    }
}

#[derive(Default, PartialEq, Clone, Copy)]
pub enum ButtonState {
    #[default]
    Off,
    Held(f32),
    On,
}

#[derive(Copy, Clone, Default, PartialEq)]
pub enum LedState {
    #[default]
    Off,
    Red {
        total_steps: u16,
        current_step: u16,
        initially_on_at: Instant,
    },
    Blue {
        total_steps: u16,
        current_step: u16,
        initially_on_at: Instant,
    },
}

impl LedState {
    pub fn handle_transition(&mut self, timer_initially_on_at: Instant) -> Option<StartLEDTimer> {
        const {
            assert!(TOTAL_LED_FADEOUT_STEPS != 0);
        };
        let step_dur = MAX_LED_ON_TIME / TOTAL_LED_FADEOUT_STEPS as u32;
        match self {
            LedState::Off => {
                *self = LedState::Off;
                None
            }
            // Case: received a message from an outdated timer
            LedState::Red {
                initially_on_at, ..
            }
            | LedState::Blue {
                initially_on_at, ..
            } if *initially_on_at != timer_initially_on_at => None,
            // Case: last step of transition
            LedState::Red {
                total_steps,
                current_step,
                ..
            }
            | LedState::Blue {
                total_steps,
                current_step,
                ..
            } if *current_step + 1 >= *total_steps => {
                *self = LedState::Off;
                None
            }
            LedState::Red {
                total_steps,
                current_step,
                initially_on_at,
            } => {
                let total_steps = *total_steps;
                let current_step = *current_step + 1;
                let initially_on_at = *initially_on_at;
                *self = LedState::Red {
                    total_steps,
                    current_step,
                    initially_on_at,
                };
                Some(StartLEDTimer {
                    start_time: initially_on_at,
                    next_at: initially_on_at + current_step as u32 * step_dur,
                })
            }
            LedState::Blue {
                total_steps,
                current_step,
                initially_on_at,
            } => {
                let total_steps = *total_steps;
                let current_step = *current_step + 1;
                let initially_on_at = *initially_on_at;
                *self = LedState::Blue {
                    total_steps,
                    current_step,
                    initially_on_at,
                };
                Some(StartLEDTimer {
                    start_time: initially_on_at,
                    next_at: initially_on_at + current_step as u32 * step_dur,
                })
            }
        }
    }
    pub fn set_red(&mut self) -> StartLEDTimer {
        let now = Instant::now();
        *self = LedState::Red {
            total_steps: TOTAL_LED_FADEOUT_STEPS,
            current_step: 0,
            initially_on_at: now,
        };
        const {
            assert!(TOTAL_LED_FADEOUT_STEPS != 0);
        };
        StartLEDTimer {
            start_time: now,
            next_at: now + MAX_LED_ON_TIME / TOTAL_LED_FADEOUT_STEPS as u32,
        }
    }
    pub fn set_blue(&mut self) -> StartLEDTimer {
        let now = Instant::now();
        *self = LedState::Blue {
            total_steps: TOTAL_LED_FADEOUT_STEPS,
            current_step: 0,
            initially_on_at: now,
        };
        const {
            assert!(TOTAL_LED_FADEOUT_STEPS != 0);
        };
        StartLEDTimer {
            start_time: now,
            next_at: now + MAX_LED_ON_TIME / TOTAL_LED_FADEOUT_STEPS as u32,
        }
    }
}

impl Default for State {
    fn default() -> Self {
        Self {
            tare_weight_g: Default::default(),
            scale_weight_g: Default::default(),
            saved_tared_scale_weight_g: Default::default(),
            y_pressed: Default::default(),
            x_pressed: Default::default(),
            b_pressed: Default::default(),
            a_pressed: Default::default(),
            led_state: Default::default(),
            last_display_state: Default::default(),
            last_led_state: Default::default(),
            last_backlight_state: Default::default(),
            screen_shown: Default::default(),
            backlight_state: DisplayBacklightState::On {
                on_at: Instant::now(),
            },
            lolly_weight_g: DEFAULT_LOLLY_WEIGHT,
            scale_raw_tare: DEFAULT_SCALE_RAW_TARE,
            scale_raw_50g: DEFAULT_SCALE_RAW_50G,
            battery_state: Default::default(),
        }
    }
}

impl State {
    pub fn to_display_state(&self) -> DisplayState {
        match self.screen_shown {
            ScreenShown::Main => {
                // Round off to 1 d.p (prevent overdrawing to display)
                let tared_scale_weight_g =
                    round_f32((self.scale_weight_g - self.tare_weight_g).mul(10.0)) as f32 / 10.0;
                let lolly_count = round_f32(tared_scale_weight_g / self.lolly_weight_g)
                    .try_into()
                    .unwrap_or_default();
                let prev_lolly_count =
                    round_f32(self.saved_tared_scale_weight_g / self.lolly_weight_g);
                DisplayState::MainScreen {
                    scale_weight_g: self.scale_weight_g - self.tare_weight_g,
                    lolly_weight_g: self.lolly_weight_g,
                    lolly_count,
                    lolly_count_change: lolly_count as i32 - prev_lolly_count,
                    t_l_state: self.y_pressed,
                    t_r_state: self.b_pressed,
                    b_l_state: self.x_pressed,
                    b_r_state: self.a_pressed,
                    battery_state: self.battery_state,
                }
            }
            ScreenShown::Calibration(state) => DisplayState::CalibrationScreen(state),
            ScreenShown::SavingSettings => DisplayState::SavingSettingsScreen,
        }
    }
}

pub fn output_state(
    state: &mut State,
    display_led_controller: &mut PimoroniDisplayRgbLedController,
) {
    if state.last_led_state.as_ref() != Some(&state.led_state) {
        debug!("Updating LEDS");
        match state.led_state {
            LedState::Off => display_led_controller.all_off(),
            LedState::Red {
                total_steps,
                current_step,
                ..
            } => {
                display_led_controller.blue_off();
                display_led_controller.set_red(Percentage(
                    100 * total_steps.saturating_sub(current_step) / total_steps,
                ))
            }
            LedState::Blue {
                total_steps,
                current_step,
                ..
            } => {
                display_led_controller.red_off();
                display_led_controller.set_blue(Percentage(
                    100 * total_steps.saturating_sub(current_step) / total_steps,
                ))
            }
        };
        state.last_led_state = Some(state.led_state);
    }
    let next_display_state = state.to_display_state();
    if state.last_display_state.as_ref() != Some(&next_display_state)
        || state.last_backlight_state.as_ref() != Some(&state.backlight_state)
    {
        CORE1_SIGNAL.signal((next_display_state.clone(), state.backlight_state));
        state.last_display_state = Some(next_display_state);
        state.last_backlight_state = Some(state.backlight_state);
    }
}
