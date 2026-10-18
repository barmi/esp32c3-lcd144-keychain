# esp32c3-lcd144-keychain

Spotpear ESP32-C3 1.44" LCD 키링(ST7735S 128×128, 16MB 플래시, 리튬 배터리, 버튼 2개)용 커스텀 펌웨어.

## 현재 상태

하드웨어 점검 펌웨어 단계입니다. LCD 화면(색·방향), K1/K2/BOOT 버튼, 빨간 LED, RESET 재시작은 실기에서 동작을 확인했습니다. 부팅하면 LCD 색 점검(RED → GREEN → BLUE → WHITE → BLACK)을 한 뒤 점검 화면을 띄웁니다. 점검 화면에는 테두리, 위쪽 화살표, 버튼 상태 칸, 펌웨어 버전이 나옵니다. 빨간 LED는 1초 주기로 깜빡입니다.

| 점검 항목 | 정상 | 아니면 |
|---|---|---|
| 색 점검 | 배경색이 가운데 글자(RED 등)와 같다 | 색 반전(`invert_colors`)이나 색 순서(`color_order`)를 고친다 |
| 테두리 | 네 변 모두 1px 흰 선이 보인다 | `LCD_OFFSET`을 고친다 |
| 화살표 | USB 단자를 아래로 두고 쥐었을 때 위를 가리키고 글자가 똑바로 읽힌다 | `LCD_ORIENTATION`을 고친다 ([LCD](docs/hardware.md#lcd)) |
| 버튼 칸 | K1·K2·BOOT를 누르면 해당 칸이 초록이 된다 | 핀 배치를 확인한다 |

## 직접 빌드하기

```bash
cd firmware && cargo run
```

`cargo install ldproxy espflash`가 선행되어야 하며, 첫 빌드는 ESP-IDF를 내려받느라 오래 걸립니다. USB 케이블 하나로 플래시와 시리얼 모니터가 모두 됩니다 (`/dev/cu.usbmodem*`).

플래시가 잘 안 되면 BOOT를 누른 채 RESET을 눌렀다 떼어 다운로드 모드로 들어간 뒤 다시 시도합니다.

펌웨어 버전은 `firmware/Cargo.toml`을 따르고, 부팅 로그와 점검 화면에 `0.1.0+<커밋>` 형태로 찍힙니다.

## 문서

| 문서 | 내용 |
|---|---|
| [하드웨어](docs/hardware.md) | 사양, 핀맵, LCD·전원 회로, 처음 받았을 때의 상태 |

## 기술 스택

- **Rust std (ESP-IDF)**: [esp-idf-svc](https://github.com/esp-rs/esp-idf-svc) 기반
- **[mipidsi](https://github.com/almindor/mipidsi)**: ST7735S LCD 드라이버 (embedded-hal 1.0)
- **[embedded-graphics](https://github.com/embedded-graphics/embedded-graphics)**: 도형과 글꼴 그리기

## 프로젝트 구조

```
.
├── docs/                    # 문서 (위 표 참고)
├── .github/workflows/ci.yml # push/PR 검증 (fmt + release build + clippy)
└── firmware/                # Rust 펌웨어 (cargo 프로젝트 루트, 빌드는 여기서)
    ├── .cargo/config.toml   # 타겟, 링커, 러너, ESP-IDF 버전 고정
    ├── rust-toolchain.toml  # nightly 고정 + 컴포넌트
    ├── sdkconfig.defaults   # ESP-IDF 설정
    ├── build.rs             # 펌웨어 버전 문자열 (FW_VERSION)
    └── src/
        ├── main.rs          # 하드웨어 점검 (LCD, 버튼, LED)
        └── lcd.rs           # 1.44" ST7735S 패널 모델 (초기화 값, 메모리 크기)
```

## 로드맵

작업은 [GitHub 이슈](https://github.com/barmi/esp32c3-lcd144-keychain/issues) 단위로 진행합니다.

| 단계 | 내용 | 상태 |
|---|---|---|
| 1 | 개발 환경 구축 및 하드웨어 점검 펌웨어 | 완료 |
