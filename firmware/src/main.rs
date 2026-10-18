//! Wi-Fi 시계·날씨 앱.
//!
//! 메인 스레드는 화면과 버튼을 맡고, 네트워크 스레드(net.rs)가 Wi-Fi 연결, SNTP 시각 동기화,
//! 날씨 갱신을 맡는다. K2를 누른 채 켜면 하드웨어 점검 모드(bringup.rs)로 들어간다.

mod bringup;
mod button;
mod clock;
mod config;
mod lcd;
mod net;
mod ui;
mod weather;

use std::sync::mpsc;

use button::Button;
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::hal::{
    delay::FreeRtos,
    gpio::{AnyInputPin, Pin, PinDriver, Pull},
    peripherals::Peripherals,
    spi::{
        config::{Config, DriverConfig},
        Dma, SpiDeviceDriver,
    },
    units::FromValueType,
};
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::sys::{self, esp};
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

/// SPI 클럭. 제조사 데모는 40MHz로 돌리지만, 1초에 한 번 화면 전체(32KB)를 보내는 데는
/// 이 정도로 충분하다.
const LCD_SPI_MHZ: u32 = 26;

/// 메인 루프 주기 (ms). 버튼 디바운스(button.rs)도 이 주기를 기준으로 한다.
const LOOP_PERIOD_MS: u32 = 20;

/// Wi-Fi가 연결되지 않은 동안 LED 깜빡임 반주기 (ms)
const LED_HALF_PERIOD_MS: u32 = 500;

fn main() -> anyhow::Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    log::info!("esp32c3-lcd144-keychain {FW_VERSION}");

    let p = Peripherals::take()?;
    let pins = p.pins;

    let mut led = PinDriver::output(pins.gpio11)?;
    // 세 버튼 모두 `GPIO ──[스위치]── GND` 배선이라 내부 풀업을 켠다.
    let mut k1 = Button::new(PinDriver::input(pins.gpio8, Pull::Up)?);
    let mut k2 = Button::new(PinDriver::input(pins.gpio10, Pull::Up)?);
    let boot = Button::new(PinDriver::input(pins.gpio9, Pull::Up)?);

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
    // 칩이 리셋되면 GPIO5의 IO MUX 기능은 JTAG(MTDI)로 돌아간다. esp-idf-hal의 PinDriver는
    // 방향만 바꾸고 기능을 GPIO로 고르지 않으므로, 그대로는 LCD 리셋 출력이 핀에 나가지 않는다 (#2).
    esp!(unsafe { sys::gpio_reset_pin(pins.gpio5.pin() as _) })?;
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

    if k2.is_held() {
        log::info!("K2가 눌려 있어 하드웨어 점검 모드로 들어간다");
        return bringup::run(&mut display, [&k1, &k2, &boot], &mut led);
    }

    let shared = net::Shared::default();
    let (refresh_tx, refresh_rx) = mpsc::channel();
    net::spawn(
        p.modem,
        EspSystemEventLoop::take()?,
        EspDefaultNvsPartition::take()?,
        shared.clone(),
        refresh_rx,
    )?;

    let mut frame = ui::Frame::default();
    let mut screen = ui::Screen::Clock;
    let mut shown: Option<ui::View> = None;
    let mut tick: u32 = 0;
    loop {
        if k1.pressed() {
            screen = screen.next();
        }
        if k2.pressed() {
            log::info!("날씨 갱신 요청");
            // 네트워크 스레드가 끝났으면 받을 곳이 없을 뿐이다
            let _ = refresh_tx.send(());
        }

        let view = ui::View {
            screen,
            time: clock::now(),
            status: shared.lock().unwrap().clone(),
        };
        let connected = view.status.wifi == net::WifiState::Connected;
        // 시각에 초가 들어 있어 시계 화면은 1초에 한 번 다시 그린다
        if shown.as_ref() != Some(&view) {
            ui::draw(&mut frame, &view);
            display
                .set_pixels(0, 0, LCD_SIZE.0 - 1, LCD_SIZE.1 - 1, frame.pixels())
                .map_err(lcd_err)?;
            shown = Some(view);
        }

        // Wi-Fi가 연결되지 않은 동안 LED를 깜빡인다
        let blink_on = (tick / (LED_HALF_PERIOD_MS / LOOP_PERIOD_MS)) % 2 == 0;
        if !connected && blink_on {
            led.set_high()?;
        } else {
            led.set_low()?;
        }
        tick = tick.wrapping_add(1);
        FreeRtos::delay_ms(LOOP_PERIOD_MS);
    }
}

/// 디스플레이 드라이버 오류는 std Error가 아니라서 anyhow로 바로 넘길 수 없다.
fn lcd_err(e: impl core::fmt::Debug) -> anyhow::Error {
    anyhow::anyhow!("LCD: {e:?}")
}
