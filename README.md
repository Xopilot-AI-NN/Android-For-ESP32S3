# Zephyr Watch Bootloader v1.6 — AOSP storage + Material SystemUI + ZPager

Current bring-up adds a lock-screen/watchface/launcher Material flow, a real 32 MiB GPT `swap` backing partition for explicit ZPager state paging, a no-reset ZADB developer bridge, and an optional native ESP32-S3 Wi-Fi/BLE radio build. See [`SYSTEM_V03.md`](SYSTEM_V03.md).

Quick development boot:

```bash
./flash.sh st7789 pc
python desktop_viewer.py      # second terminal
python zadb.py dumpsys        # optional third terminal
```

Native radio bring-up:

```bash
./flash.sh st7789 pc radio
```

> ZPager is explicit managed paging for serializable framework/app state, not Linux transparent swap. Standard USB ADB/TCP-5555 WADB transports are still separate milestones; v0.3 provides the ADB service/packet core plus ZADB over the existing no-reset bridge.

---

# Zephyr Watch Bootloader v1.2 — Android-style GPT/A-B/AVB + ST7789V3

Законченный первый этап загрузки для **Waveshare ESP32-S3-Zero-N4R2** из проекта Zephyr Watch.
Основной физический экран — **ST7789V3 240×280**. Дополнительно тот же реальный boot flow можно
видеть и управлять им на ПК через единственный Type-C разъём платы.

Это **не эмулятор ESP32-S3**: bootloader, SD, A/B, SHA-256, Rhai, Recovery и Fastboot выполняются
на настоящем микроконтроллере. ПК является только удалённой поверхностью отображения/ввода.

## Что изменилось

### v1.2 — Android-like storage/boot

- Основной boot path теперь использует raw microSD с GPT вместо FAT `SLOTA/SLOTB`.
- Добавлены AOSP A/B metadata, Android boot/init_boot/vendor_boot v4 и AVB0 hash verification.
- После SD init на 400 кГц SPI2 переключается на 20 МГц для чтения образов.
- Подробный контракт: [`ANDROID_BOOT.md`](ANDROID_BOOT.md).

### v1.1 — display/USB

- **ST7789V3 теперь default display backend**.
- Добавлен `USB Desktop Display` поверх встроенного USB Serial/JTAG ESP32-S3.
- Физический ST7789 и окно на ПК работают одновременно.
- Если дисплей ещё не припаян, USB-окно всё равно показывает boot/recovery/fastboot.
- USB mirror неблокирующий: отсутствие ПК не останавливает загрузку платы.
- Можно подключить viewer уже после старта и нажать `Sync`.
- Из PC viewer можно выбрать Normal / Recovery / Fastboot и имитировать crown.
- В Fastboot+ viewer также показывает консоль и умеет отправлять `help`, `getvar:all`, `continue`, `reboot`.

## Почему это не настоящий DisplayPort Alt Mode

Type-C на ESP32-S3-Zero подключён к native USB ESP32-S3. Сам ESP32-S3 предоставляет USB 2.0
Full-Speed D+/D−, но в нём нет DisplayPort PHY/high-speed lanes и USB-C DP Alt Mode source.
Поэтому программно превратить этот порт в настоящий DisplayPort, как у Android-телефона с DP Alt Mode,
нельзя.

Зато для часов это даже удобнее: UI-команды очень маленькие и отлично помещаются в USB Full-Speed,
а плата сохраняет штатный USB Serial/JTAG для прошивки, отладки и Fastboot+.

## Реальная схема работы

```text
                    ┌──────────── ST7789V3 240×280
                    │             GPIO1..6
                    │
ESP32-S3 boot code ─┼─> BootDisplay
(real hardware)     │
                    └──────────── USB Type-C
                                  native USB Serial/JTAG
                                          │
                                          ▼
                                desktop_viewer.py
                                live watch surface
```

То есть если позже экран будет припаян, на матрице и на ПК будет один и тот же boot state.

## Boot flow

```text
Power ON
  ↓
ESP32-S3 ROM -> Zephyr Rust bootloader
  ↓
ST7789V3 + USB Desktop Surface v2
  ↓
Normal / Recovery / Fastboot+
  ↓
microSD
  ↓
Protective MBR + GPT
  ↓
GPT header CRC32 + partition-array CRC32
  ↓
misc: AOSP bootloader_control
  ↓
select slot by priority / tries_remaining / successful_boot
  ↓
vbmeta_a|b  (AVB0)
  ├─ verify boot_a|b SHA-256 descriptor
  ├─ verify init_boot_a|b SHA-256 descriptor
  └─ verify vendor_boot_a|b SHA-256 descriptor
  ↓
boot image v4 / vendor_boot v4 validation
  ↓
init_boot ramdisk (newc CPIO)
  ↓
/init.rhai
  ↓
Rhai early userspace
  ↓
mark slot successful
  ↓
ANDROID / SYSTEM READY
```

