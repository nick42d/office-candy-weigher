use crate::config_consts::{
    BUTTON_SEMICIRCLE_COLOUR, BUTTON_SEMICIRCLE_HELD_COLOUR, BUTTON_TOOLTIP_COLOUR,
    SEMICIRCLE_DIAMETER,
};
use crate::hardware_controllers::pimoroni_display::{DISPLAY_H, DISPLAY_W};
use crate::state::{ButtonState, CalibrationState};
use crate::utils::round_f32;
use core::fmt::Write;
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::mono_font::ascii::FONT_10X20;
use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{Circle, PrimitiveStyle};
use embedded_graphics::text::Text;

#[derive(PartialEq, Clone)]
pub enum DisplayState {
    MainScreen {
        scale_weight_g: f32,
        lolly_weight_g: f32,
        lolly_count: u32,
        lolly_count_change: i32,
        t_l_state: ButtonState,
        b_l_state: ButtonState,
        t_r_state: ButtonState,
        b_r_state: ButtonState,
    },
    CalibrationScreen(CalibrationState),
    SavingSettingsScreen,
}

pub fn draw<D>(state: &DisplayState, display: &mut D)
where
    D: DrawTarget<Color = Rgb565>,
    <D as embedded_graphics::draw_target::DrawTarget>::Error: core::fmt::Debug,
{
    match *state {
        DisplayState::MainScreen {
            scale_weight_g,
            lolly_weight_g,
            lolly_count,
            lolly_count_change,
            t_l_state,
            b_l_state,
            t_r_state,
            b_r_state,
        } => draw_main_screen(
            scale_weight_g,
            lolly_weight_g,
            lolly_count,
            lolly_count_change,
            t_l_state,
            b_l_state,
            t_r_state,
            b_r_state,
            display,
        ),
        DisplayState::CalibrationScreen(state) => draw_calibration_screen(state, display),
        DisplayState::SavingSettingsScreen => draw_saving_settings_screen(display),
    }
}

