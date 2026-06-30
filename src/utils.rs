use core::{pin::Pin, task::Poll};
use pin_project::pin_project;

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
    if dp == 0 {
        return round_f32(x) as f32;
    }
    let factor = 10u32.pow(dp as u32) as f32;
    round_f32(x * factor) as f32 / factor
}

#[derive(Debug)]
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
// TODO: Consider triggering this at a specific time Instant instead of after a
// duration.
pub fn timer_future<T>(t: T, in_dur: embassy_time::Duration) -> TimerFuture<T> {
    TimerFuture {
        timer: embassy_time::Timer::after(in_dur),
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