`boot`, `init_boot`, `vendor_boot`, `vbmeta` — физические A/B-разделы, как и должен видеть bootloader.
`super` и `userdata` находятся на той же microSD, но `super` будет разбираться уже userspace-частью
прошивки, а не bootloader'ом — это соответствует разделению ответственности AOSP dynamic partitions.

## Pinout

| Устройство | GPIO |
|---|---:|
| ST7789V3 BL | 1 |
| ST7789V3 CS | 2 |
| ST7789V3 DC | 3 |
| ST7789V3 RST | 4 |
| ST7789V3 MOSI/SDA | 5 |
| ST7789V3 SCK/SCL | 6 |
| SD MISO | 7 |
| SD CLK | 8 |
| SD MOSI | 9 |
| SD CS | 10 |
| Encoder A | 11 |
| Encoder B | 12 |
| Power / crown button | 13 |
| WS2812 RGB LED | 21 |
| Native USB D− | 19 (внутри платы к Type-C) |
| Native USB D+ | 20 (внутри платы к Type-C) |

GPIO19/20 не надо дополнительно паять — они уже относятся к native USB на ESP32-S3-Zero.

> В старой версии README SSD1306 был указан как default. Для твоей сборки это исправлено: default теперь ST7789V3.
> SSD1306 backend оставлен только как опциональный код.

## USB Desktop Display на ПК

### No-reset / asynchronous USB attach

`desktop_viewer.py` v2 больше не открывает CDC-порт обычным `Serial(port, ...)`: pySerial по умолчанию
держит DTR/RTS активными, а переключение этих линий на ESP USB Serial/JTAG может участвовать в reset/download
последовательности. Viewer теперь создаёт **закрытый** serial-объект, заранее выставляет `DTR=false` и
`RTS=false`, только затем открывает `/dev/ttyACM*`. На Linux дополнительно снимается `HUPCL`.

USB работает в отдельном worker thread: окно не зависает, порт автоматически переподключается после
re-enumeration, а `@ZWIN|PING` / `@ZWUI|PONG` проверяют, что firmware жива, без reboot. Даже если
viewer запущен раньше платы, он просто ждёт появления ESP32-S3.


### Arch Linux

```bash
sudo pacman -S python tk python-pyserial
python desktop_viewer.py
```

Либо явно указать порт:

```bash
python desktop_viewer.py --port /dev/ttyACM0
```

После подключения откроется окно 240×280 в стилистике часов. В нём есть:

- текущий экран, который отдала **реальная ESP32-S3**;
- Normal;
- Recovery;
- Fastboot;
- crown left/right;
- crown press;
- Fastboot+ console.

Если окно открылось уже после загрузки, нажми **Sync**.

### Linux permissions

Если `/dev/ttyACM0` недоступен текущему пользователю, проверь группу устройства:

```bash
ls -l /dev/ttyACM0
```

Обычно достаточно правил udev/группы дистрибутива; запускать viewer от root не рекомендуется.

## USB протокол Remote Surface v2

Плата → ПК:

```text
@ZWUI|HELLO|2|240|280|Zephyr Watch|ESP32-S3-Zero-N4R2
@ZWUI|BOOT
@ZWUI|PROGRESS|CHECKING SD|0
@ZWUI|STATUS|ANDROID|SYSTEM READY|a|LIVE USB
@ZWUI|ERROR|NO SYSTEM|USB FASTBOOT
@ZWUI|PONG|2
```

ПК → плата:

```text
@ZWIN|SYNC
@ZWIN|NORMAL
@ZWIN|RECOVERY
@ZWIN|FASTBOOT
@ZWIN|ROTATE|-1
@ZWIN|ROTATE|1
@ZWIN|POWER
```

Протокол line-based специально сделан совместимым с существующим Fastboot+ на том же CDC порту.

### Почему сейчас передаются UI-команды, а не полный RGB framebuffer

Bootloader рисует небольшой набор экранов. Гонять 240×280×RGB565 целиком через USB на каждый кадр
бессмысленно и тратит полосу/память. Поэтому v1 передаёт **команды состояния**, а PC viewer рисует
ровно ту же композицию.

Для полноценного будущего Wear-like UI следующий логичный уровень — `Remote Surface v2`:

```text
DIRTY TILE -> RGB565/RLE -> USB -> PC
```

То есть только изменённые тайлы framebuffer. Логика и рендер-команды всё равно будут выполняться
на микроконтроллере, поэтому это останется hardware-in-the-loop, а не эмулятором.

