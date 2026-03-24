use crate::{
    candy_weigher_ui::DisplayState,
    config_consts::{
        DEFAULT_LOLLY_WEIGHT, LOW_BACKLIGHT_PERCENTAGE, MAX_LED_ON_TIME,
        MAX_MOMENTARY_BUTTON_ON_TIME, TIME_FROM_BACKLIGHT_LOW_TO_OFF, TIME_TO_BACKLIGHT_LOW,
        TOTAL_LED_FADEOUT_STEPS,
    },
    flash::{Config, FlashController},
    pimoroni_display_leds::{Percentage, PimoroniDisplayRgbLedController},
    Message, CORE1_SIGNAL,
};
use core::ops::Mul;
use defmt::debug;
use embassy_time::{Duration, Instant};

pub struct State {
    pub tare_weight_g: f32,
    pub scale_weight_g: f32,
    pub saved_tared_scale_weight_g: f32,
    pub lolly_weight_g: f32,
    pub t_l_pressed: MomentaryButtonState,
    pub b_l_pressed: MomentaryButtonState,
    pub t_r_pressed: MomentaryButtonState,
    pub b_r_pressed: MomentaryButtonState,
    pub led_state: LedState,
    pub backlight_state: DisplayBacklightState,
    pub last_display_state: Option<DisplayState>,
    pub last_led_state: Option<LedState>,
}

#[derive(Eq, PartialEq, Copy, Clone)]
pub enum DisplayBacklightState {
    Off,
    LowPower { on_at: Instant },
    On { on_at: Instant },
}

impl DisplayBacklightState {
    pub fn next(self) -> Self {
        match self {
            DisplayBacklightState::Off => DisplayBacklightState::Off,
            DisplayBacklightState::LowPower { .. } => DisplayBacklightState::Off,
            DisplayBacklightState::On { on_at } => DisplayBacklightState::LowPower { on_at },
        }
    }
    pub fn next_timer(
        &self,
        time_to_backlight_low: Duration,
        time_from_backlight_low_to_off: Option<Duration>,
    ) -> Option<Instant> {
        match self {
            DisplayBacklightState::Off => None,
            DisplayBacklightState::LowPower { on_at } => time_from_backlight_low_to_off.map(|t| {
                on_at
                    .saturating_add(time_to_backlight_low)
                    .saturating_add(t)
            }),
            DisplayBacklightState::On { on_at } => {
                Some(on_at.saturating_add(time_to_backlight_low))
            }
        }
    }
}

#[derive(Default)]
pub enum MomentaryButtonState {
    #[default]
    Off,
    PressedRecently {
        on_at: Instant,
    },
    Held,
}

impl MomentaryButtonState {
    pub fn next(&self) -> Self {
        match self {
            MomentaryButtonState::Off => MomentaryButtonState::Off,
            MomentaryButtonState::Held => MomentaryButtonState::Held,
            MomentaryButtonState::PressedRecently { .. } => MomentaryButtonState::Off,
        }
    }
    pub fn next_timer(&self, max_on_time: Duration) -> Option<Instant> {
        match self {
            MomentaryButtonState::Off | MomentaryButtonState::Held => None,
            MomentaryButtonState::PressedRecently { on_at } => {
                Some(on_at.saturating_add(max_on_time))
            }
        }
    }
}

#[derive(Copy, Clone, Default, PartialEq)]
pub enum LedState {
    #[default]
    Off,
    Red {
        total_steps: u16,
        current_step: u16,
        current_step_at: Instant,
    },
    Blue {
        total_steps: u16,
        current_step: u16,
        current_step_at: Instant,
    },
}

impl LedState {
    pub fn next(self) -> Self {
        match self {
            LedState::Off => LedState::Off,
            LedState::Red {
                total_steps,
                current_step,
                current_step_at,
            } if current_step + 1 >= total_steps => LedState::Off,
            LedState::Blue {
                total_steps,
                current_step,
                current_step_at,
            } if current_step + 1 >= total_steps => LedState::Off,
            LedState::Red {
                total_steps,
                current_step,
                ..
            } => LedState::Red {
                total_steps,
                current_step: current_step + 1,
                current_step_at: Instant::now(),
            },
            LedState::Blue {
                total_steps,
                current_step,
                ..
            } => LedState::Blue {
                total_steps,
                current_step: current_step + 1,
                current_step_at: Instant::now(),
            },
        }
    }
    pub fn next_timer(&self, total_animation_duration: Duration) -> Option<Instant> {
        match self {
            LedState::Off => None,
            LedState::Red {
                total_steps,
                current_step_at,
                ..
            }
            | LedState::Blue {
                total_steps,
                current_step_at,
                ..
            } => {
                let max_step_length = total_animation_duration
                    .checked_div(*total_steps as u32)
                    .unwrap_or_default();
                Some(current_step_at.saturating_add(max_step_length))
            }
        }
    }
}

