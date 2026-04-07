use core::ops::Mul;

use crate::{
    config_consts::TOTAL_LED_FADEOUT_STEPS,
    hardware_controllers::flash::Config,
    state::{
        round_f32, round_f32_dp, DisplayBacklightState, LedState, ButtonState,
        ScreenShown, State,
    },
    tasks::ScaleRawWeight,
};
use defmt::debug;
use effect_light::Effect;
use embassy_time::Instant;

#[derive(Copy, Clone, Debug, defmt::Format)]
pub enum StateEffect {
    ButtonAPressed,
    ButtonBPressed,
    ButtonARepeated,
    ButtonBRepeated,
    ButtonAReleased,
    ButtonBReleased,
    ButtonXPressed,
    ButtonYPressed,
    ButtonXHeld,
    ButtonYHeld,
    ButtonXReleased,
    ButtonYReleased,
    WeightUpdate(ScaleRawWeight),
    CalibWeightUpdate(ScaleRawWeight),
    CalibModeComplete,
}

impl Effect<&mut State> for StateEffect {
    type Output = Option<crate::Effect>;
    fn resolve(self, state: &mut State) -> Self::Output {
        debug!("About to handle message: {}", self);
        // Special case - if showing the saving settings screen, consume the first button X press.
        if let ScreenShown::SavingSettings = state.screen_shown && matches!(self, StateEffect::ButtonXPressed) {
            state.screen_shown = ScreenShown::Main;
            return None;
        }
        // Special case - reset the backlight timer when a button is pressed, does not consume the press.
        if matches!(self, StateEffect::ButtonAPressed | StateEffect::ButtonBPressed | StateEffect::ButtonXPressed | StateEffect::ButtonYPressed) {
            state.backlight_state = DisplayBacklightState::On {
                on_at: Instant::now(),
            };
        }
        match self {
            StateEffect::ButtonAPressed => {
                state.lolly_weight_g += 0.1;
                state.t_l_pressed = ButtonState::On;
            }
            StateEffect::ButtonARepeated => {
                state.lolly_weight_g += 0.1;
            }
            StateEffect::ButtonAReleased => {
                state.t_l_pressed = ButtonState::Off;
            }
            StateEffect::ButtonBPressed => {
                state.lolly_weight_g -= 0.1;
                state.b_l_pressed = ButtonState::On;
            }
            StateEffect::ButtonBRepeated => {
                state.lolly_weight_g -= 0.1;
            }
            StateEffect::ButtonBReleased => {
                state.b_l_pressed = ButtonState::Off;
            }
            StateEffect::ButtonXPressed => {
                state.saved_tared_scale_weight_g = state.scale_weight_g - state.tare_weight_g;
                state.t_r_pressed = ButtonState::On;
            }
            StateEffect::ButtonYPressed => {
                state.tare_weight_g = state.scale_weight_g;
                state.b_r_pressed = ButtonState::On;
            }
            StateEffect::ButtonXHeld => {
                state.screen_shown = ScreenShown::SavingSettings;
                return Some(crate::Effect::WriteConfig(Config {
                    tare_weight_dg: round_f32(state.tare_weight_g * 10.0),
                    lolly_weight_dg: round_f32(state.lolly_weight_g * 10.0),
                    saved_tared_scale_weight: round_f32(state.saved_tared_scale_weight_g * 10.0),
                    scale_raw_50g: state.scale_raw_50g,
                    scale_raw_tare: state.scale_raw_tare,
                }));
            }
            StateEffect::ButtonYHeld => {
                state.screen_shown = ScreenShown::Calibration;
                return Some(crate::Effect::EnterCalibrationMode);
            }
            StateEffect::ButtonXReleased => state.t_r_pressed = ButtonState::Off,
            StateEffect::ButtonYReleased => state.b_r_pressed = ButtonState::Off,
            StateEffect::WeightUpdate(w) => {
                let prev_tared_scale_weight_g =
                    round_f32_dp(state.scale_weight_g - state.tare_weight_g, 1);
                let prev_lolly_count = round_f32(prev_tared_scale_weight_g / state.lolly_weight_g);

                state.scale_weight_g =
                    round_f32_dp(w.to_grams(state.scale_raw_tare, state.scale_raw_50g), 1);
                defmt::info!("New scale weight: {}", state.scale_weight_g);
                let tared_scale_weight_g =
                    round_f32_dp(state.scale_weight_g - state.tare_weight_g, 1);
                let lolly_count = round_f32(tared_scale_weight_g / state.lolly_weight_g);

                if lolly_count < prev_lolly_count {
                    state.led_state = LedState::Red {
                        total_steps: TOTAL_LED_FADEOUT_STEPS,
                        current_step: 0,
                        current_step_at: Instant::now(),
                    };
                    state.backlight_state = DisplayBacklightState::On {
                        on_at: Instant::now(),
                    };
                }
                if lolly_count > prev_lolly_count {
                    state.led_state = LedState::Blue {
                        total_steps: TOTAL_LED_FADEOUT_STEPS,
                        current_step: 0,
                        current_step_at: Instant::now(),
                    };
                    state.backlight_state = DisplayBacklightState::On {
                        on_at: Instant::now(),
                    };
                }
            }
            StateEffect::CalibWeightUpdate(w) => {
                state.displayed_calibration_value_raw = Some(w.get_raw());
            }
            StateEffect::CalibModeComplete => {
                state.displayed_calibration_value_raw = None;
                state.screen_shown = ScreenShown::Main;
            }
        };
        None
    }
}
