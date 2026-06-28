use core::ops::Mul;

use crate::{
    config_consts::TOTAL_LED_FADEOUT_STEPS,
    hardware_controllers::flash::Config,
    state::{
        round_f32, round_f32_dp, ButtonState, CalibrationState, DisplayBacklightState, LedState,
        ScreenShown, State,
    },
    tasks::ScaleRawWeight,
};
use defmt::{debug, warn};
use effect_lite::Effect;
use embassy_time::Instant;

#[derive(Copy, Clone, Debug, defmt::Format)]
pub enum HardwareEvent {
    Button(ButtonEvent),
    WeightUpdate(ScaleRawWeight),
    CalibWeightUpdate(ScaleRawWeight),
    CalibModeComplete,
}

#[derive(Copy, Clone, Debug, defmt::Format)]
pub enum ButtonEvent {
    APressed,
    BPressed,
    ARepeated,
    BRepeated,
    AReleased,
    BReleased,
    XPressed,
    YPressed,
    XHeld(f32),
    YHeld(f32),
    XReleased,
    YReleased,
}

impl Effect<&mut State> for HardwareEvent {
    type Output = Option<crate::OfficeCandyWeigherEffect>;
    fn resolve(self, state: &mut State) -> Self::Output {
        debug!("About to handle message: {}", self);
        // Special case - if showing the saving settings screen, consume button events.
        if let (ScreenShown::SavingSettings, HardwareEvent::Button(button_event)) =
            (state.screen_shown, self)
        {
            // Specifically, transition screen on X.
            if matches!(button_event, ButtonEvent::XPressed) {
                state.screen_shown = ScreenShown::Main;
                return None;
            }
        }
        // Special case - if showing the calibration screen, consume button events.
        if let (ScreenShown::Calibration(calibration_state), HardwareEvent::Button(button_event)) =
            (state.screen_shown, self)
        {
            // Specifically, transition the correct screen on X.
            if matches!(button_event, ButtonEvent::XPressed) {
                match calibration_state {
                    // TODO: Send an effect to weigh scale controller
                    CalibrationState::WaitingConfirmation => {
                        state.screen_shown =
                            ScreenShown::Calibration(CalibrationState::CalibratingTare(0.0))
                    }
                    CalibrationState::TareCalibrated(_) => {
                        state.screen_shown =
                            ScreenShown::Calibration(CalibrationState::Calibrating25g(0.0))
                    }
                    CalibrationState::Calibrated => state.screen_shown = ScreenShown::Main,
                    _ => (),
                }
                return None;
            }
        }
        // Special case - reset the backlight timer when a button is pressed, does not
        // consume the press.
        if matches!(self, HardwareEvent::Button(_)) {
            state.backlight_state = DisplayBacklightState::On {
                on_at: Instant::now(),
            };
        }
        match self {
            HardwareEvent::Button(ButtonEvent::APressed) => {
                state.lolly_weight_g += 0.1;
                state.t_l_pressed = ButtonState::On;
            }
            HardwareEvent::Button(ButtonEvent::ARepeated) => {
                state.lolly_weight_g += 0.1;
            }
            HardwareEvent::Button(ButtonEvent::AReleased) => {
                state.t_l_pressed = ButtonState::Off;
            }
            HardwareEvent::Button(ButtonEvent::BPressed) => {
                state.lolly_weight_g -= 0.1;
                state.b_l_pressed = ButtonState::On;
            }
            HardwareEvent::Button(ButtonEvent::BRepeated) => {
                state.lolly_weight_g -= 0.1;
            }
            HardwareEvent::Button(ButtonEvent::BReleased) => {
                state.b_l_pressed = ButtonState::Off;
            }
            HardwareEvent::Button(ButtonEvent::XPressed) => {
                state.saved_tared_scale_weight_g = state.scale_weight_g - state.tare_weight_g;
                state.t_r_pressed = ButtonState::On;
            }
            HardwareEvent::Button(ButtonEvent::YPressed) => {
                state.tare_weight_g = state.scale_weight_g;
                state.b_r_pressed = ButtonState::On;
            }
            HardwareEvent::Button(ButtonEvent::XHeld(progress)) => {
                if round_f32_dp(progress, 1) == 1.0 {
                    state.screen_shown = ScreenShown::SavingSettings;
                    return Some(crate::OfficeCandyWeigherEffect::WriteConfig(Config {
                        tare_weight_dg: round_f32(state.tare_weight_g * 10.0),
                        lolly_weight_dg: round_f32(state.lolly_weight_g * 10.0),
                        saved_tared_scale_weight: round_f32(
                            state.saved_tared_scale_weight_g * 10.0,
                        ),
                        scale_raw_50g: state.scale_raw_50g,
                        scale_raw_tare: state.scale_raw_tare,
                    }));
                } else {
                    state.t_r_pressed = ButtonState::Mid(progress)
                }
            }
            HardwareEvent::Button(ButtonEvent::YHeld(progress)) => {
                if round_f32_dp(progress, 1) == 1.0 {
                    state.screen_shown = ScreenShown::Calibration(Default::default());
                    return Some(crate::OfficeCandyWeigherEffect::EnterCalibrationMode);
                } else {
                    state.b_r_pressed = ButtonState::Mid(progress)
                }
            }
            HardwareEvent::Button(ButtonEvent::XReleased) => state.t_r_pressed = ButtonState::Off,
            HardwareEvent::Button(ButtonEvent::YReleased) => state.b_r_pressed = ButtonState::Off,
            HardwareEvent::WeightUpdate(w) => {
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
            HardwareEvent::CalibWeightUpdate(w) => {
                state.displayed_calibration_value_raw = Some(w.get_raw());
            }
            HardwareEvent::CalibModeComplete => {
                state.displayed_calibration_value_raw = None;
                state.screen_shown = ScreenShown::Main;
            }
        };
        None
    }
}
