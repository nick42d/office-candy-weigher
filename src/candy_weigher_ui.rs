use crate::config_consts::{BUTTON_SEMICIRCLE_COLOUR, BUTTON_TOOLTIP_COLOUR, SEMICIRCLE_DIAMETER};
use crate::hardware_controllers::pimoroni_display::{DISPLAY_H, DISPLAY_W};
use crate::state::CalibrationState;
use core::fmt::Write;
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::mono_font::ascii::FONT_10X20;
use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{Arc, Circle, PrimitiveStyle, StyledDrawable};
use embedded_graphics::text::Text;

#[derive(PartialEq, Clone)]
pub enum DisplayState {
    MainScreen {
        scale_weight_g: f32,
        lolly_weight_g: f32,
        lolly_count: u32,
        lolly_count_change: i32,
        t_l_pressed: bool,
        b_l_pressed: bool,
        t_r_pressed: bool,
        b_r_pressed: bool,
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
            t_l_pressed,
            b_l_pressed,
            t_r_pressed,
            b_r_pressed,
        } => draw_main_screen(
            scale_weight_g,
            lolly_weight_g,
            lolly_count,
            lolly_count_change,
            t_l_pressed,
            b_l_pressed,
            t_r_pressed,
            b_r_pressed,
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
    match state {
        CalibrationState::TareCalibrated {
            latest_tare_calib_value,
        } => {
            Text::new(
                "Tare callibration complete <Results tbc>. Place 50g weight on scale and press x to continue calibration.",
                Point::new(10, 90),
                text_style,
            )
            .draw(display)
            .unwrap();
        }
        CalibrationState::CalibratingTare {
            latest_tare_calib_value,
        } => {
            Text::new("CalibratingTareMessageTBC", Point::new(10, 90), text_style)
                .draw(display)
                .unwrap();
        }
        CalibrationState::Calibrating50g {
            latest_tare_calib_value,
            latest_50g_calib_value,
        } => {
            Text::new("Calibrating25gMessageTBC", Point::new(10, 90), text_style)
                .draw(display)
                .unwrap();
        }
        CalibrationState::WaitingConfirmation => {
            Text::new(
                "Remove all weight from the scale and press x to commence calibration.",
                Point::new(10, 90),
                text_style,
            )
            .draw(display)
            .unwrap();
        }
        CalibrationState::Calibrated => {
            Text::new(
                "Calibration complete <Results tbc>. Press x to save.",
                Point::new(10, 90),
                text_style,
            )
            .draw(display)
            .unwrap();
        }
        CalibrationState::Loading => {
            Text::new("Loading...", Point::new(10, 90), text_style)
                .draw(display)
                .unwrap();
        }
    }
    // if let Some(calibration_value) = calibration_value {
    //     // Max value is 2_147_483_647 (10 digits), add extra char for minus
    // sign.     let mut calibration_value_str =
    // heapless::String::<11>::new();     core::write!(&mut
    // calibration_value_str, "{}", calibration_value as i32).unwrap();
    //     let text_calibration_value = Text::new(
    //         &calibration_value_str,
    //         Point::new(10, 90),
    //         eg_seven_segment::SevenSegmentStyleBuilder::new()
    //             .digit_size(Size {
    //                 width: 30,
    //                 height: 50,
    //             })
    //             .segment_color(Rgb565::GREEN)
    //             .build(),
    //     );
    //     text_calibration_value.draw(display).unwrap();
    // } else {
    //     let text_calibration_value = Text::new(
    //         "Calibrating...",
    //         Point::new(10, 90),
    //         embedded_graphics::mono_font::MonoTextStyleBuilder::new()
    //             .text_color(Rgb565::GREEN)
    //             .font(&FONT_10X20)
    //             .build(),
    //     );
    //     text_calibration_value.draw(display).unwrap();
    // }
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
    t_l_pressed: bool,
    b_l_pressed: bool,
    t_r_pressed: bool,
    b_r_pressed: bool,
    display: &mut D,
) where
    D: DrawTarget<Color = Rgb565>,
    <D as embedded_graphics::draw_target::DrawTarget>::Error: core::fmt::Debug,
{
    let circle_style = PrimitiveStyle::with_fill(BUTTON_SEMICIRCLE_COLOUR);
    let arc_style = PrimitiveStyle::with_stroke(BUTTON_SEMICIRCLE_COLOUR, 2);
    let arc_t_l = Arc::with_center(Point::new(0, 0), SEMICIRCLE_DIAMETER, 0.0.deg(), 90.0.deg())
        .into_styled(arc_style);
    let arc_b_l = Arc::with_center(
        Point::new(0, DISPLAY_H as i32),
        SEMICIRCLE_DIAMETER,
        270.0.deg(),
        90.0.deg(),
    )
    .into_styled(arc_style);
    let arc_t_r = Arc::with_center(
        Point::new(DISPLAY_W as i32, 0),
        SEMICIRCLE_DIAMETER,
        90.0.deg(),
        90.0.deg(),
    )
    .into_styled(arc_style);
    let arc_b_r = Arc::with_center(
        Point::new(DISPLAY_W as i32, DISPLAY_H as i32),
        SEMICIRCLE_DIAMETER,
        180.0.deg(),
        90.0.deg(),
    )
    .into_styled(arc_style);
    let circle_t_l =
        Circle::with_center(Point::new(0, 0), SEMICIRCLE_DIAMETER).into_styled(circle_style);
    // Experiment in half size circle
    let circle_b_l = Circle::with_center(Point::new(0, DISPLAY_H as i32), SEMICIRCLE_DIAMETER / 2)
        .into_styled(circle_style);
    let circle_t_r = Circle::with_center(Point::new(DISPLAY_W as i32, 0), SEMICIRCLE_DIAMETER)
        .into_styled(circle_style);
    let circle_b_r = Circle::with_center(
        Point::new(DISPLAY_W as i32, DISPLAY_H as i32),
        SEMICIRCLE_DIAMETER,
    )
    .into_styled(circle_style);
    let button_tooltip_font = FONT_10X20;
    let _button_tooltip_font_w = button_tooltip_font.character_size.width;
    let _button_tooltip_font_h = button_tooltip_font.character_size.height;
    let button_text_style = MonoTextStyle::new(&button_tooltip_font, BUTTON_TOOLTIP_COLOUR);
    let text_t_l = Text::new(
        "+",
        // 11 is a magic number that makes the plus render in a good spot...
        Point::new(1, 11),
        button_text_style,
    );
    let text_b_l = Text::new("-", Point::new(1, DISPLAY_H as i32 - 1), button_text_style);
    let text_t_r = Text::with_alignment(
        "R",
        // 11 is a magic number that makes the R render in a good spot...
        Point::new(
            (DISPLAY_W as u32)
                .saturating_sub(1)
                .try_into()
                .unwrap_or_default(),
            13,
        ),
        button_text_style,
        embedded_graphics::text::Alignment::Right,
    );
    let text_b_r = Text::with_alignment(
        "T",
        Point::new(
            (DISPLAY_W as u32)
                .saturating_sub(1)
                .try_into()
                .unwrap_or_default(),
            DISPLAY_H as i32 - 2,
        ),
        button_text_style,
        embedded_graphics::text::Alignment::Right,
    );
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

    arc_b_l.draw(display).unwrap();

    if t_l_pressed {
        circle_t_l.draw(display).unwrap();
    } else {
        arc_t_l.draw(display).unwrap();
    };
    if t_r_pressed {
        circle_t_r.draw(display).unwrap();
    } else {
        arc_t_r.draw(display).unwrap();
    };
    if b_l_pressed {
        circle_b_l.draw(display).unwrap();
    }
    if b_r_pressed {
        circle_b_r.draw(display).unwrap();
    } else {
        arc_b_r.draw(display).unwrap();
    };
    text_t_l.draw(display).unwrap();
    text_b_l.draw(display).unwrap();
    text_t_r.draw(display).unwrap();
    text_b_r.draw(display).unwrap();
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

enum ButtonStatus {
    Off,
    On,
    Held { percentage: f32 },
}

fn draw_corner_button<D>(pos: ButtonPos, text: char, status: ButtonStatus, display: &mut D)
where
    D: DrawTarget<Color = Rgb565>,
    <D as embedded_graphics::draw_target::DrawTarget>::Error: core::fmt::Debug,
{
    let circle_style = PrimitiveStyle::with_fill(BUTTON_SEMICIRCLE_COLOUR);
    let outline_style = PrimitiveStyle::with_stroke(BUTTON_SEMICIRCLE_COLOUR, 2);
    let button_tooltip_font = FONT_10X20;
    let _button_tooltip_font_w = button_tooltip_font.character_size.width;
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
                    .try_into()
                    .unwrap_or_default(),
                13,
            )
        }
        ButtonPos::BottomLeft => Point::new(1, DISPLAY_H as i32 - 1),
        ButtonPos::BottomRight => Point::new(
            (DISPLAY_W as u32)
                .saturating_sub(1)
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
        ButtonStatus::Off => Circle::with_center(circle_pos, SEMICIRCLE_DIAMETER)
            .into_styled(outline_style)
            .draw(display)
            .unwrap(),
        ButtonStatus::On => todo!(),
        ButtonStatus::Held { percentage } => todo!(),
    }
    Text::with_alignment(
        text,
        char_pos,
        button_text_style,
        embedded_graphics::text::Alignment::Right,
    )
    .draw(display)
    .unwrap();
}
