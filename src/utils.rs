use core::{pin::Pin, task::Poll};
use pin_project::pin_project;
use serde::{Deserialize, Serialize};

use crate::config_consts::scale_raw_1g_step;

/// Represents a raw adc value from the hx710.
#[derive(defmt::Format, Deserialize, Serialize, Default, PartialEq, Debug, Copy, Clone)]
pub struct ScaleRawWeight(pub f32);
impl ScaleRawWeight {
    pub const fn to_grams(
        self,
        scale_raw_tare: ScaleRawWeight,
        scale_raw_50g: ScaleRawWeight,
    ) -> f32 {
        (self.0 - scale_raw_tare.0) / scale_raw_1g_step(scale_raw_tare.0, scale_raw_50g.0)
    }
    pub const fn get_raw(self) -> f32 {
        self.0
    }
    pub const fn from_raw(raw: f32) -> Self {
        Self(raw)
    }
}

/// Implementation of f32::round in no_std environment.
pub const fn round_f32(x: f32) -> i32 {
    if x >= 0.0 {
        (x + 0.5) as i32
    } else {
        (x - 0.5) as i32
    }
}

/// Round f32 to x decimal places.
pub const fn round_f32_dp(x: f32, dp: u8) -> f32 {
    let factor = 10u32.pow(dp as u32) as f32;
    round_f32(x * factor) as f32 / factor
}

#[derive(Debug, defmt::Format)]
#[pin_project]
/// Struct for the [timer_future] method.
pub struct TimerFuture<T> {
    #[pin]
    timer: embassy_time::Timer,
    return_val: Option<T>,
}

impl<T> TimerFuture<T> {
    pub fn inspect_t(&self) -> Option<&T> {
        self.return_val.as_ref()
    }
}

/// Helper type representing a future that waits for duration and then returns T
/// - to allow it to be used in trait return type.
pub fn timer_future_in<T>(t: T, in_dur: embassy_time::Duration) -> TimerFuture<T> {
    TimerFuture {
        timer: embassy_time::Timer::after(in_dur),
        return_val: Some(t),
    }
}

/// Helper type representing a future that returns T at the specified instant.
/// - to allow it to be used in trait return type.
pub fn timer_future_at<T>(t: T, at: embassy_time::Instant) -> TimerFuture<T> {
    TimerFuture {
        timer: embassy_time::Timer::at(at),
        return_val: Some(t),
    }
}

impl<T> Future for TimerFuture<T> {
    type Output = T;
    fn poll(
        self: Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        let this = self.project();
        match this.timer.poll(cx) {
            // Panic safety: it's an error to poll this future if it's already returned Ready.
            Poll::Ready(_) => Poll::Ready(this.return_val.take().unwrap()),
            Poll::Pending => Poll::Pending,
        }
    }
}
