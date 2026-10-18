use esp_idf_svc::hal::delay::Ets;
use esp_idf_svc::hal::gpio::{Input, PinDriver};

/// 디바운스에 필요한 연속 동일 샘플 수. 20ms 루프 기준 3개 = 60ms.
const DEBOUNCE_SAMPLES: u8 = 3;

/// active-low 택트 스위치 입력.
///
/// 배선은 `GPIO ──[스위치]── GND`이고 외부 풀업이 없으므로, 내부 풀업을 켠 `PinDriver`를 받는다.
pub struct Button<'d> {
    pin: PinDriver<'d, Input>,
    /// 디바운스된 눌림 상태
    pressed: bool,
    counter: u8,
}

impl<'d> Button<'d> {
    pub fn new(pin: PinDriver<'d, Input>) -> Self {
        // 풀업이 핀을 끌어올릴 시간을 준 뒤 읽는다
        Ets::delay_us(50);
        // 켤 때 이미 눌려 있던 버튼은 눌린 상태로 시작해, 그 누름이 새 누름으로 잡히지 않게 한다
        let pressed = pin.is_low();
        Self {
            pin,
            pressed,
            counter: 0,
        }
    }

    /// 디바운스 전 핀 상태. 지금 눌려 있으면 true.
    pub fn is_held(&self) -> bool {
        self.pin.is_low()
    }

    /// 루프 주기마다 한 번 부른다. 이번에 새로 눌렸으면 true.
    pub fn pressed(&mut self) -> bool {
        let raw = self.pin.is_low();
        if raw == self.pressed {
            self.counter = 0;
            return false;
        }
        self.counter += 1;
        if self.counter < DEBOUNCE_SAMPLES {
            return false;
        }
        self.counter = 0;
        self.pressed = raw;
        raw
    }
}
