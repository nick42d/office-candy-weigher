use crate::{
    EnterOrProgressCalibrationMode, StartDimOrSleepDisplayTimer, StartLEDTimer, WriteConfig,
    hardware_controllers::flash::Config,
    state::{BatteryState, ButtonState, CalibrationState, ScreenShown, State, round_f32},
    utils::{ScaleRawWeight, round_f32_dp},
};
use effect_lite::Effect;
use embassy_time::Instant;

#[derive(Copy, Clone, Debug, defmt::Format)]
pub enum Event {
    Button(ButtonEvent),
    LoadCell(LoadCellEvent),
    Timer(TimerEvent),
    BatteryMonitorUpdate(BatteryState),
}

#[derive(Copy, Clone, Debug, defmt::Format)]
pub enum TimerEvent {
    FadeoutLEDs { start_time: Instant },
    DimOrSleepDisplay { start_time: Instant },
}

#[derive(Copy, Clone, Debug, defmt::Format)]
pub enum ButtonEvent {
    APressed,
    BPressed,
    AHeld(f32),
    BHeld(f32),
    AReleased,
    BReleased,
    XPressed,
    YPressed,
    XRepeated,
    YRepeated,
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
        Option<StartLEDTimer>,
        Option<StartDimOrSleepDisplayTimer>,
    );
    fn resolve(self, state: &mut State) -> Self::Output {
        let mut write_config_effect = None;
        let mut enter_or_progress_calibration_mode_effect = None;
        let mut led_timer_effect = None;
        let mut backlight_timer_effect = None;
        // Special case - reset the backlight timer on any button event.
        // Does not consume the press.
        if matches!(self, Event::Button(_)) {
            backlight_timer_effect = Some(state.backlight_state.reset());
        }
        // Special case - if showing the saving settings screen, consume button events.
        if let (ScreenShown::SavingSettings, Event::Button(button_event)) =
            (state.screen_shown, self)
        {
            // Specifically, transition screen on X.
            if matches!(button_event, ButtonEvent::XPressed) {
                state.screen_shown = ScreenShown::Main;
                return (
                    write_config_effect,
                    enter_or_progress_calibration_mode_effect,
                    led_timer_effect,
                    backlight_timer_effect,
                );
            }
        }
        // Special case - if showing the calibration screen, consume button events.
        if let (ScreenShown::Calibration(calibration_state), Event::Button(button_event)) =
            (state.screen_shown, self)
        {
            // Specifically, transition the correct screen on X.
            if matches!(button_event, ButtonEvent::XPressed) {
                match calibration_state {
                    CalibrationState::WaitingConfirmation => {
                        state.screen_shown =
                            ScreenShown::Calibration(CalibrationState::CalibratingTare {
                                latest_tare_calib_value: ScaleRawWeight::default(),
                            });
                        enter_or_progress_calibration_mode_effect =
                            Some(EnterOrProgressCalibrationMode);
                    }
                    CalibrationState::TareCalibrated {
                        latest_tare_calib_value,
                    } => {
                        state.screen_shown =
                            ScreenShown::Calibration(CalibrationState::Calibrating50g {
                                latest_tare_calib_value,
                                latest_50g_calib_value: ScaleRawWeight::default(),
                            });
                        enter_or_progress_calibration_mode_effect =
                            Some(EnterOrProgressCalibrationMode);
                    }
                    CalibrationState::Calibrated {
                        latest_tare_calib_value,
                        latest_50g_calib_value,
                    } => {
                        state.scale_raw_tare = latest_tare_calib_value;
                        state.scale_raw_50g = latest_50g_calib_value;
                        state.screen_shown = ScreenShown::Main;
                        enter_or_progress_calibration_mode_effect =
                            Some(EnterOrProgressCalibrationMode);
                    }
                    _ => (),
                }
            }
            return (
                write_config_effect,
                enter_or_progress_calibration_mode_effect,
                led_timer_effect,
                backlight_timer_effect,
            );
        }
        match self {
            Event::Button(ButtonEvent::YPressed) => {
                state.lolly_weight_g += 0.1;
                state.y_pressed = ButtonState::On;
            }
            Event::Button(ButtonEvent::YRepeated) => {
                state.lolly_weight_g += 0.1;
            }
            Event::Button(ButtonEvent::YReleased) => {
                state.y_pressed = ButtonState::Off;
            }
            Event::Button(ButtonEvent::XPressed) => {
                state.lolly_weight_g -= 0.1;
                state.x_pressed = ButtonState::On;
            }
            Event::Button(ButtonEvent::XRepeated) => {
                state.lolly_weight_g -= 0.1;
            }
            Event::Button(ButtonEvent::XReleased) => {
                state.x_pressed = ButtonState::Off;
            }
            Event::Button(ButtonEvent::BPressed) => {
                state.saved_tared_scale_weight_g = state.scale_weight_g - state.tare_weight_g;
                state.b_pressed = ButtonState::On;
            }
            Event::Button(ButtonEvent::APressed) => {
                state.tare_weight_g = state.scale_weight_g;
                state.a_pressed = ButtonState::On;
            }
            Event::Button(ButtonEvent::BHeld(progress)) if round_f32(progress * 100.0) == 100 => {
                state.screen_shown = ScreenShown::SavingSettings;
                write_config_effect = Some(crate::WriteConfig(Config {
                    tare_weight_dg: round_f32(state.tare_weight_g * 10.0),
                    lolly_weight_dg: round_f32(state.lolly_weight_g * 10.0),
                    saved_tared_scale_weight: round_f32(state.saved_tared_scale_weight_g * 10.0),
                    scale_raw_50g: state.scale_raw_50g,
                    scale_raw_tare: state.scale_raw_tare,
                }));
            }
            Event::Button(ButtonEvent::BHeld(progress)) => {
                state.b_pressed = ButtonState::Held(progress)
            }
            Event::Button(ButtonEvent::AHeld(progress)) if round_f32(progress * 100.0) == 100 => {
                state.screen_shown = ScreenShown::Calibration(CalibrationState::Loading);
                enter_or_progress_calibration_mode_effect = Some(EnterOrProgressCalibrationMode);
            }
            Event::Button(ButtonEvent::AHeld(progress)) => {
                state.a_pressed = ButtonState::Held(progress)
            }
            Event::Button(ButtonEvent::BReleased) => state.b_pressed = ButtonState::Off,
            Event::Button(ButtonEvent::AReleased) => state.a_pressed = ButtonState::Off,
            Event::LoadCell(LoadCellEvent::WeightUpdate(w)) => {
                let prev_tared_scale_weight_g =
                    round_f32_dp(state.scale_weight_g - state.tare_weight_g, 1);
                let prev_lolly_count = round_f32(prev_tared_scale_weight_g / state.lolly_weight_g);

                state.scale_weight_g =
                    round_f32_dp(w.to_grams(state.scale_raw_tare, state.scale_raw_50g), 1);
                defmt::debug!("Calculated scale weight: {}", state.scale_weight_g);
                let tared_scale_weight_g =
                    round_f32_dp(state.scale_weight_g - state.tare_weight_g, 1);
                let lolly_count = round_f32(tared_scale_weight_g / state.lolly_weight_g);

                if lolly_count < prev_lolly_count {
                    led_timer_effect = Some(state.led_state.set_red());
                    backlight_timer_effect = Some(state.backlight_state.reset())
                }
                if lolly_count > prev_lolly_count {
                    led_timer_effect = Some(state.led_state.set_blue());
                    backlight_timer_effect = Some(state.backlight_state.reset())
                }
            }
            Event::LoadCell(LoadCellEvent::EnteredCalibMode) => {
                if matches!(
                    state.screen_shown,
                    ScreenShown::Calibration(CalibrationState::Loading)
                ) {
                    backlight_timer_effect = Some(state.backlight_state.reset());
                    state.screen_shown =
                        ScreenShown::Calibration(CalibrationState::WaitingConfirmation);
                };
            }
            Event::LoadCell(LoadCellEvent::CalibTareWeightUpdate(w)) => {
                match state.screen_shown {
                    ScreenShown::Calibration(CalibrationState::Loading) => {
                        state.screen_shown =
                            ScreenShown::Calibration(CalibrationState::CalibratingTare {
                                latest_tare_calib_value: w,
                            })
                    }
                    ScreenShown::Calibration(CalibrationState::CalibratingTare { .. }) => {
                        state.screen_shown =
                            ScreenShown::Calibration(CalibrationState::CalibratingTare {
                                latest_tare_calib_value: w,
                            })
                    }
                    _ => (),
                };
            }
            Event::LoadCell(LoadCellEvent::CalibTareWeightModeComplete) => {
                if let ScreenShown::Calibration(CalibrationState::CalibratingTare {
                    latest_tare_calib_value,
                }) = state.screen_shown
                {
                    state.screen_shown =
                        ScreenShown::Calibration(CalibrationState::TareCalibrated {
                            latest_tare_calib_value,
                        })
                };
                backlight_timer_effect = Some(state.backlight_state.reset());
            }
            Event::LoadCell(LoadCellEvent::Calib50gWeightUpdate(w)) => {
                if let ScreenShown::Calibration(CalibrationState::Calibrating50g {
                    latest_tare_calib_value,
                    ..
                }) = state.screen_shown
                {
                    state.screen_shown =
                        ScreenShown::Calibration(CalibrationState::Calibrating50g {
                            latest_tare_calib_value,
                            latest_50g_calib_value: w,
                        })
                };
            }
            Event::LoadCell(LoadCellEvent::CalibModeComplete) => {
                if let ScreenShown::Calibration(CalibrationState::Calibrating50g {
                    latest_tare_calib_value,
                    latest_50g_calib_value,
                }) = state.screen_shown
                {
                    state.screen_shown = ScreenShown::Calibration(CalibrationState::Calibrated {
                        latest_tare_calib_value,
                        latest_50g_calib_value,
                    })
                };
                backlight_timer_effect = Some(state.backlight_state.reset());
            }
            Event::Timer(TimerEvent::FadeoutLEDs { start_time }) => {
                led_timer_effect = state.led_state.handle_transition(start_time);
            }
            Event::Timer(TimerEvent::DimOrSleepDisplay { start_time }) => {
                backlight_timer_effect = state.backlight_state.handle_transition(start_time);
            }
            Event::BatteryMonitorUpdate(new_battery_state) => {
                state.battery_state = Some(new_battery_state)
            }
        };
        (
            write_config_effect,
            enter_or_progress_calibration_mode_effect,
            led_timer_effect,
            backlight_timer_effect,
        )
    }
}