impl Default for State {
    fn default() -> Self {
        Self {
            tare_weight_g: Default::default(),
            scale_weight_g: Default::default(),
            saved_tared_scale_weight_g: Default::default(),
            lolly_weight_g: DEFAULT_LOLLY_WEIGHT,
            t_l_pressed: Default::default(),
            b_l_pressed: Default::default(),
            t_r_pressed: Default::default(),
            b_r_pressed: Default::default(),
            led_state: Default::default(),
            last_display_state: Default::default(),
            last_led_state: Default::default(),
            backlight_state: DisplayBacklightState::On {
                on_at: Instant::now(),
            },
        }
    }
}

impl State {
    pub fn to_display_state(&self) -> DisplayState {
        // Round off to 1 d.p (prevent overdrawing to display)
        let tared_scale_weight_g =
            round_f32((self.scale_weight_g - self.tare_weight_g).mul(10.0)) as f32 / 10.0;
        let lolly_count = round_f32(tared_scale_weight_g / self.lolly_weight_g)
            .try_into()
            .unwrap();
        let prev_lolly_count = round_f32(self.saved_tared_scale_weight_g / self.lolly_weight_g);
        DisplayState {
            scale_weight_g: self.scale_weight_g - self.tare_weight_g,
            lolly_weight_g: self.lolly_weight_g,
            lolly_count,
            lolly_count_change: lolly_count as i32 - prev_lolly_count,
            t_l_pressed: matches!(
                self.t_l_pressed,
                MomentaryButtonState::PressedRecently { .. } | MomentaryButtonState::Held
            ),
            t_r_pressed: matches!(
                self.t_r_pressed,
                MomentaryButtonState::PressedRecently { .. } | MomentaryButtonState::Held
            ),
            b_l_pressed: matches!(
                self.b_l_pressed,
                MomentaryButtonState::PressedRecently { .. } | MomentaryButtonState::Held
            ),
            b_r_pressed: matches!(
                self.b_r_pressed,
                MomentaryButtonState::PressedRecently { .. } | MomentaryButtonState::Held
            ),
            backlight_state: self.backlight_state,
        }
    }
    pub fn get_next_transitions(
        &self,
    ) -> Option<(
        Instant,
        impl Iterator<Item = for<'a> fn(&'a mut Self)> + use<>,
    )> {
        let transitions = self.get_transitions();
        let min_duration = transitions
            .flatten()
            .min_by_key(|(duration, _)| *duration)
            .map(|(duration, _)| duration);
        min_duration.map(move |min_duration| {
            (
                min_duration,
                self.get_transitions()
                    .flatten()
                    .filter(move |(duration, _)| *duration == min_duration)
                    .map(|(_, transition)| transition),
            )
        })
    }
    fn get_transitions(
        &self,
    ) -> impl Iterator<Item = Option<(Instant, for<'a> fn(&'a mut Self))>> + use<> {
        [
            self.backlight_state
                .next_timer(TIME_TO_BACKLIGHT_LOW, TIME_FROM_BACKLIGHT_LOW_TO_OFF)
                .map(|t| {
                    (
                        t,
                        (|this: &mut Self| this.backlight_state = this.backlight_state.next())
                            as for<'a> fn(&'a mut Self),
                    )
                }),
            self.t_l_pressed
                .next_timer(MAX_MOMENTARY_BUTTON_ON_TIME)
                .map(|t| {
                    (
                        t,
                        (|this: &mut Self| this.t_l_pressed = this.t_l_pressed.next())
                            as for<'a> fn(&'a mut Self),
                    )
                }),
            self.t_r_pressed
                .next_timer(MAX_MOMENTARY_BUTTON_ON_TIME)
                .map(|t| {
                    (
                        t,
                        (|this: &mut Self| this.t_r_pressed = this.t_r_pressed.next())
                            as for<'a> fn(&'a mut Self),
                    )
                }),
            self.b_l_pressed
                .next_timer(MAX_MOMENTARY_BUTTON_ON_TIME)
                .map(|t| {
                    (
                        t,
                        (|this: &mut Self| this.b_l_pressed = this.b_l_pressed.next())
                            as for<'a> fn(&'a mut Self),
                    )
                }),
            self.b_r_pressed
                .next_timer(MAX_MOMENTARY_BUTTON_ON_TIME)
                .map(|t| {
                    (
                        t,
                        (|this: &mut Self| this.b_r_pressed = this.b_r_pressed.next())
                            as for<'a> fn(&'a mut Self),
                    )
                }),
            self.led_state.next_timer(MAX_LED_ON_TIME).map(|t| {
                (
                    t,
                    (|this: &mut Self| this.led_state = this.led_state.next())
                        as for<'a> fn(&'a mut Self),
                )
            }),
        ]
        .into_iter()
    }
    pub fn handle_message(
        &mut self,
        flash_controller: &mut FlashController<'_, { crate::FLASH_STORAGE_OFFSET_BYTES }>,
        message: Message,
    ) {
        debug!("About to handle message: {}", message);
        match message {
            Message::ButtonAHeld => {
                self.t_l_pressed = MomentaryButtonState::Held;
                self.backlight_state = DisplayBacklightState::On {
                    on_at: Instant::now(),
                };
            }
            Message::ButtonBHeld => {
                self.b_l_pressed = MomentaryButtonState::Held;
                self.backlight_state = DisplayBacklightState::On {
                    on_at: Instant::now(),
                };
            }
            Message::ButtonAHoldCancelled => {
                self.t_l_pressed = MomentaryButtonState::Off;
                self.backlight_state = DisplayBacklightState::On {
                    on_at: Instant::now(),
                };
            }
            Message::ButtonBHoldCancelled => {
                self.b_l_pressed = MomentaryButtonState::Off;
                self.backlight_state = DisplayBacklightState::On {
                    on_at: Instant::now(),
                };
            }
            Message::ButtonAPressed => {
                self.lolly_weight_g += 0.1;
                if matches!(self.t_l_pressed, MomentaryButtonState::Off) {
                    self.t_l_pressed = MomentaryButtonState::PressedRecently {
                        on_at: Instant::now(),
                    };
                }
                self.backlight_state = DisplayBacklightState::On {
                    on_at: Instant::now(),
                };
            }
            Message::ButtonBPressed => {
                self.lolly_weight_g -= 0.1;
                if matches!(self.b_l_pressed, MomentaryButtonState::Off) {
                    self.b_l_pressed = MomentaryButtonState::PressedRecently {
                        on_at: Instant::now(),
                    };
                }
                self.backlight_state = DisplayBacklightState::On {
                    on_at: Instant::now(),
                };
            }
            Message::ButtonXPressed => {
                self.saved_tared_scale_weight_g = self.scale_weight_g - self.tare_weight_g;
                self.t_r_pressed = MomentaryButtonState::PressedRecently {
                    on_at: Instant::now(),
                };
                self.backlight_state = DisplayBacklightState::On {
                    on_at: Instant::now(),
                };
            }
            Message::ButtonYPressed => {
                self.tare_weight_g = self.scale_weight_g;
                self.b_r_pressed = MomentaryButtonState::PressedRecently {
                    on_at: Instant::now(),
                };
                self.backlight_state = DisplayBacklightState::On {
                    on_at: Instant::now(),
                };
            }
            Message::ButtonXHeld => flash_controller.write::<_, 4096>(&Config {
                tare_weight_g: self.tare_weight_g,
                lolly_weight_g: self.lolly_weight_g,
            }),
            Message::ButtonYHeld => (),
            Message::ButtonXReleased => (),
            Message::ButtonYReleased => (),
            Message::WeightUpdate(w) => {
                let prev_tared_scale_weight_g =
                    round_f32((self.scale_weight_g - self.tare_weight_g).mul(10.0)) as f32 / 10.0;
                let prev_lolly_count = round_f32(prev_tared_scale_weight_g / self.lolly_weight_g);

                self.scale_weight_g = w;
                let tared_scale_weight_g =
                    round_f32((self.scale_weight_g - self.tare_weight_g).mul(10.0)) as f32 / 10.0;
                let lolly_count = round_f32(tared_scale_weight_g / self.lolly_weight_g);

                if lolly_count < prev_lolly_count {
                    self.led_state = LedState::Red {
                        total_steps: TOTAL_LED_FADEOUT_STEPS,
                        current_step: 0,
                        current_step_at: Instant::now(),
                    };
                    self.backlight_state = DisplayBacklightState::On {
                        on_at: Instant::now(),
                    };
                }
                if lolly_count > prev_lolly_count {
                    self.led_state = LedState::Blue {
                        total_steps: TOTAL_LED_FADEOUT_STEPS,
                        current_step: 0,
                        current_step_at: Instant::now(),
                    };
                    self.backlight_state = DisplayBacklightState::On {
                        on_at: Instant::now(),
                    };
                }
            }
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
    if state.last_display_state.as_ref() != Some(&next_display_state) {
        CORE1_SIGNAL.signal(next_display_state.clone());
        state.last_display_state = Some(next_display_state);
    }
}

/// Implementation of f32::round in no_std environment.
fn round_f32(x: f32) -> i32 {
    if x >= 0.0 {
        (x + 0.5) as i32
    } else {
        (x - 0.5) as i32
    }
}
