# Архитектура часов

Проект использует похожее на Android разделение ответственности: загрузка,
аппаратный слой, системные службы, framework, SystemUI и приложения. Это
прошивка ESP32-S3 с собственным интерфейсом, а не порт Linux/Android/ART.

| Слой | Исходники | Ответственность |
|---|---|---|
| ROM → второй этап ESP-IDF | ROM платы, загрузчик из espflash | Проверить flash-образ ESP и передать управление прошивке |
| Загрузочная библиотека | [Bootloader/src](Bootloader/src), [Cargo.toml](Bootloader/Cargo.toml) | SD-транспорт, GPT, A/B, AVB, boot/init_boot, CPIO и liblp; извлечение системного образа |
| Инициализация прошивки | [Firmware/src/main.rs](Firmware/src/main.rs), [init.rs](Firmware/src/init.rs) | Инициализация памяти/платы и запуск системного цикла |
| HAL | [Firmware/src/hal](Firmware/src/hal) | Дисплей, подсветка, энкодер и аппаратные адаптеры |
| Системные службы | [Firmware/src/services](Firmware/src/services) | Накопитель документов, радио, сеть, ADB, RTC, настройки и фоновые запросы |
| Framework | [runtime.rs](Firmware/src/runtime.rs), [material.rs](Firmware/src/material.rs), [system/framework](Firmware/system/framework) | Rhai VM, состояние, общие графические примитивы, кадры и системный ABI |
| SystemUI | [App/system/priv-app/SystemUI](App/system/priv-app/SystemUI) | Блокировка, уведомления, быстрые настройки, маршрутизация ввода и переходов |
| Приложения | [App/system/app](App/system/app), [каталог](App/catalog.json) | Контроллеры, экраны и манифесты приложений |

Загрузочная библиотека является отдельным Rust crate. Зависимость направлена
от Firmware к Bootloader. Загрузчик не импортирует приложения, рендерер,
Rhai VM, Wi-Fi или BLE. Поддержка ESP-RTOS принадлежит прошивке:
[Firmware/platform](Firmware/platform).

После аппаратного запуска прошивка вызывает загрузочную библиотеку для чтения
GPT-носителя SD/PC. Библиотека возвращает проверенные данные, затем framework
запускает SystemUI и обрабатывает события приложений. Чтение системного носителя
после запуска ESP и первичная загрузка ESP из flash — разные этапы.

## Пакет приложения

Например, [Files/main.rs](App/system/app/Files/main.rs) содержит настоящий
контроллер файлового менеджера, [Files/view.rs](App/system/app/Files/view.rs) —
его интерфейс, [manifest.json](App/system/app/Files/manifest.json) — идентификатор,
точку входа, native ID и список исходников. Пустые описания main.rhai удалены.
Редактор использует общий документный контроллер Files, но операции редактирования
живут в [TextEditor/main.rs](App/system/app/TextEditor/main.rs).

В каталоге есть все 18 приложений меню и 3 системных пакета. Мировое время —
компонент Clock. Связь, сведения об устройстве, диагностика и память — компоненты
Settings. Общая реализация не копируется в каждый пакет: зависимости указаны
в sharedSources манифеста. Общие события и представления приложений собраны
в [App/src/lib.rs](App/src/lib.rs).

[Сборщик образов](Firmware/tools/build_firmware.py) берёт пакеты только из App,
проверяет их исходники/ID/соответствие меню и помещает их в system. Второй копии
пакетов в исходном Firmware/system нет. Native package manager и ADB перечисляют
те же пакеты. Проверки: [test_app_packages.py](Firmware/tools/test_app_packages.py).

## Сборка и исполняемые файлы

```sh
# Исполняемая прошивка, выполнять из Firmware:
cargo +esp build --release --offline

# Отдельные flash-сегменты без записи устройства, выполнять из корня:
bash Bootloader/build_image.sh

# Системный GPT-образ с пакетами и ресурсами:
./Firmware/build.sh

# Полная сборка для PC без прошивки устройства:
./flash pc --build-only
```

Flash-файлы находятся в Bootloader/out/flash-bundle:

| Адрес | Файл | Содержимое |
|---|---|---|
| 0x00000 | bootloader.bin | Реальный второй этап ESP-IDF из espflash, дополненный до границы сегмента |
| 0x08000 | partition-table.bin | Таблица внутренней flash из Bootloader/partitions.csv |
| 0x10000 | firmware.bin | Исполняемый образ Firmware |

flash.bin объединяет эти сегменты; manifest.json содержит адреса, размеры и SHA-256.
Системный образ aosp-wear-sd.img — отдельный носитель для SD/PC, не прошивка SPI flash.

**Текущее ограничение:** нативные Rust-приложения статически компонуются с Firmware.
Их исходники разделены, но изменение контроллера требует пересборки firmware.bin.
Манифест не превращает Rust-исходники в загружаемый код. Независимая установка
приложений, отдельные процессы, ART, APK, Binder и изоляция памяти здесь
не реализованы. Rhai сейчас исполняет системную навигацию. Добавление собственного
интерпретируемого формата приложений — отдельная задача.

## Проверки на ПК

```sh
PYTHONPATH=Firmware/tools /usr/bin/python -m unittest discover -s Firmware/tools -p test_app_packages.py
CARGO_ENCODED_RUSTFLAGS= WATCH_TEST_IMAGE="$PWD/Firmware/out/app-architecture-build/aosp-wear-sd.img" \
  cargo +stable test --manifest-path Firmware/tools/runtime-check/Cargo.toml \
  --target x86_64-unknown-linux-gnu --offline -- --include-ignored --test-threads=1
```

Host-стенд использует те же контроллеры и рендерер. Проверки на ПК не заменяют
запуск на плате. Этот перенос не прошивал и не перезапускал подключённые часы.

## Раннее восстановление

До SD/PC-диска выполняется общее окно выбора колёсиком: + Fastboot, − Recovery.
Оба сервисных меню встроены во внутреннюю flash, TIMG1 следит за непрерывным
удержанием GPIO13 в течение 10 секунд независимо от основного цикла. Ошибки
загрузки передают USB в Fastboot+, сохраняя владение транспортом. Подробные
команды обновления и ограничения: [DELIVERY](Bootloader/DELIVERY.md).