## Сборка и прошивка — ST7789V3 default

```bash
cargo install espup cargo-espflash
espup install
source "$HOME/export-esp.sh"
./flash.sh
```

Или вручную:

```bash
cargo +esp build --release
cargo espflash flash --release --monitor --chip esp32s3
```

`DISPLAY=st7789` теперь не нужен — это default.

SSD1306 всё ещё можно собрать явно:

```bash
DISPLAY=ssd1306 ./flash.sh
```

## microSD — Android-like raw GPT image

FAT32 больше не является основным форматом системы. Карта содержит настоящую GPT:

```text
misc
metadata
boot_a
boot_b
init_boot_a
init_boot_b
vendor_boot_a
vendor_boot_b
vbmeta_a
vbmeta_b
super
userdata
```

Собрать тестовый/установочный образ:

```bash
python tools/mk_android_sd.py \
  --output out/zephyr-watch-sd.img \
  --size-mib 512 \
  --version 0.2.0
```

Проверить образ перед записью:

```bash
python tools/inspect_android_sd.py out/zephyr-watch-sd.img
```

Затем образ можно записать на microSD обычным `dd`/USB Imager. Будь внимателен с выбором устройства.

`init_boot_*` использует Android boot image header v4 и содержит uncompressed `newc` CPIO с
`/init.rhai`. `boot_*` тоже имеет `ANDROID!` header v4. `vendor_boot_*` имеет `VNDRBOOT` v4.
`vbmeta_*` использует реальный контейнер `AVB0` и hash descriptors для `boot`, `init_boot`,
`vendor_boot`. После инициализации карты на 400 кГц загрузчик переключает SPI2 на 20 МГц.

Точный boot ABI и текущие ограничения описаны в [`ANDROID_BOOT.md`](ANDROID_BOOT.md).

### AVB state

Сейчас development image использует `AVB_ALGORITHM_TYPE_NONE`, но SHA-256 descriptors реально
проверяются bootloader'ом. Поэтому состояние отображается как **AVB ORANGE**, а не как secure/green.
Полноценная production-защита требует RSA-подписи vbmeta + доверенного ключа/rollback state во
внутренней защищённой памяти/eFuse ESP32-S3.

Старый FAT-вариант `SLOTA/SLOTB` сохранён только в `legacy/fat-slot-v1/` и больше не входит в normal boot.

## Fastboot+

В Fastboot можно попасть тремя способами:

1. удерживать физическую кнопку/crown GPIO13 на старте;
2. нажать **Fastboot** в `desktop_viewer.py` во время boot window;
3. после `SYSTEM READY` нажать **Fastboot** в viewer.

Viewer уже содержит основные Fastboot-кнопки. Не открывай `fastboot_tool.py` и `desktop_viewer.py`
одновременно на одном `/dev/ttyACM*`, потому что один serial port обычно может принадлежать одному процессу.

Отдельный CLI остаётся доступен:

```bash
python fastboot_tool.py devices
python fastboot_tool.py getvar:all
python fastboot_tool.py getvar:slot-count
python fastboot_tool.py getvar:has-slot:boot
python fastboot_tool.py pins
python fastboot_tool.py memory
python fastboot_tool.py continue
python fastboot_tool.py reboot
```

> Текущий **Fastboot+** — bring-up/debug протокол поверх USB Serial/JTAG. Он уже сообщает A/B/GPT/AVB capabilities, но бинарный Android Fastboot transport и `flash <partition>` ещё не реализованы; это отдельно отмечено в `ANDROID_BOOT.md`.

## Rhai

Rhai 1.25.1 используется как `no_std` early userspace. `/init.rhai` извлекается из `init_boot` CPIO после AVB-проверки и должен вернуть `0`.

```rhai
boot_version();
device_width();
device_height();
```

## Важное про аппаратную проверку

В этом окружении нет подключённой твоей ESP32-S3 и ESP Rust Xtensa toolchain, поэтому физический flash
из архива здесь выполнить нельзя. Python viewer проверяется локально через `py_compile`, shell-скрипт — через
`bash -n`; Rust-код подготовлен под тот же `esp-hal 1.0.0`, который использовал предыдущий bootloader.

Лицензия: GPL-3.0.

### Solder-free PC block boot

For development before the microSD socket is available, the bootloader can use
a raw GPT image served by the attached PC over USB-Serial-JTAG:

```bash
./flash.sh st7789 pc
python usb_block_server.py --image out/zephyr-watch-sd.img
```

See `USB_PC_BOOT.md`. This development mode uses the exact same GPT/A-B/AVB boot
pipeline as microSD. Native USB host/hub/flash-drive support is documented in
`USB_HOST_FUTURE.md` and requires suitable USB power/role hardware on the final
board.
