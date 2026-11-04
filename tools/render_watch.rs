// Модуль предпросмотра настоящим RGB565-рендерером.
//! Host renderer for the exact RGB565 firmware surface; no display mockups.
//! Build through tools/runtime-check/Cargo.toml, bin render-watch (host target).
use embedded_sdmmc::{Block, BlockCount, BlockDevice, BlockIdx};
use std::{
    cell::RefCell,
    io::{Read, Seek, SeekFrom},
};
use std::{fs, io::Write};
use watch_runtime_check::material;
/// Read-only доступ к образу для предпросмотра компонента предпросмотра настоящим RGB565-рендерером.
struct Image(RefCell<fs::File>);
impl BlockDevice for Image {
    /// Причины отказа операций компонента предпросмотра настоящим RGB565-рендерером.
    type Error = std::io::Error;
    /// Возвращает ёмкость блочного устройства в секторах по 512 байт.
    fn num_blocks(&self) -> Result<BlockCount, Self::Error> {
        Ok(BlockCount((self.0.borrow().metadata()?.len() / 512) as u32))
    }
    /// Реализует секторное чтение для данного блочного backend.
    fn read(&self, blocks: &mut [Block], index: BlockIdx) -> Result<(), Self::Error> {
        let mut file = self.0.borrow_mut();
        file.seek(SeekFrom::Start(u64::from(index.0) * 512))?;
        for block in blocks {
            file.read_exact(&mut block.contents)?;
        }
        Ok(())
    }
    /// Реализует секторную запись для данного backend с его ограничениями.
    fn write(&self, _: &[Block], _: BlockIdx) -> Result<(), Self::Error> {
        Err(std::io::Error::other("preview image is read only"))
    }
}

