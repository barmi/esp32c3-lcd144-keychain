# 하드웨어

Spotpear **ESP32-C3 1.44inch LCD Mini TV** (판매명 "ESP32 C3 desktop trinket Mini TV Portable Pendant", 모델 `ESP32C3-1.44`). 키링 고리가 달린 작은 보드에 1.44" TFT LCD, 버튼, 리튬 배터리 충전 회로가 들어 있다.

- [제품 페이지](https://spotpear.com/shop/ESP32-C3-desktop-trinket-Mini-TV-Portable-Pendant-LVGL-1.44inch-LCD-ST7735.html)
- [위키](https://spotpear.com/wiki/ESP32-C3-desktop-trinket-Mini-TV-Portable-Pendant-LVGL-1.44inch-LCD-ST7735.html) (데모 코드, 공장 펌웨어, 빌드 설명)
- [회로도 PDF](https://cdn.static.spotpear.com/uploads/picture/product/esp32/ESP32C3-1.44/Schematic/ESP32-C3-1.44inch.pdf)
- [제조사 데모 코드](https://github.com/Spotpear/ESP32C3_1.44inch) (Arduino + TFT_eSPI 2.5.0)

## 사양

| 항목 | 내용 | 근거 |
|---|---|---|
| MCU | ESP32-C3 (QFN32, 내장 플래시 없음) rev v0.4, RISC-V 싱글코어 160MHz, Wi-Fi 2.4GHz + BLE 5 | esptool |
| 크리스털 | 40MHz | esptool |
| 플래시 | Winbond W25Q128 **16MB** (외장, JEDEC ID `EF 4018`) | esptool, 회로도 U1 |
| USB | 마이크로 5핀 → 칩 내장 USB-Serial/JTAG (VID:PID `303A:1001`). USB-UART 칩 없음 | 실측 |
| LCD | 1.44" ST7735S, 128×128, 4선 SPI | 회로도 T1 |
| 버튼 | K1, K2, BOOT, RESET | 회로도 |
| LED | 빨강 1개 (GPIO11), 충전 표시 빨강 1개 (충전 IC가 직접 구동) | 회로도 |
| 전원 | 3.7V 리튬 배터리 (1.27mm 커넥터), PL4054 충전, CAT6219 3.3V LDO | 회로도, 제품 페이지 |

USB 단자를 아래로 두고 화면을 볼 때 K1·K2는 위쪽 가장자리(왼쪽 K1, 오른쪽 K2), RESET·BOOT는 아래쪽 가장자리(왼쪽 RESET, 오른쪽 BOOT)에 있다. 확장 헤더는 위쪽, LCD FPC는 오른쪽이다.

## 핀맵

| GPIO | 연결 | 비고 |
|---|---|---|
| 0 | LCD DC (RS) | |
| 1 | 헤더 `IO1` | ADC1_CH1 |
| 2 | LCD CS | 부팅 스트래핑 핀 |
| 3 | LCD SCK | |
| 4 | LCD MOSI (SDA) | JTAG MTMS 핀 |
| 5 | LCD RST | JTAG MTDI 핀. `PinDriver` 전에 GPIO 기능을 골라야 한다 (아래) |
| 6 | 헤더 `IO6` | JTAG MTCK 핀 |
| 7 | 헤더 `IO7` | JTAG MTDO 핀 |
| 8 | K1 (active-low) | 부팅 스트래핑 핀 |
| 9 | BOOT (active-low) | 부팅 스트래핑 핀. 누른 채 리셋하면 다운로드 모드 |
| 10 | K2 (active-low) | |
| 11 | 빨간 LED (1kΩ, active-high) | 원래 VDD_SPI 핀. eFuse `VDD_SPI_AS_GPIO`가 켜져 있어 GPIO로 쓸 수 있다 |
| 12–17 | SPI 플래시 | 사용 불가 |
| 18, 19 | USB D−, D+ | USB-Serial/JTAG |
| 20 | 헤더 `IO20` (U0RXD) | |
| 21 | 헤더 `IO21` (U0TXD) | |

- 버튼 세 개는 모두 `GPIO ──[스위치]── GND` 배선이고 외부 풀업이 없다. 내부 풀업을 켜야 한다.
- GPIO4~7은 JTAG 핀이라 칩이 리셋되면 IO MUX 기능이 JTAG로 돌아간다. ESP-IDF SPI 드라이버는 GPIO 기능을 직접 고르지만 esp-idf-hal의 `PinDriver`는 방향만 바꾼다. 그래서 GPIO5(LCD RST)와 헤더 IO6·IO7을 `PinDriver`로 쓸 때는 먼저 `gpio_reset_pin()`으로 GPIO 기능을 고른다 ([#2](https://github.com/barmi/esp32c3-lcd144-keychain/issues/2)).
- RESET은 CHIP_EN(10kΩ 풀업 + 100nF)을 GND로 당긴다.
- 헤더 실크 인쇄는 `IO 1 6 7 20 21 GND`이고, 아래쪽 가장자리에 `3V3`, `5V` 패드가 따로 있다.

## LCD

- 컨트롤러는 ST7735S, SPI 모드 0. 제조사 데모는 SPI 40MHz로 구동한다.
- **백라이트는 끌 수 없다.** LEDA가 3.3V에, LEDK가 5.1Ω(R11)을 거쳐 GND에 바로 붙어 있어 전원이 있는 동안 항상 켜져 있고 밝기도 조절할 수 없다.
- 제조사 데모(TFT_eSPI)의 설정은 `ST7735_DRIVER`, `ST7735_GREENTAB3`, `TFT_RGB_ORDER TFT_BGR`, `TFT_INVERSION_ON`, `setRotation(2)`이다. GREENTAB3의 오프셋은 회전 없는 방향(MADCTL에 MX·MY 없음)에서 (2, 1), 180° 방향(MX|MY)에서 (2, 3)이다.
- [CircuitPython 보드 정의](https://github.com/adafruit/circuitpython/tree/main/ports/espressif/boards/spotpear_esp32c3_lcd_1_44)는 MADCTL `0xC8`, 오프셋 (2, 1), 반전 없음으로 되어 있어 제조사 설정과 다르다. MCU도 ESP32-C3FH4(내장 4MB)로 적혀 있어, 외장 16MB 플래시를 쓰는 이 보드와는 다른 판일 수 있다.

### 실기 확인 결과 (2026-10-18)

- mipidsi에 들어 있는 `ST7735s` 모델로 초기화하면 명령은 오류 없이 들어가지만, 화면은 백라이트만 켜진 흰색으로 남는다. CircuitPython 보드 정의의 초기화 순서(Adafruit ST7735R 순서)로는 화면이 나온다. 둘은 전원 설정(PWCTR1~5)과 감마 값이 다르다. 그래서 펌웨어는 패널 전용 모델(`firmware/src/lcd.rs`)을 쓴다.
- 색 반전 없이 검정이 검정으로 나온다. 제조사 설정의 `TFT_INVERSION_ON`과 다르다.
- MADCTL `0xC8`(MX|MY)에 오프셋 (2, 1)을 쓰면, USB 단자를 아래로 둔 기준으로 오른쪽 끝에 2px 흰 줄이 남는다. 뒤집힌 방향의 행 오프셋은 GREENTAB3처럼 3이어야 하므로, 패널은 132×132 메모리처럼 동작한다. mipidsi 내장 모델은 메모리를 132×162로 보고 뒤집힌 방향의 오프셋을 계산하므로, 전용 모델은 메모리를 132×132로 둔다.
- MADCTL `0xC8`에서는 화면 위쪽이 USB 단자를 아래로 둔 기준 왼쪽에 보인다. 그러므로 패널 기본 방향(MADCTL `0x00`, 제조사 데모의 `setRotation(2)`)은 USB 단자를 오른쪽에 두어야 위쪽이 위로 온다.
- 펌웨어는 USB 단자를 아래로 두고 쓰도록 270° 돌린다(mipidsi `Rotation::Deg270`, MADCTL `0xA8`). 이 방향에서 화면이 똑바로 보이고, BGR 색 순서도 맞다.
- K1, K2, BOOT 버튼이 회로도의 GPIO(8, 10, 9)대로 입력되고, 빨간 LED(GPIO11)도 켜진다.
- LCD RST(GPIO5)를 GPIO 기능으로 바꾸지 않으면 리셋 출력이 핀에 나가지 않는다. 이 선은 풀업·풀다운 없이 떠 있어서, 앞서 돈 펌웨어가 HIGH로 올려 둔 전압이 남아 있는 동안에는 화면이 나왔다. RESET 버튼으로 재시작하면 LCD가 리셋 상태에 묶여 흰 화면으로 남았다 ([#2](https://github.com/barmi/esp32c3-lcd144-keychain/issues/2)).

## 전원

- USB 5V는 1N4148(D1)을 거쳐 5V 레일로, 배터리는 APM2307 P-MOSFET를 거쳐 같은 레일로 들어간다. USB가 꽂히면 MOSFET 게이트(R51 1kΩ / R52 10kΩ)가 올라가 배터리 경로가 끊긴다.
- 5V 레일은 CAT6219 LDO로 3.3V를 만든다. LDO의 EN이 입력에 묶여 있어 항상 켜져 있다.
- 충전은 PL4054(TP4054 계열 선형 충전 IC)가 맡고, 충전 중에는 USB 단자 옆 빨간 LED가 켜진다.
- **전원 스위치가 없다.** 배터리가 연결되어 있으면 항상 켜져 있다.
- **배터리 전압 측정 회로가 없다.** 잔량을 보려면 헤더 `IO1`(ADC1_CH1)에 분압 저항을 달아야 한다.
- 백라이트가 계속 켜져 있으므로 칩을 딥슬립시켜도 배터리 소모가 크게 줄지 않는다.

## 처음 받았을 때의 상태

2026-10-18에 esptool/espefuse로 읽은 결과.

- 보안: Secure Boot와 플래시 암호화가 꺼져 있고, JTAG과 USB 다운로드 모드가 열려 있다. 제한 없이 다시 플래시할 수 있다.
- eFuse `VDD_SPI_AS_GPIO` = 1 (GPIO11을 LED에 쓰려고 공장에서 켠 것으로 보인다).
- 들어 있던 펌웨어는 제조사 날씨 데모 `MiNiApp V1.0`을 arduino-esp32 3.1.x로 빌드한 것이었다. 파티션은 4MB용 `huge_app` 배치라 16MB 중 4MB만 쓰고 있었다.
- 이 펌웨어는 부팅하자마자 `Store access fault`(MTVAL=0x10)로 죽고 1초 남짓마다 재부팅했다. LCD를 초기화하기 전에 죽으므로 화면에는 백라이트만 켜진 흰색이 보였다.
  - 원인: 데모에 들어 있는 TFT_eSPI 2.5.0은 SPI 레지스터 주소를 `REG_SPI_BASE(SPI2_HOST)`로 구한다. arduino-esp32 3.x(ESP-IDF 5.x)의 ESP32-C3 헤더에서 이 매크로는 인자가 2일 때만 실제 주소를 주고 그 밖에는 0을 준다. `SPI2_HOST`는 1이므로 0x10 번지에 쓰다가 죽는다.
- 이 펌웨어에는 Wi-Fi SSID·비밀번호와 OpenWeatherMap API 키가 평문으로 들어 있다. 플래시 덤프는 저장소에 넣지 않는다 (`.gitignore`의 `*.bin`).
