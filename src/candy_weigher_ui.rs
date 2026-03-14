use crate::pimori_display::{DISPLAY_H, DISPLAY_W};
use core::fmt::Write;
use embedded_graphics::mono_font::ascii::FONT_10X20;
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{
    Arc, Circle, PrimitiveStyle, PrimitiveStyleBuilder, Rectangle, StyledDrawable,
};
use embedded_graphics::text::Text;

pub struct DisplayState {
    pub scale_weight_g: f32,
    pub lolly_weight_g: f32,
    pub lolly_count: u32,
    pub lolly_count_change: u32,
    pub t_l_pressed: bool,
    pub b_l_pressed: bool,
    pub t_r_pressed: bool,
    pub b_r_pressed: bool,
}

pub fn draw<D>(state: &DisplayState, display: &mut D)
where
    D: DrawTarget<Color = Rgb565>,
    <D as embedded_graphics::draw_target::DrawTarget>::Error: core::fmt::Debug,
{
    let arc_diameter = 40;
    let circle_style = PrimitiveStyle::with_fill(Rgb565::WHITE);
    let arc_style = PrimitiveStyle::with_stroke(Rgb565::WHITE, 2);
    let arc_t_l = Arc::with_center(Point::new(0, 0), arc_diameter, 0.0.deg(), 90.0.deg())
        .into_styled(arc_style);
    let arc_b_l = Arc::with_center(
        Point::new(0, DISPLAY_H as i32),
        arc_diameter,
        270.0.deg(),
        90.0.deg(),
    )
    .into_styled(arc_style);
    let arc_t_r = Arc::with_center(
        Point::new(DISPLAY_W as i32, 0),
        arc_diameter,
        90.0.deg(),
        90.0.deg(),
    )
    .into_styled(arc_style);
    let arc_b_r = Arc::with_center(
        Point::new(DISPLAY_W as i32, DISPLAY_H as i32),
        arc_diameter,
        180.0.deg(),
        90.0.deg(),
    )
    .into_styled(arc_style);
    let circle_t_l = Circle::with_center(Point::new(0, 0), arc_diameter).into_styled(circle_style);
    let circle_b_l = Circle::with_center(Point::new(0, DISPLAY_H as i32), arc_diameter)
        .into_styled(circle_style);
    let circle_t_r = Circle::with_center(Point::new(DISPLAY_W as i32, 0), arc_diameter)
        .into_styled(circle_style);
    let circle_b_r =
        Circle::with_center(Point::new(DISPLAY_W as i32, DISPLAY_H as i32), arc_diameter)
            .into_styled(circle_style);
    let button_text_style = MonoTextStyle::new(&FONT_10X20, Rgb565::GREEN);
    let text_t_l = Text::new("+", Point::new(28, 20), button_text_style);
    let text_b_l = Text::new(
        "-",
        Point::new(28, DISPLAY_H as i32 - 10),
        button_text_style,
    );
    let text_t_r = Text::with_alignment(
        "R",
        Point::new(DISPLAY_W as i32 - 28, 20),
        button_text_style,
        embedded_graphics::text::Alignment::Right,
    );
    let text_b_r = Text::with_alignment(
        "T",
        Point::new(DISPLAY_W as i32 - 28, DISPLAY_H as i32 - 10),
        button_text_style,
        embedded_graphics::text::Alignment::Right,
    );
    let weight_text_style = MonoTextStyle::new(&FONT_10X20, Rgb565::BLUE);
    let mut scale_weight_str = heapless::String::<30>::new();
    let mut lolly_weight_str = heapless::String::<30>::new();
    let mut lolly_count_str = heapless::String::<30>::new();
    let mut lolly_count_change_str = heapless::String::<30>::new();
    core::write!(
        &mut scale_weight_str,
        "Scale weight: {:.1}",
        state.scale_weight_g
    )
    .unwrap();
    core::write!(
        &mut lolly_weight_str,
        "Lolly weight: {:.1}",
        state.lolly_weight_g
    )
    .unwrap();
    core::write!(&mut lolly_count_str, "Lolly count: {}", state.lolly_count).unwrap();
    core::write!(
        &mut lolly_count_change_str,
        "Lolly change: {}",
        state.lolly_count_change
    )
    .unwrap();
    let text_scale_weight = Text::new(&scale_weight_str, Point::new(28, 40), weight_text_style);
    let text_lolly_weight = Text::new(&lolly_weight_str, Point::new(28, 60), weight_text_style);
    let text_lolly_count = Text::new(&lolly_count_str, Point::new(28, 80), weight_text_style);
    let text_lolly_change = Text::new(
        &lolly_count_change_str,
        Point::new(28, 100),
        weight_text_style,
    );

    display.clear(Rgb565::BLACK).unwrap();

    if state.t_l_pressed {
        circle_t_l.draw(display).unwrap();
    } else {
        arc_t_l.draw(display).unwrap();
    };
    if state.t_r_pressed {
        circle_t_r.draw(display).unwrap();
    } else {
        arc_t_r.draw(display).unwrap();
    };
    if state.b_l_pressed {
        circle_b_l.draw(display).unwrap();
    } else {
        arc_b_l.draw(display).unwrap();
    };
    if state.b_r_pressed {
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
