use crate::{
    candy_weigher_ui::{self, draw, DisplayState},
    config_consts::{
        DEFAULT_LOLLY_WEIGHT, MAX_LED_ON_TIME, MAX_MOMENTARY_BUTTON_ON_TIME,
        TOTAL_LED_FADEOUT_STEPS,
    },
    pimoroni_display::PimoroniDisplayController,
    pimoroni_display_leds::{Percentage, PimoroniDisplayRgbLedController},
    Message,
};
use defmt::debug;
use embassy_time::{Duration, Instant, Timer};

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
    pub last_updated: Instant,
    pub last_display_state: Option<DisplayState>,
    pub last_led_state: Option<LedState>,
}

#[derive(Default)]
pub enum MomentaryButtonState {
    #[default]
    Off,
    PressedRecently {
        on_at: Instant,
    },
}

impl MomentaryButtonState {
    pub fn next(&self) -> Self {
        match self {
            MomentaryButtonState::Off => MomentaryButtonState::Off,
            MomentaryButtonState::PressedRecently { .. } => MomentaryButtonState::Off,
        }
    }
    pub fn next_timer(&self, max_on_time: Duration) -> Option<Instant> {
        match self {
            MomentaryButtonState::Off => None,
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
            last_updated: Instant::now(),
            last_display_state: Default::default(),
            last_led_state: Default::default(),
        }
    }
}

impl State {
    pub fn to_display_state(&self) -> DisplayState {
        // Addition of 0.5 is a neat hack to round positive float to integer.
        let tared_scale_weight_g = self.scale_weight_g - self.tare_weight_g;
        let lolly_count = (tared_scale_weight_g / self.lolly_weight_g + 0.5) as u32;
        let prev_lolly_count = (self.saved_tared_scale_weight_g / self.lolly_weight_g + 0.5) as u32;
        DisplayState {
            scale_weight_g: self.scale_weight_g - self.tare_weight_g,
            lolly_weight_g: self.lolly_weight_g,
            lolly_count,
            lolly_count_change: lolly_count as i32 - prev_lolly_count as i32,
            t_l_pressed: matches!(
                self.t_l_pressed,
                MomentaryButtonState::PressedRecently { .. }
            ),
            t_r_pressed: matches!(
                self.t_r_pressed,
                MomentaryButtonState::PressedRecently { .. }
            ),
            b_l_pressed: matches!(
                self.b_l_pressed,
                MomentaryButtonState::PressedRecently { .. }
            ),
            b_r_pressed: matches!(
                self.b_r_pressed,
                MomentaryButtonState::PressedRecently { .. }
            ),
        }
    }
    pub fn get_transitions(&self) -> [Option<(Instant, for<'a> fn(&'a mut Self))>; 5] {
        [
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
    }
    pub fn handle_message(&mut self, message: Message) {
        debug!("About to handle message: {}", message);
        match message {
            Message::ButtonAPressed => {
                self.lolly_weight_g += 0.1;
                self.t_l_pressed = MomentaryButtonState::PressedRecently {
                    on_at: Instant::now(),
                };
            }
            Message::ButtonBPressed => {
                self.lolly_weight_g -= 0.1;
                self.b_l_pressed = MomentaryButtonState::PressedRecently {
                    on_at: Instant::now(),
                };
            }
            Message::ButtonXPressed => {
                self.saved_tared_scale_weight_g = self.scale_weight_g - self.tare_weight_g;
                self.t_r_pressed = MomentaryButtonState::PressedRecently {
                    on_at: Instant::now(),
                };
            }
            Message::ButtonYPressed => {
                self.tare_weight_g = self.scale_weight_g;
                self.b_r_pressed = MomentaryButtonState::PressedRecently {
                    on_at: Instant::now(),
                };
            }
            Message::WeightUpdate(w) => {
                if w < self.scale_weight_g {
                    self.led_state = LedState::Red {
                        total_steps: TOTAL_LED_FADEOUT_STEPS,
                        current_step: 0,
                        current_step_at: Instant::now(),
                    };
                }
                if w > self.scale_weight_g {
                    self.led_state = LedState::Blue {
                        total_steps: TOTAL_LED_FADEOUT_STEPS,
                        current_step: 0,
                        current_step_at: Instant::now(),
                    };
                }
                self.scale_weight_g = w;
            }
        }
    }
}

pub fn output_state(
    state: &mut State,
    display_controller: &mut PimoroniDisplayController,
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
        display_controller
            .draw_via_framebuffer(|display| candy_weigher_ui::draw(&next_display_state, display));
        state.last_display_state = Some(next_display_state);
    }
}
