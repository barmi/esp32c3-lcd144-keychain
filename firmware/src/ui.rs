//! 화면 그리기.
//!
//! 128×128 프레임버퍼(32KB)에 화면 전체를 그린 뒤 한 번에 LCD로 보낸다. LCD에 바로 지우고
//! 그리면 1초마다 시계가 깜빡인다.

use core::convert::Infallible;
use core::f32::consts::FRAC_PI_4;

use embedded_graphics::{
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{Circle, Line, Polyline, PrimitiveStyle, Rectangle},
};
use u8g2_fonts::{
    fonts,
    types::{FontColor, HorizontalAlignment as Align, VerticalPosition},
    FontRenderer,
};

use crate::clock::LocalTime;
use crate::net::{Status, WifiState};
use crate::weather::Sky;

const WIDTH: u32 = 128;
const HEIGHT: u32 = 128;

/// 시계 숫자 (숫자·콜론·하이픈만 있다)
const BIG: FontRenderer = FontRenderer::new::<fonts::u8g2_font_logisoso38_tn>();
/// 기온 (° 기호가 있는 글꼴)
const MEDIUM: FontRenderer =
    FontRenderer::new::<fonts::u8g2_font_logisoso26_tf>().with_ignore_unknown_chars(true);
const LABEL: FontRenderer =
    FontRenderer::new::<fonts::u8g2_font_helvB12_tr>().with_ignore_unknown_chars(true);
const SMALL: FontRenderer =
    FontRenderer::new::<fonts::u8g2_font_6x12_tf>().with_ignore_unknown_chars(true);

const TEXT: Rgb565 = Rgb565::WHITE;
const MUTED: Rgb565 = Rgb565::CSS_SILVER;
const RULE: Rgb565 = Rgb565::CSS_DIM_GRAY;
const WARN: Rgb565 = Rgb565::CSS_ORANGE;
const SUN: Rgb565 = Rgb565::CSS_GOLD;
const MOON: Rgb565 = Rgb565::CSS_LEMON_CHIFFON;
const CLOUD: Rgb565 = Rgb565::CSS_LIGHT_GRAY;
const STORM_CLOUD: Rgb565 = Rgb565::CSS_GRAY;
const WATER: Rgb565 = Rgb565::CSS_DEEP_SKY_BLUE;

/// 화면 전체를 담는 프레임버퍼
pub struct Frame {
    pixels: Vec<Rgb565>,
}

impl Default for Frame {
    fn default() -> Self {
        Self {
            pixels: vec![Rgb565::BLACK; (WIDTH * HEIGHT) as usize],
        }
    }
}

impl Frame {
    /// 왼쪽 위부터 한 줄씩 픽셀을 돌려준다 (LCD에 그대로 보낼 순서)
    pub fn pixels(&self) -> impl Iterator<Item = Rgb565> + '_ {
        self.pixels.iter().copied()
    }
}

impl OriginDimensions for Frame {
    fn size(&self) -> Size {
        Size::new(WIDTH, HEIGHT)
    }
}

impl DrawTarget for Frame {
    type Color = Rgb565;
    type Error = Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(p, color) in pixels {
            if let (Ok(x), Ok(y)) = (u32::try_from(p.x), u32::try_from(p.y)) {
                if x < WIDTH && y < HEIGHT {
                    self.pixels[(y * WIDTH + x) as usize] = color;
                }
            }
        }
        Ok(())
    }

    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        self.pixels.fill(color);
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    /// 날짜, 큰 시계, 현재 날씨 요약
    Clock,
    /// 날씨 자세히
    Weather,
}

impl Screen {
    pub fn next(self) -> Self {
        match self {
            Screen::Clock => Screen::Weather,
            Screen::Weather => Screen::Clock,
        }
    }
}

/// 한 화면을 그리는 데 필요한 모든 값. 이전과 같으면 다시 그리지 않는다.
#[derive(Clone, Debug, PartialEq)]
pub struct View {
    pub screen: Screen,
    pub time: Option<LocalTime>,
    pub status: Status,
}

pub fn draw(f: &mut Frame, view: &View) {
    let _ = f.clear(Rgb565::BLACK);
    match view.screen {
        Screen::Clock => draw_clock(f, view),
        Screen::Weather => draw_weather(f, view),
    }
}

