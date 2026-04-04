use embassy_sync::{blocking_mutex::raw::ThreadModeRawMutex, signal::Signal};

pub use flash::FlashController;
pub use pimoroni_display::PimoroniDisplayController;
pub use pimoroni_display_leds::PimoroniDisplayRgbLedController;

pub mod flash;
pub mod pimoroni_display;
pub mod pimoroni_display_leds;

static HX710_CONTROLLER_SIGNAL: Signal<ThreadModeRawMutex, ()> = Signal::new();

pub struct HX710Controller;
impl HX710Controller {
    pub fn enter_calibration_mode(&self) {
        HX710_CONTROLLER_SIGNAL.signal(());
    }
    pub fn get_signal(&self) -> &'static Signal<ThreadModeRawMutex, ()> {
        &HX710_CONTROLLER_SIGNAL
    }
}
