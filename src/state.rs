use crate::{candy_weigher_ui::DisplayState, DEFAULT_LOLLY_WEIGHT};
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
    pub fn next(self) -> Self {
        match self {
            MomentaryButtonState::Off => MomentaryButtonState::Off,
            MomentaryButtonState::PressedRecently { .. } => MomentaryButtonState::Off,
        }
    }
    pub async fn next_timer(&self, max_on_time: Duration) {
        match self {
            MomentaryButtonState::Off => core::future::pending().await,
            MomentaryButtonState::PressedRecently { on_at } => {
                let on_for = Instant::now() - *on_at;
                let rem_on = max_on_time.checked_sub(on_for).unwrap_or_default();
                Timer::after(rem_on).await;
            }
        }
    }
}

#[derive(Default)]
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
    pub async fn next_timer(&self, total_animation_duration: Duration) {
        match self {
            LedState::Off => core::future::pending().await,
            LedState::Red {
                total_steps,
                current_step_at,
                ..
            } => {
                let current_step_for = Instant::now() - *current_step_at;
                let max_step_length = total_animation_duration
                    .checked_div(*total_steps as u32)
                    .unwrap_or_default();
                let rem_current_step = max_step_length
                    .checked_sub(current_step_for)
                    .unwrap_or_default();
                Timer::after(rem_current_step).await
            }
            LedState::Blue {
                total_steps,
                current_step_at,
                ..
            } => {
                let current_step_for = Instant::now() - *current_step_at;
                let max_step_length = total_animation_duration
                    .checked_div(*total_steps as u32)
                    .unwrap_or_default();
                let rem_current_step = max_step_length
                    .checked_sub(current_step_for)
                    .unwrap_or_default();
                Timer::after(rem_current_step).await
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
}
