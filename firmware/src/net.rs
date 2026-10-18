//! 네트워크 스레드: Wi-Fi 연결 유지, SNTP 시각 동기화, 날씨 주기 갱신.
//!
//! Wi-Fi 연결과 HTTPS 요청은 몇 초씩 걸리므로 메인 스레드에서 하면 시계가 멈춘다. 화면(메인
//! 스레드)과는 공유 상태([`Shared`])와 날씨 갱신 요청 채널로만 주고받는다.

use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::hal::modem::Modem;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::sntp::EspSntp;
use esp_idf_svc::wifi::{AuthMethod, BlockingWifi, ClientConfiguration, Configuration, EspWifi};

use crate::clock::{self, LocalTime};
use crate::config::CONFIG;
use crate::weather::{self, Weather};

/// 날씨 갱신 주기. Open-Meteo의 현재 날씨는 15분 간격으로 바뀐다.
const WEATHER_PERIOD: Duration = Duration::from_secs(15 * 60);

/// 날씨 조회에 실패했을 때 다시 시도하기까지
const WEATHER_RETRY: Duration = Duration::from_secs(60);

/// SNTP로 시각을 맞출 때까지 첫 날씨 조회를 이만큼 기다린다. 갱신 시각을 남기려면 시각이
/// 필요해서다. 넘으면 시각 없이 조회한다.
const SNTP_WAIT: Duration = Duration::from_secs(20);

/// Wi-Fi 연결에 실패했을 때 다시 시도하기까지
const RECONNECT_DELAY: Duration = Duration::from_secs(10);

/// HTTPS(mbedTLS) 핸드셰이크가 이 스레드에서 돌아서 스택을 넉넉히 준다
const STACK_SIZE: usize = 16 * 1024;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WifiState {
    #[default]
    Connecting,
    Connected,
    Failed,
    /// cfg.toml에 Wi-Fi 설정이 없다
    NoConfig,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Status {
    pub wifi: WifiState,
    pub weather: Option<Weather>,
    /// 마지막으로 날씨를 받은 시각 (그때 시각을 맞추지 못했으면 None)
    pub weather_updated: Option<LocalTime>,
    /// 마지막 날씨 조회가 실패했다. 그 전에 받은 값은 `weather`에 남아 있다.
    pub weather_failed: bool,
}

pub type Shared = Arc<Mutex<Status>>;

/// 네트워크 스레드를 띄운다. `refresh`로 메시지가 오면 날씨를 바로 다시 받는다.
pub fn spawn(
    modem: Modem<'static>,
    sysloop: EspSystemEventLoop,
    nvs: EspDefaultNvsPartition,
    shared: Shared,
    refresh: Receiver<()>,
) -> anyhow::Result<()> {
    std::thread::Builder::new()
        .name("net".into())
        .stack_size(STACK_SIZE)
        .spawn(move || {
            if let Err(e) = run(modem, sysloop, nvs, &shared, &refresh) {
                log::error!("네트워크 스레드 종료: {e:?}");
                update(&shared, |s| s.wifi = WifiState::Failed);
            }
        })?;
    Ok(())
}

fn update(shared: &Shared, f: impl FnOnce(&mut Status)) {
    f(&mut shared.lock().unwrap());
}

fn run(
    modem: Modem<'static>,
    sysloop: EspSystemEventLoop,
    nvs: EspDefaultNvsPartition,
    shared: &Shared,
    refresh: &Receiver<()>,
) -> anyhow::Result<()> {
    if CONFIG.wifi_ssid.is_empty() {
        log::warn!("cfg.toml에 Wi-Fi 설정이 없다 (firmware/cfg.toml.example 참고)");
        update(shared, |s| s.wifi = WifiState::NoConfig);
        return Ok(());
    }

    let mut wifi = BlockingWifi::wrap(EspWifi::new(modem, sysloop.clone(), Some(nvs))?, sysloop)?;
    wifi.set_configuration(&Configuration::Client(ClientConfiguration {
        ssid: CONFIG
            .wifi_ssid
            .try_into()
            .map_err(|_| anyhow::anyhow!("SSID가 32바이트보다 길다"))?,
        password: CONFIG
            .wifi_password
            .try_into()
            .map_err(|_| anyhow::anyhow!("Wi-Fi 비밀번호가 64바이트보다 길다"))?,
        auth_method: if CONFIG.wifi_password.is_empty() {
            AuthMethod::None
        } else {
            AuthMethod::WPA2Personal
        },
        ..Default::default()
    }))?;
    wifi.start()?;

    let mut sntp = None;
    let mut sntp_started = Instant::now();
    let mut next_weather = Instant::now();
    loop {
        if !wifi.is_connected()? {
            update(shared, |s| s.wifi = WifiState::Connecting);
            log::info!("Wi-Fi '{}'에 연결하는 중", CONFIG.wifi_ssid);
            if let Err(e) = wifi.connect().and_then(|()| wifi.wait_netif_up()) {
                log::warn!("Wi-Fi 연결 실패: {e}");
                update(shared, |s| s.wifi = WifiState::Failed);
                std::thread::sleep(RECONNECT_DELAY);
                continue;
            }
            let ip = wifi.wifi().sta_netif().get_ip_info()?.ip;
            log::info!("Wi-Fi 연결됨: {ip}");
            update(shared, |s| s.wifi = WifiState::Connected);
        }

        // SNTP는 처음 연결됐을 때 한 번 시작하면 이후 한 시간마다 스스로 다시 맞춘다
        if sntp.is_none() {
            sntp = Some(EspSntp::new_default()?);
            sntp_started = Instant::now();
        }

        let time_ready = clock::now().is_some() || sntp_started.elapsed() >= SNTP_WAIT;
        if time_ready && Instant::now() >= next_weather {
            match weather::fetch(CONFIG.latitude, CONFIG.longitude) {
                Ok(w) => {
                    log::info!("날씨: {w:?}");
                    update(shared, |s| {
                        s.weather = Some(w);
                        s.weather_updated = clock::now();
                        s.weather_failed = false;
                    });
                    next_weather = Instant::now() + WEATHER_PERIOD;
                }
                Err(e) => {
                    log::warn!("날씨 조회 실패: {e:?}");
                    update(shared, |s| s.weather_failed = true);
                    next_weather = Instant::now() + WEATHER_RETRY;
                }
            }
        }

        // 1초마다 연결 상태를 보고, 갱신 요청(K2)이 오면 날씨를 바로 다시 받는다
        match refresh.recv_timeout(Duration::from_secs(1)) {
            Ok(()) => next_weather = Instant::now(),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return Ok(()),
        }
    }
}
