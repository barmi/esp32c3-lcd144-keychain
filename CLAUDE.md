# esp32c3-lcd144-keychain

Spotpear ESP32-C3 1.44" LCD 키링용 펌웨어. Rust std(ESP-IDF) 환경.

## 워크플로

- GitHub 이슈(barmi/esp32c3-lcd144-keychain) 단위로 작업을 진행한다. 새 작업이 생기면 이슈부터 등록.
- 문서와 이슈는 한국어로 작성한다.

## 저장소 구조

- 루트에는 문서(README.md, CLAUDE.md)만 둔다. Rust 펌웨어는 `firmware/`가 cargo 프로젝트 루트.

## 빌드 / 실행

- 빌드/실행은 반드시 `firmware/` 안에서: `cargo build`, `cargo run`(espflash 플래시 + 시리얼 모니터).
- 첫 빌드는 ESP-IDF(v5.3.3)를 `firmware/.embuild/`에 내려받아 오래 걸린다. 보드 포트는 `/dev/cu.usbmodem*`.
- 타겟: `riscv32imc-esp-espidf` (build-std, firmware/rust-toolchain.toml이 자동 적용).
- 버전은 `firmware/Cargo.toml`이 기준이고 펌웨어에는 `0.1.0+<커밋>`으로 찍힌다 (build.rs).
- nightly는 `nightly-2026-07-20`으로 고정. 이후 nightly는 std의 fchmodat/AT_FDCWD 사용(rust-lang/rust#158168) 때문에 espidf 타겟 빌드가 깨진다. libc가 espidf에 해당 심볼을 추가하면 고정 해제 가능.

## 하드웨어 핵심 사실

- ESP32-C3(QFN32) + 외장 플래시 W25Q128 **16MB**. USB는 칩 내장 USB-Serial/JTAG이고 USB-UART 칩은 없다.
- LCD ST7735S 128×128: **SCK=GPIO3, MOSI=GPIO4, CS=GPIO2, DC=GPIO0, RST=GPIO5**. 백라이트는 3.3V 직결이라 끄거나 밝기를 바꿀 수 없다.
- 버튼(모두 active-low, 외부 풀업 없음 → 내부 풀업): **K1=GPIO8, K2=GPIO10, BOOT=GPIO9**. GPIO8·9는 부팅 스트래핑 핀.
- 빨간 LED=GPIO11 (active-high). eFuse `VDD_SPI_AS_GPIO`가 이미 켜져 있어 GPIO로 쓸 수 있다.
- 헤더: IO1, IO6, IO7, IO20(U0RXD), IO21(U0TXD), GND.
- 배터리 전압 측정 회로와 전원 스위치가 없다.
- LCD는 mipidsi 내장 `ST7735s` 모델로는 흰 화면만 나온다 (초기화 값이 이 패널과 맞지 않음). 패널 전용 모델 `firmware/src/lcd.rs`를 쓴다: CircuitPython 초기화 순서, 메모리 132×132.
- 색 반전은 끈다 (제조사 데모의 TFT_INVERSION_ON과 다름, 실기 확인). 오프셋은 패널 기본 방향에서 (2, 1).
- 패널 기본 방향은 USB 단자를 오른쪽에 둘 때 위쪽이 위로 온다. 펌웨어는 USB를 아래로 두고 쓰도록 270° 돌린다. 이 방향과 BGR 색 순서는 실기에서 확인했다 (docs/hardware.md#lcd).

## 주의

- CI가 `cargo clippy -- -D warnings`로 막고 있으니 경고를 남기지 말 것.
- 출고 펌웨어에는 Wi-Fi 비밀번호와 API 키가 평문으로 들어 있었다. 플래시 덤프(`*.bin`)와 비밀값은 커밋하지 않는다. Wi-Fi 기능을 넣을 때는 비밀값을 gitignore된 파일이나 빌드 환경 변수로 분리한다.
