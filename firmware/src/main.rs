//! 하드웨어 점검 펌웨어.
//!
//! LCD(색 반전·방향·오프셋), 버튼 K1/K2/BOOT, 빨간 LED가 회로도대로 동작하는지 눈으로
//! 확인한다. 핀 배치와 근거는 docs/hardware.md 참고.

mod lcd;

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
    gpio::{AnyInputPin, PinDriver, Pull},
    peripherals::Peripherals,
    spi::{
        config::{Config, DriverConfig},
        Dma, SpiDeviceDriver,
    },
    units::FromValueType,
};
use mipidsi::{
    interface::SpiInterface,
    options::{ColorInversion, ColorOrder, Orientation, Rotation},
    Builder,
};

// ESP-IDF 앱 정보(부트로더 로그와 바이너리 헤더의 버전·빌드 시각)를 Cargo.toml에서 채운다.
// 매크로가 esp-idf-sys의 cfg 조건을 이 크레이트 안에서 펼쳐 unexpected_cfgs 경고가 나므로
// 모듈로 감싸 그 범위에서만 끈다 (심볼은 #[no_mangle]이라 모듈 안에 있어도 이름이 같다).
#[allow(unexpected_cfgs)]
mod app_desc {
    // 인자 true로 불러야 빌드 날짜와 시각이 제자리에 들어간다.
    esp_idf_svc::sys::esp_app_desc!(true);
}

/// 펌웨어 버전: "0.1.0"(릴리스 태그) 또는 "0.1.0+<커밋>[-dirty]" (build.rs)
const FW_VERSION: &str = env!("FW_VERSION");

/// 패널 크기 (px)
const LCD_SIZE: (u16, u16) = (128, 128);

/// ST7735S 메모리 안에서 패널이 시작하는 위치. 패널 기본 방향(회전·반전 없음)에서의 값이고,
/// 다른 방향의 오프셋은 mipidsi가 `lcd::St7735s144`의 메모리 크기(132×132)로 계산한다.
const LCD_OFFSET: (u16, u16) = (2, 1);

/// 화면 방향. 패널 기본 방향은 USB 단자를 오른쪽에 둘 때 똑바로 보이므로, USB 단자를
/// 아래로 두고 쓰도록 270° 돌린다 (docs/hardware.md#lcd).
const LCD_ORIENTATION: Orientation = Orientation::new().rotate(Rotation::Deg270);

/// SPI 클럭. 제조사 데모는 40MHz로 돌리지만, 점검 단계라 여유 있게 잡는다.
const LCD_SPI_MHZ: u32 = 26;

/// 버튼 이름 (화면·로그 표시용). 순서는 main의 `buttons` 배열과 같다.
const BUTTON_NAMES: [&str; 3] = ["K1", "K2", "BOOT"];

/// 버튼 폴링 주기 (ms)
const LOOP_PERIOD_MS: u32 = 20;

/// LED 깜빡임 반주기 (ms)
const LED_HALF_PERIOD_MS: u32 = 500;

fn main() -> anyhow::Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    log::info!("esp32c3-lcd144-keychain {FW_VERSION}: 하드웨어 점검");

    let p = Peripherals::take()?;
    let pins = p.pins;

    let mut led = PinDriver::output(pins.gpio11)?;
    // 세 버튼 모두 `GPIO ──[스위치]── GND` 배선이라 내부 풀업을 켠다.
    let buttons = [
        PinDriver::input(pins.gpio8, Pull::Up)?,
        PinDriver::input(pins.gpio10, Pull::Up)?,
        PinDriver::input(pins.gpio9, Pull::Up)?,
    ];

    let spi = SpiDeviceDriver::new_single(
        p.spi2,
        pins.gpio3,                  // SCK
        pins.gpio4,                  // MOSI
        Option::<AnyInputPin>::None, // MISO 없음 (LCD는 쓰기만 한다)
        Some(pins.gpio2),            // CS
        &DriverConfig::new().dma(Dma::Auto(4096)),
        &Config::new().baudrate(LCD_SPI_MHZ.MHz().into()),
    )?;
    let dc = PinDriver::output(pins.gpio0)?;
    let rst = PinDriver::output(pins.gpio5)?;
    let mut buffer = [0u8; 1024];
    let di = SpiInterface::new(spi, dc, &mut buffer);
    // 색 순서는 제조사 데모와 같은 BGR. 색 반전은 하지 않는다: 제조사 데모 설정에는
    // TFT_INVERSION_ON이 있지만, 실기에서는 반전 없이 검정이 검정으로 나온다.
    let mut display = Builder::new(lcd::St7735s144, di)
        .display_size(LCD_SIZE.0, LCD_SIZE.1)
        .display_offset(LCD_OFFSET.0, LCD_OFFSET.1)
        .orientation(LCD_ORIENTATION)
        .color_order(ColorOrder::Bgr)
        .invert_colors(ColorInversion::Normal)
        .reset_pin(rst)
        .init(&mut FreeRtos)
        .map_err(lcd_err)?;

    // 색 점검: 화면 색과 글자가 다르면 반전(invert_colors)이나 색 순서(color_order)가 틀린 것
    for (name, color) in [
        ("RED", Rgb565::RED),
        ("GREEN", Rgb565::GREEN),
        ("BLUE", Rgb565::BLUE),
        ("WHITE", Rgb565::WHITE),
        ("BLACK", Rgb565::BLACK),
    ] {
        log::info!("색 점검: {name}");
        draw_color_card(&mut display, name, color).map_err(lcd_err)?;
        FreeRtos::delay_ms(800);
    }

    draw_test_screen(&mut display).map_err(lcd_err)?;
    let mut pressed = [false; 3];
    for (i, pin) in buttons.iter().enumerate() {
        pressed[i] = pin.is_low();
        draw_button(&mut display, i, pressed[i]).map_err(lcd_err)?;
    }
    log::info!("점검 화면 표시. 버튼을 누르면 해당 칸이 초록색이 된다");

    let mut tick: u32 = 0;
    loop {
        for (i, pin) in buttons.iter().enumerate() {
            let now = pin.is_low();
            if now != pressed[i] {
                pressed[i] = now;
                log::info!("{} {}", BUTTON_NAMES[i], if now { "눌림" } else { "뗌" });
                draw_button(&mut display, i, now).map_err(lcd_err)?;
            }
        }
        if tick % (LED_HALF_PERIOD_MS / LOOP_PERIOD_MS) == 0 {
            led.toggle()?;
        }
        tick = tick.wrapping_add(1);
        FreeRtos::delay_ms(LOOP_PERIOD_MS);
    }
}

/// 디스플레이 드라이버 오류는 std Error가 아니라서 anyhow로 바로 넘길 수 없다.
fn lcd_err(e: impl core::fmt::Debug) -> anyhow::Error {
    anyhow::anyhow!("LCD: {e:?}")
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
