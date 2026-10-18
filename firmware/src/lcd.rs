//! 이 보드의 1.44" ST7735S 패널 모델 (mipidsi `Model` 구현).
//!
//! mipidsi에 들어 있는 `ST7735s` 모델은 이 패널과 두 가지가 맞지 않는다.
//! - 초기화 값(전원 PWCTR1~5, 감마)이 다른 모듈 기준이다. 이 패널에 그대로 쓰면 명령은 들어가도
//!   화면이 백라이트만 켜진 흰색으로 남는다 (실기 확인).
//! - 메모리를 132×162로 보고 뒤집힌 방향의 오프셋을 계산하는데, 이 패널은 132×132처럼 동작한다.
//!
//! 초기화 순서와 값은 실기에서 화면이 나오는 것을 확인한 CircuitPython 보드 정의
//! (Adafruit ST7735R 순서)를 따른다.

use embedded_graphics::pixelcolor::Rgb565;
use embedded_hal::delay::DelayNs;
use mipidsi::{
    dcs::{
        BitsPerPixel, EnterNormalMode, ExitSleepMode, InterfaceExt, PixelFormat, SetAddressMode,
        SetDisplayOn, SetInvertMode, SetPixelFormat,
    },
    interface::Interface,
    models::{Model, ModelInitError},
    options::ModelOptions,
};

/// Spotpear ESP32-C3 1.44" LCD 패널 (ST7735S, 128×128)
pub struct St7735s144;

impl Model for St7735s144 {
    type ColorFormat = Rgb565;

    /// 오프셋이 회전 없는 방향에서 (2, 1), 상하좌우를 뒤집은 방향에서 (2, 3)이므로
    /// 메모리는 132×132로 본다.
    const FRAMEBUFFER_SIZE: (u16, u16) = (132, 132);

    /// 리셋 펄스 길이(µs). 기본값 10µs는 FreeRtos 지연에서 틱 단위로 올림되어 길이가
    /// 들쭉날쭉하므로, 제조사 데모(TFT_eSPI)처럼 20ms로 넉넉히 준다.
    const RESET_DURATION: u32 = 20_000;

    fn init<DELAY, DI>(
        &mut self,
        di: &mut DI,
        delay: &mut DELAY,
        options: &ModelOptions,
    ) -> Result<SetAddressMode, ModelInitError<DI::Error>>
    where
        DELAY: DelayNs,
        DI: Interface,
    {
        let madctl = SetAddressMode::from(options);

        // 리셋이 풀린 뒤 명령을 받기까지 120ms가 필요하다
        delay.delay_ms(150);
        di.write_command(ExitSleepMode)?;
        delay.delay_ms(150);

        di.write_raw(0xB1, &[0x01, 0x2C, 0x2D])?; // FRMCTR1: 프레임 주기 (일반 모드)
        di.write_raw(0xB2, &[0x01, 0x2C, 0x2D])?; // FRMCTR2: 프레임 주기 (대기 모드)
        di.write_raw(0xB3, &[0x01, 0x2C, 0x2D, 0x01, 0x2C, 0x2D])?; // FRMCTR3: 부분 모드
        di.write_raw(0xB4, &[0x07])?; // INVCTR: 라인 반전
        di.write_raw(0xC0, &[0xA2, 0x02, 0x84])?; // PWCTR1: GVDD 4.7V, 1.0µA
        di.write_raw(0xC1, &[0xC5])?; // PWCTR2: VGH 14.7V, VGL -7.35V
        di.write_raw(0xC2, &[0x0A, 0x00])?; // PWCTR3: 연산 증폭기 전류 소, 승압 주파수
        di.write_raw(0xC3, &[0x8A, 0x2A])?; // PWCTR4
        di.write_raw(0xC4, &[0x8A, 0xEE])?; // PWCTR5
        di.write_raw(0xC5, &[0x0E])?; // VMCTR1: VCOMH 4V, VCOML -1.1V
        di.write_command(SetInvertMode::new(options.invert_colors))?;
        di.write_command(madctl)?;
        let pf = PixelFormat::with_all(BitsPerPixel::from_rgb_color::<Self::ColorFormat>());
        di.write_command(SetPixelFormat::new(pf))?;
        di.write_raw(
            0xE0, // GMCTRP1: 감마 (+)
            &[
                0x02, 0x1C, 0x07, 0x12, 0x37, 0x32, 0x29, 0x2D, 0x29, 0x25, 0x2B, 0x39, 0x00, 0x01,
                0x03, 0x10,
            ],
        )?;
        di.write_raw(
            0xE1, // GMCTRN1: 감마 (-)
            &[
                0x03, 0x1D, 0x07, 0x06, 0x2E, 0x2C, 0x29, 0x2D, 0x2E, 0x2E, 0x37, 0x3F, 0x00, 0x00,
                0x02, 0x10,
            ],
        )?;
        di.write_command(EnterNormalMode)?;
        delay.delay_ms(10);
        di.write_command(SetDisplayOn)?;
        delay.delay_ms(100);

        Ok(madctl)
    }
}
