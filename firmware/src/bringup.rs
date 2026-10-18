//! 하드웨어 점검 모드. K2를 누른 채 켜거나 RESET하면 들어간다.
//!
//! LCD(색 반전·방향·오프셋), 버튼 K1/K2/BOOT, 빨간 LED가 회로도대로 동작하는지 눈으로
//! 확인한다. 핀 배치와 근거는 docs/hardware.md 참고.

use core::fmt::Debug;

use embedded_graphics::{
    mono_font::{
        ascii::{FONT_10X20, FONT_6X10},
        MonoTextStyle,
    },
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{PrimitiveStyle, PrimitiveStyleBuilder, Rectangle, Triangle},
    text::{Alignment, Baseline, Text, TextStyleBuilder},
};
use esp_idf_svc::hal::{
    delay::FreeRtos,
    gpio::{Output, PinDriver},
};

use crate::button::Button;
use crate::{lcd_err, FW_VERSION};

/// 버튼 이름 (화면·로그 표시용). 순서는 `run`의 `buttons`와 같다.
const BUTTON_NAMES: [&str; 3] = ["K1", "K2", "BOOT"];

/// 버튼 폴링 주기 (ms)
const LOOP_PERIOD_MS: u32 = 20;

/// LED 깜빡임 반주기 (ms)
const LED_HALF_PERIOD_MS: u32 = 500;

/// 점검 화면을 띄우고 버튼 입력을 계속 보여 준다. 오류가 나야만 돌아온다.
pub fn run<D>(
    display: &mut D,
    buttons: [&Button<'_>; 3],
    led: &mut PinDriver<'_, Output>,
) -> anyhow::Result<()>
where
    D: DrawTarget<Color = Rgb565>,
    D::Error: Debug,
{
    // 색 점검: 화면 색과 글자가 다르면 반전(invert_colors)이나 색 순서(color_order)가 틀린 것
    for (name, color) in [
        ("RED", Rgb565::RED),
        ("GREEN", Rgb565::GREEN),
        ("BLUE", Rgb565::BLUE),
        ("WHITE", Rgb565::WHITE),
        ("BLACK", Rgb565::BLACK),
    ] {
        log::info!("색 점검: {name}");
        draw_color_card(display, name, color).map_err(lcd_err)?;
        FreeRtos::delay_ms(800);
    }

    draw_test_screen(display).map_err(lcd_err)?;
    let mut pressed = [false; 3];
    for (i, button) in buttons.iter().enumerate() {
        pressed[i] = button.is_held();
        draw_button(display, i, pressed[i]).map_err(lcd_err)?;
    }
    log::info!("점검 화면 표시. 버튼을 누르면 해당 칸이 초록색이 된다");

    let mut tick: u32 = 0;
    loop {
        for (i, button) in buttons.iter().enumerate() {
            let now = button.is_held();
            if now != pressed[i] {
                pressed[i] = now;
                log::info!("{} {}", BUTTON_NAMES[i], if now { "눌림" } else { "뗌" });
                draw_button(display, i, now).map_err(lcd_err)?;
            }
        }
        if tick % (LED_HALF_PERIOD_MS / LOOP_PERIOD_MS) == 0 {
            led.toggle()?;
        }
        tick = tick.wrapping_add(1);
        FreeRtos::delay_ms(LOOP_PERIOD_MS);
    }
}

/// 화면 전체를 `color`로 채우고 가운데에 색 이름을 쓴다.
fn draw_color_card<D>(d: &mut D, name: &str, color: Rgb565) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Rgb565>,
{
    d.clear(color)?;
    let ink = if color == Rgb565::WHITE || color == Rgb565::GREEN {
        Rgb565::BLACK
    } else {
        Rgb565::WHITE
    };
    let center = d.bounding_box().center();
    Text::with_text_style(
        name,
        center,
        MonoTextStyle::new(&FONT_10X20, ink),
        TextStyleBuilder::new()
            .alignment(Alignment::Center)
            .baseline(Baseline::Middle)
            .build(),
    )
    .draw(d)?;
    Ok(())
}

/// 점검 화면의 고정 요소: 테두리, 위쪽 화살표, 버튼 이름, 펌웨어 버전.
///
/// - 테두리 네 변이 모두 1px로 보여야 오프셋이 맞다. 한 변이 사라지고 반대편에 잡음이
///   보이면 `LCD_OFFSET`이 틀린 것이다.
/// - 화살표는 화면의 위쪽을 가리킨다. 기기를 어느 방향으로 쥐었을 때 똑바로 보이는지
///   확인해 화면 방향을 정한다.
fn draw_test_screen<D>(d: &mut D) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Rgb565>,
{
    d.clear(Rgb565::BLACK)?;
    d.bounding_box()
        .into_styled(PrimitiveStyle::with_stroke(Rgb565::WHITE, 1))
        .draw(d)?;
    Triangle::new(Point::new(64, 5), Point::new(55, 17), Point::new(73, 17))
        .into_styled(PrimitiveStyle::with_fill(Rgb565::YELLOW))
        .draw(d)?;

    let small = MonoTextStyle::new(&FONT_6X10, Rgb565::WHITE);
    let centered = TextStyleBuilder::new()
        .alignment(Alignment::Center)
        .baseline(Baseline::Middle)
        .build();
    Text::with_text_style("TOP", Point::new(64, 25), small, centered).draw(d)?;

    let label = MonoTextStyle::new(&FONT_10X20, Rgb565::WHITE);
    let left_middle = TextStyleBuilder::new().baseline(Baseline::Middle).build();
    for (i, name) in BUTTON_NAMES.iter().enumerate() {
        let y = button_row_y(i) + BUTTON_BOX_SIZE.height as i32 / 2;
        Text::with_text_style(name, Point::new(10, y), label, left_middle).draw(d)?;
    }

    let version = MonoTextStyle::new(&FONT_6X10, Rgb565::CSS_GRAY);
    Text::with_text_style(FW_VERSION, Point::new(64, 118), version, centered).draw(d)?;
    Ok(())
}

/// 버튼 상태 칸 크기
const BUTTON_BOX_SIZE: Size = Size::new(36, 18);

/// `row`번째 버튼 칸의 위쪽 y 좌표
fn button_row_y(row: usize) -> i32 {
    36 + row as i32 * 24
}

/// 버튼 상태 칸: 눌리면 초록으로 채우고, 떼면 회색 테두리만 그린다.
fn draw_button<D>(d: &mut D, row: usize, pressed: bool) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Rgb565>,
{
    let fill = if pressed {
        Rgb565::GREEN
    } else {
        Rgb565::BLACK
    };
    let style = PrimitiveStyleBuilder::new()
        .stroke_color(Rgb565::CSS_GRAY)
        .stroke_width(1)
        .fill_color(fill)
        .build();
    Rectangle::new(Point::new(80, button_row_y(row)), BUTTON_BOX_SIZE)
        .into_styled(style)
        .draw(d)
}