pub fn draw_calibration_screen<D>(state: CalibrationState, display: &mut D)
where
    D: DrawTarget<Color = Rgb565>,
    <D as embedded_graphics::draw_target::DrawTarget>::Error: core::fmt::Debug,
{
    display.clear(Rgb565::BLACK).unwrap();
    let text_style = embedded_graphics::mono_font::MonoTextStyleBuilder::new()
        .text_color(Rgb565::GREEN)
        .font(&FONT_10X20)
        .build();
    let calib_value_style = eg_seven_segment::SevenSegmentStyleBuilder::new()
        .digit_size(Size {
            width: 10,
            height: 20,
        })
        .segment_color(Rgb565::GREEN)
        .build();
    match state {
        CalibrationState::TareCalibrated {
            latest_tare_calib_value,
        } => {
            Text::new(
                "Tare callibration complete. Place 50g weight on scale and press x to continue calibration.",
                Point::new(10, 30),
                text_style,
            )
            .draw(display)
            .unwrap();
            Text::new("Raw Tare", Point::new(10, 30 + 22), text_style)
                .draw(display)
                .unwrap();
            // Max value is 2_147_483_647 (10 digits), add extra char for
            // minus sign.
            let mut raw_tare_str = heapless::String::<11>::new();
            core::write!(&mut raw_tare_str, "{}", latest_tare_calib_value.0 as i32).unwrap();
            Text::new(
                &raw_tare_str,
                // Length of "Raw Tare" + 1 char padding
                Point::new(8 * 10 + 10, 30 + 22),
                calib_value_style,
            )
            .draw(display)
            .unwrap();
        }
        CalibrationState::CalibratingTare {
            latest_tare_calib_value,
        } => {
            Text::new(
                "Calibrating tare in progress.",
                Point::new(10, 30),
                text_style,
            )
            .draw(display)
            .unwrap();
            Text::new("Raw Tare", Point::new(10, 30 + 22), text_style)
                .draw(display)
                .unwrap();
            // Max value is 2_147_483_647 (10 digits), add extra char for
            // minus sign.
            let mut raw_tare_str = heapless::String::<11>::new();
            core::write!(&mut raw_tare_str, "{}", latest_tare_calib_value.0 as i32).unwrap();
            Text::new(
                &raw_tare_str,
                // Length of "Raw Tare" + 1 char padding
                Point::new(8 * 10 + 10, 30 + 22),
                calib_value_style,
            )
            .draw(display)
            .unwrap();
        }
        CalibrationState::Calibrating50g {
            latest_tare_calib_value,
            latest_50g_calib_value,
        } => {
            Text::new(
                "Calibrating with 50g weight in progress.",
                Point::new(10, 30),
                text_style,
            )
            .draw(display)
            .unwrap();
            Text::new("Raw Tare", Point::new(10, 30 + 22), text_style)
                .draw(display)
                .unwrap();
            Text::new("Raw 50g", Point::new(10, 30 + 22 * 2), text_style)
                .draw(display)
                .unwrap();
            // Max value is 2_147_483_647 (10 digits), add extra char for
            // minus sign.
            let mut raw_tare_str = heapless::String::<11>::new();
            let mut raw_50g_str = heapless::String::<11>::new();
            core::write!(&mut raw_tare_str, "{}", latest_tare_calib_value.0 as i32).unwrap();
            core::write!(&mut raw_50g_str, "{}", latest_50g_calib_value.0 as i32).unwrap();
            Text::new(
                &raw_tare_str,
                // Length of "Raw Tare" + 1 char padding
                Point::new(8 * 10 + 10, 30 + 22),
                calib_value_style,
            )
            .draw(display)
            .unwrap();
            Text::new(
                &raw_50g_str,
                // Length of "Raw Tare" + 1 char padding
                Point::new(8 * 10 + 10, 30 + 22 * 2),
                calib_value_style,
            )
            .draw(display)
            .unwrap();
        }
        CalibrationState::WaitingConfirmation => {
            Text::new(
                "Remove all weight from the scale and press x to commence calibration.",
                Point::new(10, 30),
                text_style,
            )
            .draw(display)
            .unwrap();
        }
        CalibrationState::Calibrated {
            latest_tare_calib_value,
            latest_50g_calib_value,
        } => {
            Text::new(
                "Calibration complete. Press x to apply.",
                Point::new(10, 30),
                text_style,
            )
            .draw(display)
            .unwrap();
            Text::new("Raw Tare", Point::new(10, 30 + 22), text_style)
                .draw(display)
                .unwrap();
            Text::new("Raw 50g", Point::new(10, 30 + 22 * 2), text_style)
                .draw(display)
                .unwrap();
            // Max value is 2_147_483_647 (10 digits), add extra char for
            // minus sign.
            let mut raw_tare_str = heapless::String::<11>::new();
            let mut raw_50g_str = heapless::String::<11>::new();
            core::write!(&mut raw_tare_str, "{}", latest_tare_calib_value.0 as i32).unwrap();
            core::write!(&mut raw_50g_str, "{}", latest_50g_calib_value.0 as i32).unwrap();
            Text::new(
                &raw_tare_str,
                // Length of "Raw Tare" + 1 char padding
                Point::new(8 * 10 + 10, 30 + 22),
                calib_value_style,
            )
            .draw(display)
            .unwrap();
            Text::new(
                &raw_50g_str,
                // Length of "Raw Tare" + 1 char padding
                Point::new(8 * 10 + 10, 30 + 22 * 2),
                calib_value_style,
            )
            .draw(display)
            .unwrap();
        }
        CalibrationState::Loading => {
            Text::new("Loading...", Point::new(10, 30), text_style)
                .draw(display)
                .unwrap();
        }
    }
}

pub fn draw_saving_settings_screen<D>(display: &mut D)
where
    D: DrawTarget<Color = Rgb565>,
    <D as embedded_graphics::draw_target::DrawTarget>::Error: core::fmt::Debug,
{
    display.clear(Rgb565::BLACK).unwrap();
    let text_calibration_value = Text::new(
        "Settings saved - press X to continue",
        Point::new(10, 90),
        embedded_graphics::mono_font::MonoTextStyleBuilder::new()
            .text_color(Rgb565::GREEN)
            .font(&FONT_10X20)
            .build(),
    );
    text_calibration_value.draw(display).unwrap();
}

