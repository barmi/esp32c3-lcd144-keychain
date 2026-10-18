//! 시스템 시각(UTC)을 한국 표준시로 바꾼다.

use std::time::{SystemTime, UNIX_EPOCH};

/// KST = UTC+9. 일광 절약 시간이 없으므로 고정 오프셋이면 된다.
const UTC_OFFSET_SECS: i64 = 9 * 3600;

/// 시스템 시각이 이보다 이르면(2025-01-01 UTC) 아직 SNTP로 맞추지 않은 것으로 본다.
/// 부팅 직후 시스템 시각은 1970년에서 시작한다.
const SYNCED_AFTER_SECS: i64 = 1_735_689_600;

const WEEKDAYS: [&str; 7] = ["SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalTime {
    pub year: i32,
    pub month: u8,
    pub day: u8,
    /// 0 = 일요일
    pub weekday: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

impl LocalTime {
    pub fn weekday_name(&self) -> &'static str {
        WEEKDAYS[usize::from(self.weekday)]
    }
}

/// 지금 한국 시각. 아직 시각을 맞추지 않았으면 None.
pub fn now() -> Option<LocalTime> {
    let utc = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs() as i64;
    (utc >= SYNCED_AFTER_SECS).then(|| from_unix(utc + UTC_OFFSET_SECS))
}

/// 유닉스 시각(초)을 날짜와 시각으로 나눈다.
fn from_unix(secs: i64) -> LocalTime {
    let days = secs.div_euclid(86_400);
    let time = secs.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    LocalTime {
        year,
        month,
        day,
        // 1970-01-01은 목요일
        weekday: (days + 4).rem_euclid(7) as u8,
        hour: (time / 3600) as u8,
        minute: (time / 60 % 60) as u8,
        second: (time % 60) as u8,
    }
}

/// 1970-01-01부터 센 날 수를 (연, 월, 일)로 바꾼다.
/// Howard Hinnant의 `civil_from_days` 알고리즘 (http://howardhinnant.github.io/date_algorithms.html).
fn civil_from_days(days: i64) -> (i32, u8, u8) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year as i32, month as u8, day as u8)
}