fn draw_clock(f: &mut Frame, view: &View) {
    let date = match view.time {
        Some(t) => format!("{:02}-{:02} {}", t.month, t.day, t.weekday_name()),
        None => "--  --".into(),
    };
    text(f, &LABEL, &date, (6, 12), Align::Left, MUTED);
    wifi_dot(f, view.status.wifi);

    // 시와 분을 따로 그려 콜론이 깜빡여도 숫자가 움직이지 않게 한다. 콜론은 짝수 초에만 그린다.
    let (hours, minutes, colon) = match view.time {
        Some(t) => (
            format!("{:02}", t.hour),
            format!("{:02}", t.minute),
            t.second % 2 == 0,
        ),
        None => ("--".into(), "--".into(), true),
    };
    let y = 46;
    text(f, &BIG, &hours, (59, y), Align::Right, TEXT);
    if colon {
        text(f, &BIG, ":", (64, y), Align::Center, TEXT);
    }
    text(f, &BIG, &minutes, (69, y), Align::Left, TEXT);

    put(
        f,
        Line::new(Point::new(6, 73), Point::new(121, 73))
            .into_styled(PrimitiveStyle::with_stroke(RULE, 1)),
    );

    match view.status.weather {
        Some(w) => {
            icon(f, Point::new(6, 82), Sky::from_wmo(w.code), w.is_day);
            let temperature = format!("{:.0}°", w.temperature);
            text(f, &MEDIUM, &temperature, (122, 94), Align::Right, TEXT);
            let humidity = format!("{}%", w.humidity);
            text(f, &SMALL, &humidity, (122, 117), Align::Right, MUTED);
            if view.status.weather_failed {
                // 갱신에 실패해 오래된 값을 보여 주는 중
                text(f, &SMALL, "!", (50, 117), Align::Left, WARN);
            }
        }
        None => {
            let message = status_message(&view.status);
            text(f, &LABEL, message, (64, 100), Align::Center, MUTED);
        }
    }
}

fn draw_weather(f: &mut Frame, view: &View) {
    text(f, &LABEL, "WEATHER", (6, 12), Align::Left, MUTED);
    wifi_dot(f, view.status.wifi);

    let Some(w) = view.status.weather else {
        let message = status_message(&view.status);
        text(f, &LABEL, message, (64, 64), Align::Center, MUTED);
        return;
    };

    let sky = Sky::from_wmo(w.code);
    icon(f, Point::new(6, 24), sky, w.is_day);
    let temperature = format!("{:.1}°", w.temperature);
    text(f, &MEDIUM, &temperature, (122, 38), Align::Right, TEXT);
    text(f, &SMALL, sky.label(), (122, 58), Align::Right, MUTED);

    let lines = [
        format!("HI {:.0}°  LO {:.0}°", w.temp_max, w.temp_min),
        format!("HUMIDITY {}%", w.humidity),
        format!("WIND {:.1} m/s", w.wind_speed),
    ];
    for (i, line) in lines.iter().enumerate() {
        let y = 74 + 14 * i as i32;
        text(f, &SMALL, line, (6, y), Align::Left, TEXT);
    }

    let (updated, color) = match (view.status.weather_failed, view.status.weather_updated) {
        (true, _) => ("UPDATE FAILED".to_string(), WARN),
        (false, Some(t)) => (format!("UPDATED {:02}:{:02}", t.hour, t.minute), MUTED),
        (false, None) => ("UPDATED --:--".to_string(), MUTED),
    };
    text(f, &SMALL, &updated, (6, 118), Align::Left, color);
}

/// 날씨를 아직 받지 못했을 때 보여 줄 상태
fn status_message(status: &Status) -> &'static str {
    match status.wifi {
        WifiState::NoConfig => "NO WIFI CFG",
        WifiState::Connecting => "CONNECTING",
        WifiState::Failed => "WIFI FAILED",
        WifiState::Connected if status.weather_failed => "WEATHER FAIL",
        WifiState::Connected => "LOADING",
    }
}

/// 오른쪽 위 Wi-Fi 상태 점: 초록 연결, 노랑 연결 중, 빨강 실패, 회색 설정 없음
fn wifi_dot(f: &mut Frame, wifi: WifiState) {
    let color = match wifi {
        WifiState::Connected => Rgb565::CSS_LIME_GREEN,
        WifiState::Connecting => Rgb565::CSS_GOLD,
        WifiState::Failed => Rgb565::CSS_RED,
        WifiState::NoConfig => RULE,
    };
    put(
        f,
        Circle::with_center(Point::new(118, 12), 7).into_styled(PrimitiveStyle::with_fill(color)),
    );
}

