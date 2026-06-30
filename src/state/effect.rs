use core::ops::Mul;

use crate::{
    DisplayTimer, EnterOrProgressCalibrationMode, LEDTimer, WriteConfig,
    config_consts::TOTAL_LED_FADEOUT_STEPS,
    hardware_controllers::flash::Config,
    state::{
        ButtonState, CalibrationState, DisplayBacklightState, LedState, ScreenShown, State,
        round_f32,
    },
    tasks::ScaleRawWeight,
};
use defmt::{debug, warn};
use effect_lite::Effect;
use embassy_time::Instant;

#[derive(Copy, Clone, Debug, defmt::Format)]
pub enum Event {
    Button(ButtonEvent),
    LoadCell(LoadCellEvent),
    Timer(TimerEvent),
}

#[derive(Copy, Clone, Debug, defmt::Format)]
pub enum TimerEvent {
    FadeoutLEDs { start_time: Instant },
    DimDisplay { start_time: Instant },
    SleepDisplay { start_time: Instant },
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

#[derive(Copy, Clone, Debug, defmt::Format)]
pub enum LoadCellEvent {
    WeightUpdate(ScaleRawWeight),
    EnteredCalibMode,
    CalibTareWeightUpdate(ScaleRawWeight),
    CalibTareWeightModeComplete,
    Calib50gWeightUpdate(ScaleRawWeight),
    CalibModeComplete,
}

impl Effect<&mut State> for Event {
    type Output = (
        Option<WriteConfig>,
        Option<EnterOrProgressCalibrationMode>,
        Option<LEDTimer>,
        Option<DisplayTimer>,
    );
    fn resolve(self, state: &mut State) -> Self::Output {
        let mut write_config_effect = None;
        let mut enter_or_progress_calibration_mode_effect = None;
        let mut led_timer_effect = None;
        let mut display_timer_effect = None;
        debug!("About to handle message: {}", self);
        // Special case - if showing the saving settings screen, consume button events.
        if let (ScreenShown::SavingSettings, Event::Button(button_event)) =
            (state.screen_shown, self)
        {
            // Specifically, transition screen on X.
            if matches!(button_event, ButtonEvent::XPressed) {
                state.screen_shown = ScreenShown::Main;
                return Default::default();
            }
        }
        // Special case - if showing the calibration screen, consume button events.
        if let (ScreenShown::Calibration(calibration_state), Event::Button(button_event)) =
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
                            ScreenShown::Calibration(CalibrationState::Calibrating50g(0.0))
                    }
                    CalibrationState::Calibrated => state.screen_shown = ScreenShown::Main,
                    _ => (),
                }
                return Default::default();
            }
        }
        // Special case - reset the backlight timer when a button is pressed, does not
        // consume the press.
        if matches!(self, Event::Button(_)) {
            state.backlight_state = DisplayBacklightState::On {
                on_at: Instant::now(),
            };
        }
        match self {
            Event::Button(ButtonEvent::APressed) => {
                state.lolly_weight_g += 0.1;
                state.t_l_pressed = ButtonState::On;
            }
            Event::Button(ButtonEvent::ARepeated) => {
                state.lolly_weight_g += 0.1;
            }
            Event::Button(ButtonEvent::AReleased) => {
                state.t_l_pressed = ButtonState::Off;
            }
            Event::Button(ButtonEvent::BPressed) => {
                state.lolly_weight_g -= 0.1;
                state.b_l_pressed = ButtonState::On;
            }
            Event::Button(ButtonEvent::BRepeated) => {
                state.lolly_weight_g -= 0.1;
            }
            Event::Button(ButtonEvent::BReleased) => {
                state.b_l_pressed = ButtonState::Off;
            }
            Event::Button(ButtonEvent::XPressed) => {
                state.saved_tared_scale_weight_g = state.scale_weight_g - state.tare_weight_g;
                state.t_r_pressed = ButtonState::On;
            }
            Event::Button(ButtonEvent::YPressed) => {
                state.tare_weight_g = state.scale_weight_g;
                state.b_r_pressed = ButtonState::On;
            }
            Event::Button(ButtonEvent::XHeld(progress)) => {
                if round_f32(progress * 100.0) == 100 {
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
            Event::Button(ButtonEvent::YHeld(progress)) => {
                if round_f32(progress * 100.0) == 100 {
                    state.screen_shown = ScreenShown::Calibration(CalibrationState::Loading);
                    return Some(crate::OfficeCandyWeigherEffect::EnterOrProgressCalibrationMode);
                } else {
                    state.b_r_pressed = ButtonState::Mid(progress)
                }
            }
            Event::Button(ButtonEvent::XReleased) => state.t_r_pressed = ButtonState::Off,
            Event::Button(ButtonEvent::YReleased) => state.b_r_pressed = ButtonState::Off,
            Event::LoadCell(LoadCellEvent::WeightUpdate(w)) => {
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
            Event::LoadCell(LoadCellEvent::CalibTareWeightUpdate(w)) => {
                state.displayed_calibration_value_raw = Some(w.get_raw());
            }
            Event::LoadCell(LoadCellEvent::CalibModeComplete) => {
                state.displayed_calibration_value_raw = None;
                state.screen_shown = ScreenShown::Main;
            }
        };
        None
    }
}