pub fn draw_main_screen<D>(
    scale_weight_g: f32,
    lolly_weight_g: f32,
    lolly_count: u32,
    lolly_count_change: i32,
    t_l_state: ButtonState,
    b_l_state: ButtonState,
    t_r_state: ButtonState,
    b_r_state: ButtonState,
    display: &mut D,
) where
    D: DrawTarget<Color = Rgb565>,
    <D as embedded_graphics::draw_target::DrawTarget>::Error: core::fmt::Debug,
{
    let weight_text_style = MonoTextStyle::new(&FONT_10X20, Rgb565::BLUE);
    let mut scale_weight_str = heapless::String::<30>::new();
    let mut lolly_weight_str = heapless::String::<30>::new();
    let mut lolly_count_str = heapless::String::<10>::new();
    let mut lolly_count_change_str = heapless::String::<30>::new();
    core::write!(&mut scale_weight_str, "W-Scale: {:.1}g", scale_weight_g).unwrap();
    core::write!(&mut lolly_weight_str, "W-Lolly: {:.1}g", lolly_weight_g).unwrap();
    core::write!(&mut lolly_count_str, "{}", lolly_count).unwrap();
    if lolly_count_change >= 0 {
        core::write!(&mut lolly_count_change_str, "+{}", lolly_count_change)
    } else {
        core::write!(&mut lolly_count_change_str, "{}", lolly_count_change)
    }
    .unwrap();
    let text_scale_weight = Text::new(&scale_weight_str, Point::new(40, 22), weight_text_style);
    let text_lolly_weight = Text::new(
        &lolly_weight_str,
        Point::new(40, DISPLAY_H as i32 - 5),
        weight_text_style,
    );
    let text_lolly_count = Text::new(
        &lolly_count_str,
        Point::new(10, 90),
        eg_seven_segment::SevenSegmentStyleBuilder::new()
            .digit_size(Size {
                width: 30,
                height: 50,
            })
            .segment_color(Rgb565::GREEN)
            .build(),
    );
    let text_lolly_change = Text::new(
        &lolly_count_change_str,
        Point::new(160, 70),
        weight_text_style,
    );

    display.clear(Rgb565::BLACK).unwrap();

    draw_corner_button(ButtonPos::TopLeft, "+", t_l_state, display).unwrap();
    draw_corner_button(ButtonPos::BottomLeft, "-", b_l_state, display).unwrap();
    draw_corner_button(ButtonPos::TopRight, "R", t_r_state, display).unwrap();
    draw_corner_button(ButtonPos::BottomRight, "T", b_r_state, display).unwrap();
    text_scale_weight.draw(display).unwrap();
    text_lolly_weight.draw(display).unwrap();
    text_lolly_count.draw(display).unwrap();
    text_lolly_change.draw(display).unwrap();
}

enum ButtonPos {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

fn draw_corner_button<D>(
    pos: ButtonPos,
    text: &str,
    status: ButtonState,
    display: &mut D,
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Rgb565>,
    <D as embedded_graphics::draw_target::DrawTarget>::Error: core::fmt::Debug,
{
    let circle_style = PrimitiveStyle::with_fill(BUTTON_SEMICIRCLE_COLOUR);
    let circle_held_style = PrimitiveStyle::with_fill(BUTTON_SEMICIRCLE_HELD_COLOUR);
    let outline_style = PrimitiveStyle::with_stroke(BUTTON_SEMICIRCLE_COLOUR, 2);
    let button_tooltip_font = FONT_10X20;
    let button_tooltip_font_w = button_tooltip_font.character_size.width;
    let _button_tooltip_font_h = button_tooltip_font.character_size.height;
    let button_text_style = MonoTextStyle::new(&button_tooltip_font, BUTTON_TOOLTIP_COLOUR);
    let char_pos = match pos {
        // 11 is a magic number that makes the char render in a good spot...
        ButtonPos::TopLeft => Point::new(1, 11),
        ButtonPos::TopRight =>
        // 13 is a magic number that makes the char render in a good spot...
        {
            Point::new(
                (DISPLAY_W as u32)
                    .saturating_sub(1)
                    .saturating_sub(button_tooltip_font_w)
                    .try_into()
                    .unwrap_or_default(),
                13,
            )
        }
        ButtonPos::BottomLeft => Point::new(1, DISPLAY_H as i32 - 1),
        ButtonPos::BottomRight => Point::new(
            (DISPLAY_W as u32)
                .saturating_sub(1)
                .saturating_sub(button_tooltip_font_w)
                .try_into()
                .unwrap_or_default(),
            DISPLAY_H as i32 - 2,
        ),
    };
    let circle_pos = match pos {
        ButtonPos::TopLeft => Point::new(0, 0),
        ButtonPos::TopRight => Point::new(DISPLAY_W as i32, 0),
        ButtonPos::BottomLeft => Point::new(0, DISPLAY_H as i32),
        ButtonPos::BottomRight => Point::new(DISPLAY_W as i32, DISPLAY_H as i32),
    };
    match status {
        ButtonState::Off => Circle::with_center(circle_pos, SEMICIRCLE_DIAMETER)
            .into_styled(outline_style)
            .draw(display)?,
        ButtonState::On => Circle::with_center(circle_pos, SEMICIRCLE_DIAMETER)
            .into_styled(circle_style)
            .draw(display)?,
        ButtonState::Held(percentage) => {
            Circle::with_center(circle_pos, SEMICIRCLE_DIAMETER)
                .into_styled(circle_style)
                .draw(display)?;
            Circle::with_center(
                circle_pos,
                round_f32(SEMICIRCLE_DIAMETER as f32 * percentage)
                    .try_into()
                    .unwrap_or_default(),
            )
            .into_styled(circle_held_style)
            .draw(display)?;
        }
    }
    Text::with_alignment(
        text,
        char_pos,
        button_text_style,
        embedded_graphics::text::Alignment::Left,
    )
    .draw(display)?;
    Ok(())
}