/// 36×36 날씨 아이콘. `o`는 왼쪽 위.
fn icon(f: &mut Frame, o: Point, sky: Sky, is_day: bool) {
    let sun_or_moon = |f: &mut Frame, center: Point, radius: u32| {
        if is_day {
            sun(f, center, radius);
        } else {
            moon(f, center, radius + 2);
        }
    };
    match sky {
        Sky::Clear => sun_or_moon(f, o + Point::new(18, 18), 9),
        Sky::PartlyCloudy => {
            sun_or_moon(f, o + Point::new(12, 12), 6);
            cloud(f, o + Point::new(5, 13), CLOUD);
        }
        Sky::Cloudy => cloud(f, o + Point::new(3, 8), CLOUD),
        Sky::Fog => {
            for i in 0..4 {
                let y = 8 + 7 * i;
                put(
                    f,
                    Line::new(o + Point::new(4, y), o + Point::new(32, y))
                        .into_styled(PrimitiveStyle::with_stroke(CLOUD, 2)),
                );
            }
        }
        Sky::Drizzle => {
            cloud(f, o + Point::new(3, 4), CLOUD);
            for x in [11, 19, 27] {
                put(
                    f,
                    Line::new(o + Point::new(x, 28), o + Point::new(x, 30))
                        .into_styled(PrimitiveStyle::with_stroke(WATER, 2)),
                );
            }
        }
        Sky::Rain => {
            cloud(f, o + Point::new(3, 4), CLOUD);
            for x in [12, 20, 28] {
                put(
                    f,
                    Line::new(o + Point::new(x, 27), o + Point::new(x - 3, 34))
                        .into_styled(PrimitiveStyle::with_stroke(WATER, 2)),
                );
            }
        }
        Sky::Snow => {
            cloud(f, o + Point::new(3, 4), CLOUD);
            for (x, y) in [(11, 29), (19, 33), (27, 29)] {
                put(
                    f,
                    Circle::with_center(o + Point::new(x, y), 4)
                        .into_styled(PrimitiveStyle::with_fill(TEXT)),
                );
            }
        }
        Sky::Thunder => {
            cloud(f, o + Point::new(3, 4), STORM_CLOUD);
            let bolt = [
                o + Point::new(20, 24),
                o + Point::new(15, 30),
                o + Point::new(20, 30),
                o + Point::new(16, 36),
            ];
            put(
                f,
                Polyline::new(&bolt).into_styled(PrimitiveStyle::with_stroke(SUN, 2)),
            );
        }
    }
}

/// 원 세 개와 사각형으로 만든 구름 (약 28×20). `o`는 왼쪽 위.
fn cloud(f: &mut Frame, o: Point, color: Rgb565) {
    let style = PrimitiveStyle::with_fill(color);
    put(
        f,
        Circle::with_center(o + Point::new(6, 13), 12).into_styled(style),
    );
    put(
        f,
        Circle::with_center(o + Point::new(14, 9), 16).into_styled(style),
    );
    put(
        f,
        Circle::with_center(o + Point::new(22, 13), 11).into_styled(style),
    );
    put(
        f,
        Rectangle::new(o + Point::new(6, 13), Size::new(16, 6)).into_styled(style),
    );
}

fn sun(f: &mut Frame, center: Point, radius: u32) {
    put(
        f,
        Circle::with_center(center, 2 * radius + 1).into_styled(PrimitiveStyle::with_fill(SUN)),
    );
    let ray = |distance: u32, angle: f32| {
        let (sin, cos) = angle.sin_cos();
        let d = distance as f32;
        center + Point::new((d * cos).round() as i32, (d * sin).round() as i32)
    };
    for k in 0..8 {
        let angle = k as f32 * FRAC_PI_4;
        put(
            f,
            Line::new(ray(radius + 3, angle), ray(radius + 6, angle))
                .into_styled(PrimitiveStyle::with_stroke(SUN, 2)),
        );
    }
}

/// 원 하나를 검은 원으로 가려 만든 초승달
fn moon(f: &mut Frame, center: Point, radius: u32) {
    put(
        f,
        Circle::with_center(center, 2 * radius + 1).into_styled(PrimitiveStyle::with_fill(MOON)),
    );
    let r = radius as i32;
    put(
        f,
        Circle::with_center(center + Point::new(r / 2, -r / 2), 2 * radius - 1)
            .into_styled(PrimitiveStyle::with_fill(Rgb565::BLACK)),
    );
}

/// 프레임버퍼에 그린다. 프레임버퍼는 그리기 오류가 나지 않는다.
fn put(f: &mut Frame, drawable: impl Drawable<Color = Rgb565, Output = ()>) {
    let _ = drawable.draw(f);
}

/// `(x, y)`가 세로 가운데, `align`에 따라 가로 왼쪽 끝·가운데·오른쪽 끝이 되도록 글자를 쓴다.
/// 글꼴에 없는 글자는 건너뛴다.
fn text(
    f: &mut Frame,
    font: &FontRenderer,
    content: &str,
    (x, y): (i32, i32),
    align: Align,
    color: Rgb565,
) {
    let _ = font.render_aligned(
        content,
        Point::new(x, y),
        VerticalPosition::Center,
        align,
        FontColor::Transparent(color),
        f,
    );
}
