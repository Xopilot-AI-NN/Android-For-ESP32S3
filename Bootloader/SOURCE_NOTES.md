# Что изменено относительно текущей ветки Bootloader

Эта версия сделана как самостоятельная замена содержимого bootloader-проекта, а не как набор
разрозненных патчей.

- Удалена зависимость boot flow от отсутствующего `src/vendor/*`.
- `probe_sd_card() -> NoResponse` заменён настоящим SPI SD init + FAT16/FAT32.
- Диагностический бесконечный цикл заменён цепочкой boot -> verify -> Rhai hand-off.
- Добавлен A/B filesystem fallback.
- Добавлена обязательная SHA-256 проверка `BOOT.RHA` из manifest.
- Rhai оставлен основной VM; MicroPython не используется.
- Добавлены отдельные SSD1306 и ST7789V3 backend'ы.
- Добавлены Android/Pixel-Watch-like splash, progress, recovery и boot-error экраны.
- Fastboot+ оставлен текстовым протоколом поверх встроенного USB Serial/JTAG ESP32-S3.
- Low-level CPU/GPU/TSENS экспериментальные части старой ветки не включены в critical boot path:
  загрузчик должен сначала быть максимально предсказуемым, а эти компоненты лучше вернуть после
  аппаратной проверки как независимые модули.

`SLOTA` сейчас является основным early-userspace, `SLOTB` — минимальным fallback. Это ещё не
криптографически подписанный Android Verified Boot: SHA-256 обнаруживает повреждение, но не защищает
от намеренной подмены карты. Для secure boot следующим этапом нужны подпись manifest и eFuse/secure-boot
политика ESP32-S3.

## v1.1 USB Desktop Display

- ST7789V3 240×280 теперь выбран по умолчанию.
- Добавлен `src/desktop.rs`: двунаправленный Remote Surface v2 через native USB Serial/JTAG.
- BootDisplay одновременно рисует на физическом ST7789 и отправляет компактное состояние на ПК.
- Передача board -> PC неблокирующая, поэтому отсутствие host не стопорит boot.
- PC -> board поддерживает SYNC, Normal, Recovery, Fastboot и crown events.
- Добавлен `desktop_viewer.py` — host-side окно, но выполнение системы остаётся на реальном ESP32-S3.
- Настоящий DisplayPort Alt Mode не реализуется: ESP32-S3 имеет USB 2.0 FS D+/D−, но не DP Alt Mode PHY/lanes.

## 2026-09-25: Rhai no_std build fix

- Rhai dependency raised from 1.24.0 to 1.25.1.
- Reason: Rhai 1.24.0 procedural plugin macros can fail in `no_std` builds with repeated
  `E0433: could not find rhai in the crate root` errors. Rhai 1.25.x changed these macros
  so standard Rhai types no longer need to be imported by the expansion; 1.25.1 is used
  instead of 1.25.0 because it restores exports accidentally removed in 1.25.0.


## v1.1 USB async changes

- Host viewer opens serial with DTR/RTS inactive before `open()`.
- Linux viewer clears `HUPCL` when supported.
- Background auto-reconnect follows USB re-enumeration.
- Added `@ZWIN|PING` / `@ZWUI|PONG|2`.
- Board RX now queues multiple commands received in one USB packet.
- `SYNC` in Fastboot also re-sends protocol HELLO.
