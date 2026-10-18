//! Open-Meteo 현재 날씨 조회 (HTTPS, API 키 불필요).

use std::time::Duration;

use embedded_svc::{http::client::Client, utils::io};
use esp_idf_svc::http::client::{Configuration, EspHttpConnection};
use serde::Deserialize;

/// 응답은 600바이트 남짓이다
const BODY_MAX: usize = 2048;

const TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Weather {
    /// 기온 (°C)
    pub temperature: f32,
    /// 상대 습도 (%)
    pub humidity: u8,
    /// WMO 날씨 코드
    pub code: u8,
    pub is_day: bool,
    /// 풍속 (m/s)
    pub wind_speed: f32,
    /// 오늘 최고·최저 기온 (°C)
    pub temp_max: f32,
    pub temp_min: f32,
}

#[derive(Deserialize)]
struct Response {
    current: Current,
    daily: Daily,
}

#[derive(Deserialize)]
struct Current {
    temperature_2m: f32,
    relative_humidity_2m: f32,
    weather_code: u8,
    is_day: u8,
    wind_speed_10m: f32,
}

#[derive(Deserialize)]
struct Daily {
    temperature_2m_max: Vec<f32>,
    temperature_2m_min: Vec<f32>,
}

/// `latitude`, `longitude` 위치의 현재 날씨와 오늘 최고·최저 기온을 받는다.
/// 오늘의 기준은 한국 시간(Asia/Seoul)이다.
pub fn fetch(latitude: &str, longitude: &str) -> anyhow::Result<Weather> {
    let url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={latitude}&longitude={longitude}\
         &current=temperature_2m,relative_humidity_2m,weather_code,is_day,wind_speed_10m\
         &daily=temperature_2m_max,temperature_2m_min&forecast_days=1\
         &wind_speed_unit=ms&timezone=Asia%2FSeoul"
    );
    let connection = EspHttpConnection::new(&Configuration {
        // ESP-IDF에 들어 있는 루트 인증서 묶음으로 서버 인증서를 검증한다
        crt_bundle_attach: Some(esp_idf_svc::sys::esp_crt_bundle_attach),
        timeout: Some(TIMEOUT),
        ..Default::default()
    })?;
    let mut client = Client::wrap(connection);
    let mut response = client.get(&url)?.submit()?;
    let status = response.status();
    anyhow::ensure!(status == 200, "Open-Meteo 응답 HTTP {status}");

    let mut body = [0u8; BODY_MAX];
    let len = io::try_read_full(&mut response, &mut body).map_err(|(e, _)| e)?;
    anyhow::ensure!(
        len < BODY_MAX,
        "Open-Meteo 응답이 {BODY_MAX}바이트보다 길다"
    );
    let r: Response = serde_json::from_slice(&body[..len])?;
    // daily는 forecast_days=1이라 오늘 값 하나만 들어 있다
    let today = |values: &[f32]| values.first().copied().unwrap_or(f32::NAN);

    Ok(Weather {
        temperature: r.current.temperature_2m,
        humidity: r.current.relative_humidity_2m.round() as u8,
        code: r.current.weather_code,
        is_day: r.current.is_day != 0,
        wind_speed: r.current.wind_speed_10m,
        temp_max: today(&r.daily.temperature_2m_max),
        temp_min: today(&r.daily.temperature_2m_min),
    })
}

/// 화면에 그릴 날씨 종류. WMO 날씨 코드를 아이콘 단위로 묶는다.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sky {
    Clear,
    PartlyCloudy,
    Cloudy,
    Fog,
    Drizzle,
    Rain,
    Snow,
    Thunder,
}

impl Sky {
    /// https://open-meteo.com/en/docs 의 "WMO Weather interpretation codes"
    pub fn from_wmo(code: u8) -> Self {
        match code {
            0 => Sky::Clear,
            1 | 2 => Sky::PartlyCloudy,
            45 | 48 => Sky::Fog,
            51..=57 => Sky::Drizzle,
            61..=67 | 80..=82 => Sky::Rain,
            71..=77 | 85 | 86 => Sky::Snow,
            95..=99 => Sky::Thunder,
            _ => Sky::Cloudy,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Sky::Clear => "CLEAR",
            Sky::PartlyCloudy => "PARTLY CLOUDY",
            Sky::Cloudy => "CLOUDY",
            Sky::Fog => "FOG",
            Sky::Drizzle => "DRIZZLE",
            Sky::Rain => "RAIN",
            Sky::Snow => "SNOW",
            Sky::Thunder => "THUNDER",
        }
    }
}