/// Рисует набор проверочных экранов тем же RGB565-рендерером и сохраняет PPM-файлы.
fn main() {
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "out/ui-preview".into());
    fs::create_dir_all(&out).unwrap();
    let image = std::env::args()
        .nth(2)
        .map(|path| Image(RefCell::new(fs::File::open(path).unwrap())));
    if let Some(image) = &image {
        watch_runtime_check::ui_resources::mount(image, watch_runtime_check::boot_control::Slot::A)
            .expect("mount system graphics for an accurate preview");
    }
    for (name, screen, cursor) in [
        ("watch", 1, 0),
        ("analog", 1, 0),
        ("complications", 1, 0),
        ("power", 1, 0),
        ("splash", 1, 0),
        ("splash-start", 1, 0),
        ("splash-mid", 1, 0),
        ("fastboot", 1, 0),
        ("recovery", 1, 0),
        ("display", 14, 3),
        ("media-playing", 10, 1),
        ("quick", 4, 0),
        ("settings", 5, 0),
        ("connectivity", 7, 0),
        ("wifi", 12, 0),
        ("bluetooth", 13, 1),
        ("pairing", 30, 0),
        ("media", 10, 3),
        ("about", 8, 0),
        ("launcher", 2, 2),
        ("settings-scroll", 5, 4),
        ("clock", 11, 0),
        ("clock-hub", 6, 3),
        ("clock-layout-grid", 6, 0),
        ("clock-layout-dock", 6, 0),
        ("date-time", 16, 0),
        ("developer", 22, 0),
        ("name-keyboard", 8, 0),
        ("weather", 1, 0),
        ("world", 6, 3),
        ("calendar", 1, 0),
        ("gallery", 1, 0),
        ("gallery-local", 1, 0),
        ("contacts", 1, 0),
        ("contacts-list", 1, 0),
        ("messages", 1, 0),
        ("photos", 1, 0),
        ("bt-keyboard", 1, 0),
        ("google", 1, 0),
        ("google-calendar", 1, 0),
        ("gmail", 1, 0),
        ("translator", 1, 0),
        ("google-qr", 1, 0),
        ("google-confirm", 1, 0),
        ("google-month", 1, 0),
        ("gmail-list", 1, 0),
        ("gmail-body", 1, 0),
        ("google-results", 1, 0),
        ("new-apps", 2, 11),
        ("calculator", 1, 0),
        ("files", 1, 0),
        ("files-last", 1, 0),
        ("files-toolbar", 1, 0),
        ("files-properties", 1, 0),
        ("share", 1, 0),
        ("share-offline", 1, 0),
        ("flashlight", 29, 0),
        ("memory", 1, 0),
        ("diagnostics", 1, 0),
        ("diagnostics-display",1,0),
        ("diagnostics-render",1,0),
        ("diagnostics-network",1,0),
        ("air-keyboard-full",1,0),
        ("air-keyboard-inline",1,0),
        ("air-keyboard-menu",1,0),
        ("browser",1,0),
        ("browser-page",1,0),
        ("browser-light",1,0),
        ("browser-menu",1,0),
        ("chip-temperature", 1, 0),
        ("editor-long", 1, 0),
        ("phone-pair", 1, 0),
        ("editor", 1, 0),
        ("tasks", 1, 0),
        ("focus", 1, 0),
        ("phone", 1, 0),
        ("minimal", 1, 0),
        ("dashboard", 1, 0),
        ("translucent-status", 1, 0),
        ("timer-presets", 11, 2),
        ("timer-custom", 11, 4),
        ("list-launcher", 26, 6),
        ("tiles", 9, 1),
        ("alarm", 31, 0),
        ("timer-alert", 11, 0),
        ("brightness", 28, 0),
        ("keyboard", 24, 0),
        ("gestures", 18, 0),
        ("security", 20, 1),
        ("pin-entry", 0, 0),
        ("lock-timeout", 20, 0),
    ] {
        let scenario = name;
        let name = match name {
            "files-last" | "files-toolbar" | "files-properties" => "files",
            "editor-long" => "editor",
            "phone-pair" => "phone",
            _ => name,
        };
        let raw = format!("M3|{screen}|{cursor}|60|0|0|0|0|1|1|0|0|131|0|0|1|1|1|3");
        let mut frame = material::MaterialFrame::parse(&raw).unwrap();
        use watch_runtime_check::apps;
        if name == "calculator" {
            let mut calculator = apps::calculator::Calculator::default();
            for key in [13, 19, 14, 11, 8, 23] {
                calculator.press(key);
            }
            frame.app.id = apps::CALCULATOR;
            frame.app.calculator = calculator.view();
            let value = calculator.text().as_bytes();
            frame.app.text[..value.len()].copy_from_slice(value);
            frame.app.len = value.len() as u8;
        }
        if name == "files" || name == "editor" {
            frame.app.id = if name == "files" {
                apps::FILES
            } else {
                apps::EDITOR
            };
            let v = &mut frame.app.files;
            v.title_len = apps::files::copy_text(
                &mut v.title,
                if name == "files" {
                    "/"
                } else {
                    "Заметка.txt"
                },
            );
            v.mode = if name == "files" { 0 } else { 3 };
            v.count = if name == "files" { 8 } else { 9 };
            if name == "files" {
                v.items = 4;
                v.selected_len = apps::files::copy_text(&mut v.selected_name, "Документы");
            }
            for (i, (row, label)) in v
                .rows
                .iter_mut()
                .zip(if name == "files" {
                    ["Проекты", "tasks.txt", "config.json", "photo.png"]
                } else {
                    ["# Задачи", "", "- Собрать проект", "- Проверить GPIO"]
                })
                .enumerate()
            {
                row.len = apps::files::copy_text(&mut row.text, label);
                row.selected = i == 1;
                if name == "files" {
                    row.kind = if i == 0 { 2 } else { 1 };
                    row.size = 124;
                }
            }
        }
        if name == "tasks" {
            frame.app.id = if name == "tasks" {
                apps::TASKS
            } else {
                apps::EXPENSES
            };
            frame.app.organizer.total = if name == "tasks" { 1 } else { 1750 };
            for (i, (row, label)) in frame
                .app
                .organizer
                .rows
                .iter_mut()
                .zip(if name == "tasks" {
                    ["[x] Подготовить детали", "[ ] Собрать корпус", "Добавить"]
                } else {
                    ["12.50 · Продукты", "5.00 · Транспорт", "Добавить"]
                })
                .enumerate()
            {
                row.len = apps::files::copy_text(&mut row.text, label);
                row.selected = i == 1;
            }
        }
        if name == "focus" {
            frame.app.id = apps::FOCUS;
        }
        if name == "gallery" || name == "gallery-local" {
            frame.app.id = apps::GALLERY;
            frame.app.gallery.source = if name == "gallery-local" { 1 } else { 0 };
        }
        if name == "contacts" || name == "contacts-list" {
            frame.app.id=apps::CONTACTS;frame.app.remote.kind=4;
        }
        if name == "photos" {
            frame.app.id=apps::GALLERY;frame.app.gallery.source=2;frame.app.remote.kind=5;
        }
        if name == "contacts-list" || name == "messages" {
            if name == "messages" {frame.app.id=apps::MESSAGES;frame.app.remote.kind=6;}
            let v=&mut frame.app.remote;v.mode=2;v.count=3;v.cursor=1;v.message="Только синтетические данные";
            for (i,line) in ["Алексей","+000 123 456","Мария","fixture@example.invalid","Служба уведомлений","Это тестовый текст"].iter().enumerate() {
                v.lengths[i]=apps::files::copy_text(&mut v.lines[i],line);
            }
        }
        if name == "bt-keyboard" {
            frame.app.id = apps::BT_KEYBOARD;
        }
        for (label, id, kind) in [
            ("google", apps::GOOGLE, 0),
            ("google-calendar", apps::CALENDAR, 1),
            ("gmail", apps::GMAIL, 2),
            ("translator", apps::TRANSLATOR, 3),
        ] {
            if name == label {
                frame.app.id = id;
                frame.app.remote.kind = kind;
                if kind == 1 { frame.app.cursor = 1; }
                frame.app.remote.message = "HTTPS Gateway ещё не настроен";
            }
        }
        if name == "phone" {
            frame.app.id = apps::PHONE;
        }
        if name.starts_with("google-")
            && matches!(
                name,
                "google-qr" | "google-confirm" | "google-month" | "google-results"
            )
            || name.starts_with("gmail-")
        {
            use watch_runtime_check::google_gateway::Phase;
            frame.app.id = if name.starts_with("gmail-") {
                apps::GMAIL
            } else if name == "google-month" {
                apps::CALENDAR
            } else {
                apps::GOOGLE
            };
            if name == "google-month" { frame.app.cursor = 1; }
            let v = &mut frame.app.remote;
            v.kind = if name.starts_with("gmail-") {
                2
            } else if name == "google-month" {
                1
            } else {
                0
            };
            v.online = true;
            v.message = "Обновлено";
            if name == "google-qr" {
                v.mode = 3;
                v.phase = Phase::Waiting as u8;
                use qrcodegen_no_heap::{QrCode, QrCodeEcc, Version};
                let mut temp = [0; 512];
                let mut out = [0; 512];
                let qr = QrCode::encode_text(
                    "https://gateway.example.org/p/fixture-pairing-identifier",
                    &mut temp,
                    &mut out,
                    QrCodeEcc::Medium,
                    Version::MIN,
                    Version::new(10),
                    None,
                    true,
                )
                .unwrap();
                let n = qr.size() as usize;
                v.qr_size = n as u8;
                for y in 0..n {
                    for x in 0..n {
                        if qr.get_module(x as i32, y as i32) {
                            let b = y * n + x;
                            v.qr[b / 8] |= 1 << (b % 8);
                        }
                    }
                }
            }
            if name == "google-confirm" {
                v.mode = 3;
                v.phase = Phase::Confirm as u8;
                v.services = 3;
                let email = "f***@example.invalid";
                v.email[..email.len()].copy_from_slice(email.as_bytes());
                v.email_len = email.len() as u8;
            }
            if name == "google-month" {
                v.mode = 5;
                v.month_valid = true;
                v.month = 2026 * 12 + 10;
                v.days = (1 << 2) | (1 << 18);
            }
            if matches!(name, "gmail-list" | "google-results") {
                v.mode = 2;
                v.count = 3;
                for (i, line) in [
                    "Проверка интеграции",
                    "fixture@example.invalid",
                    "Длинная тема письма",
                    "Только синтетические данные",
                    "События проекта",
                    "Без личной информации",
                ]
                .iter()
                .enumerate()
                {
                    let n = line.len();
                    v.lines[i][..n].copy_from_slice(line.as_bytes());
                    v.lengths[i] = n as u8;
                }
                if name == "google-results" {
                    let label = "DuckDuckGo";
                    v.provider[..label.len()].copy_from_slice(label.as_bytes());
                    v.provider_len = label.len() as u8;
                }
            }
            if name == "gmail-body" {
                v.mode = 4;
                let sample=[
                    "Тема тестового письма",
                    "fixture@example.invalid",
                    "2026-11-03",
                    "Текст читается постранично.",
                    "Сеть не блокирует колёсико.",
                    "HTML не выполняется.",
                    "Это тестовые данные.",
                ].join("\n");
                let lines=watch_runtime_check::apps::remote::wrap(&sample);v.total_lines=lines.len()as u16;
                for (i,line) in lines.iter().take(7).enumerate(){
                    let n = line.len();
                    v.lines[i][..n].copy_from_slice(line.as_bytes());
                    v.lengths[i] = n as u8;
                }
            }
        }
        if name == "minimal" {
            frame.watch_face = 3;
        }
        if name == "dashboard" {
            frame.watch_face = 4;
        }
        if name == "name-keyboard" {
            frame.name_editing = true;
            frame.name_draft[..4].copy_from_slice("ми".as_bytes());
            frame.name_draft_len = 4;
            frame.keyboard.suggestions = ["Минск", "", ""];
            frame.keyboard.focus = (frame.keyboard.character_count() + 7) as u8;
        }
        if name == "weather" {
            frame.app.id = watch_runtime_check::apps::WEATHER;
        }
        if name == "world" {
            frame.app.id = watch_runtime_check::apps::WORLD;
        }
        if name == "calendar" {
            frame.app.id = watch_runtime_check::apps::CALENDAR;
            frame.app.tools.month = 2026 * 12 + 10;
            frame.app.tools.today = Some((2026, 11, 2));
        }

        if name == "translucent-status" {
            frame.wallpaper_enabled = true;
            frame.wallpaper_index = 1;
            frame.wallpaper_level = 100;
        }
        if name == "timer-presets" {
            frame.timer_editor = 1;
        }
        if name == "timer-custom" {
            frame.timer_editor = 2;
        }
        if name == "pin-entry" {
            frame.has_pin = true;
            frame.security_view = 3;
            frame.pin_count = 2;
            frame.pin_digit = 5;
        }
        if name == "lock-timeout" {
            frame.has_pin = true;
            frame.security_view = 5;
            frame.pin_digit = 10;
        }
        frame.bt_status = 3;
        frame.wifi_link_state = 4;
        frame.wifi_ip = Some([10, 214, 249, 238]);
        if name == "timer-alert" {
            frame.clock_alert = 1;
        }
        if screen == 30 {
            frame.bt_pin = Some(123456);
        }
        frame.time_minutes = 14 * 60 + 9;
        frame.time_seconds = 37;
        frame.date_day = 1;
        frame.date_month = 11;
        if name == "analog" {
            frame.watch_face = 1;
        }
        if name == "complications" {
            frame.watch_face = 2;
        }
        if name == "power" {
            frame.power_menu = true;
        }
        if name == "media-playing" {
            frame.media_known = true;
            frame.media_playing = true;
            frame.media_progress = 420;
            frame.animation_phase = 100;
        }
        if scenario.starts_with("diagnostics-") {
            frame.app.id=apps::DIAGNOSTICS;
            frame.app.cursor=match scenario {"diagnostics-display"=>5,"diagnostics-render"=>6,_=>7};
        }
        if scenario.starts_with("air-keyboard") {
            frame.app.id=apps::AIR_KEYBOARD;
            let v=&mut frame.app.air_keyboard;
            let text="Приве\nAir Keyboard";
            v.text[..text.len()].copy_from_slice(text.as_bytes()); v.len=text.len() as u16;
            v.caret="Приве".len() as u16; v.preview=Some('т'); v.selected=28;
            v.inline=scenario=="air-keyboard-inline";
            v.menu=scenario=="air-keyboard-menu";
            v.menu_index=3;
        }
        if scenario.starts_with("browser") {
            frame.app.id=apps::BROWSER;
            if scenario!="browser" {
                let v=&mut frame.app.browser;
                v.mode=if scenario=="browser-menu"{3}else{2};
                v.cursor=3;v.total=24;v.pages=3;v.light=scenario=="browser-light";
                let title="Пример HTML-сайта";v.title[..title.len()].copy_from_slice(title.as_bytes());v.title_len=title.len() as u8;
                for (i,(text,kind,height)) in [("Заголовок",1,31),("Текст сайта на русском языке.",0,18),("• Первый пункт списка",0,18),("Открыть следующую страницу",3,18),("Фото · нажмите для просмотра",4,42)].iter().enumerate(){
                    let row=&mut v.rows[i]; row.text[..text.len()].copy_from_slice(text.as_bytes());row.len=text.len() as u8;row.kind=*kind;row.height=*height;row.selected=i==3;
                }
                v.rows_len=5;
            }
        }
        match scenario {
            "share" | "share-offline" => { frame.app.id = apps::SHARE; if scenario == "share" { frame.wifi_ip = Some([192, 0, 2, 1]); frame.adb = true; } }
            "memory" => frame.app.id = apps::MEMORY,
            "diagnostics" => frame.app.id = apps::DIAGNOSTICS,
            "chip-temperature" => {
                frame.app.id = apps::DIAGNOSTICS;
                frame.app.cursor = 4;
                frame.app.temperature_deci_c = Some(453);
            }
            "files-properties" => { frame.app.files.mode = 10; frame.app.files.title_len = apps::files::copy_text(&mut frame.app.files.title, "Документ.txt"); frame.app.files.selected_size = 4096; }
            _ => {}
        }
        if matches!(scenario, "files-last" | "files-toolbar") {
            let v = &mut frame.app.files;
            for (i, row) in v.rows.iter_mut().enumerate() { row.selected = scenario == "files-last" && i == 3; }
            v.selected_len = apps::files::copy_text(&mut v.selected_name, "photo.png");
            v.toolbar = if scenario == "files-toolbar" { 1 } else { 255 };
        }
        if scenario == "editor-long" {
            let v = &mut frame.app.files;
            v.title_len = apps::files::copy_text(&mut v.title, "Очень длинное имя документа.txt");
        }
        if scenario == "phone-pair" {
            frame.app.phone.pending = true;
            frame.app.phone.code = *b"1234";
        }
        if let Some(theme) = std::env::var("WATCH_PREVIEW_THEME").ok().and_then(|s| s.parse::<u8>().ok()) {
            frame.theme = theme.min(3);
        }
        let mut surface = material::Surface::new();
        if name.starts_with("splash") {
            surface.boot_frame(match name { "splash-start" => 0, "splash-mid" => 15, _ => 31 });
        } else if name == "fastboot" {
            surface.boot_menu_frame(1, 2);
        } else if name == "recovery" {
            surface.boot_menu_frame(2, 1);
        } else if name == "clock-layout-grid" || name == "clock-layout-dock" {
            surface.preview_clock_layout(frame, name == "clock-layout-dock");
        } else {
            surface.render(frame);
        }
        if name == "translucent-status" {
            if let Some(image) = &image {
                watch_runtime_check::ui_resources::service(image);
                surface.render(frame);
            }
        }
        let mut file = fs::File::create(format!("{out}/{scenario}.ppm")).unwrap();
        write!(file, "P6\n240 280\n255\n").unwrap();
        for &p in surface.pixels() {
            file.write_all(&[
                (((p >> 11) & 31) * 255 / 31) as u8,
                (((p >> 5) & 63) * 255 / 63) as u8,
                ((p & 31) * 255 / 31) as u8,
            ])
            .unwrap();
        }
    }
}
