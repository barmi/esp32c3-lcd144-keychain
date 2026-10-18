//! 빌드할 때 `firmware/cfg.toml`에서 읽는 설정 (toml-cfg).
//!
//! `cfg.toml`은 커밋하지 않는다(`cfg.toml.example` 참고). 파일이 없으면 아래 기본값으로 빌드되고,
//! Wi-Fi SSID가 비어 있으면 화면에 설정이 없다고 표시한다 (CI 빌드가 이 경우다).

#[toml_cfg::toml_config]
pub struct Config {
    #[default("")]
    wifi_ssid: &'static str,
    #[default("")]
    wifi_password: &'static str,
    /// 날씨를 볼 위치. Open-Meteo URL에 그대로 들어간다. 기본값은 서울시청.
    #[default("37.5665")]
    latitude: &'static str,
    #[default("126.9780")]
    longitude: &'static str,
}
