# Прошивка часов 17.2.1

Исполняемая прошивка ESP32-S3: [точка входа](src/main.rs),
[инициализация](src/init.rs), [HAL](src/hal), [службы](src/services),
[framework/runtime](src/runtime.rs) и [рендерер](src/material.rs).

Загрузочная библиотека подключена отдельной зависимостью из Bootloader.
Код приложений и SystemUI берётся из App. [Полная архитектура](../ARCHITECTURE.md).

```sh
cargo +esp build --release --offline  # firmware ELF для PC-режима
./build.sh                           # системный GPT-образ SD/PC
```

Исполняемый файл: target/xtensa-esp32s3-none-elf/release/firmware.
Дисковый образ: out/target/product/zero/aosp-wear-sd.img.
Это прошивка ESP32, без ядра Linux и ART. Нативные приложения пока связаны
с ней статически. Системные образы используют Android-подобные GPT/A-B/AVB/liblp.
