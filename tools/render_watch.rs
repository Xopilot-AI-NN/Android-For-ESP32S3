// Модуль предпросмотра настоящим RGB565-рендерером.
//! Host renderer for the exact RGB565 firmware surface; no display mockups.
//! Build through tools/runtime-check/Cargo.toml, bin render-watch (host target).
use watch_runtime_check::material;
use std::{fs, io::Write};
use embedded_sdmmc::{Block, BlockCount, BlockDevice, BlockIdx};
use std::{cell::RefCell, io::{Read, Seek, SeekFrom}};
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
        for block in blocks { file.read_exact(&mut block.contents)?; }
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
    let image = std::env::args().nth(2).map(|path| Image(RefCell::new(fs::File::open(path).unwrap())));
    if let Some(image) = &image {
        watch_runtime_check::ui_resources::mount(image, watch_runtime_check::boot_control::Slot::A)
            .expect("mount system graphics for an accurate preview");
    }
    for (name, screen, cursor) in [
        ("watch", 1, 0),
        ("analog",1,0),
        ("complications",1,0),
        ("power",1,0),
        ("splash",1,0),
        ("fastboot",1,0),
        ("recovery",1,0),
        ("display",14,3),
        ("media-playing",10,1),
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
        ("date-time", 16, 0),
        ("developer", 22, 0),
        ("name-keyboard", 8, 0),
        ("weather", 1, 0),
        ("world", 6, 3),
        ("calendar", 1, 0),
        ("converter", 1, 0),
        ("counter", 1, 0),
        ("new-apps", 2, 11),
        ("calculator", 1, 0),
        ("files", 1, 0),
        ("editor", 1, 0),
        ("tasks", 1, 0),
        ("expenses", 1, 0),
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
        let raw = format!("M3|{screen}|{cursor}|60|0|0|0|0|1|1|0|0|131|0|0|1|1|1|3");
        let mut frame = material::MaterialFrame::parse(&raw).unwrap();
        use watch_runtime_check::apps;
        if name == "calculator" {
            let mut calculator = apps::calculator::Calculator::default();
            for key in [13, 19, 14, 11, 8, 23] { calculator.press(key); }
            frame.app.id = apps::CALCULATOR;
            frame.app.calculator = calculator.view();
            let value = calculator.text().as_bytes();
            frame.app.text[..value.len()].copy_from_slice(value); frame.app.len = value.len() as u8;
        }
        if name == "files" || name == "editor" {
            frame.app.id = if name == "files" { apps::FILES } else { apps::EDITOR };
            let v = &mut frame.app.files;
            v.title_len = apps::files::copy_text(&mut v.title, if name == "files" { "/" } else { "Заметка.txt" });
            v.mode = if name == "files" { 0 } else { 3 };
            v.count = if name == "files" { 6 } else { 5 };
            if name == "files" { v.items = 2; v.selected_len = apps::files::copy_text(&mut v.selected_name, "Документы"); }
            for (i, (row, label)) in v.rows.iter_mut().zip(if name == "files" { ["Документы", "Заметка.txt", "Новый файл"] } else { ["Привет! Это текст на часах.", "Вторая строка документа.", "Добавить строку"] }).enumerate() {
                row.len = apps::files::copy_text(&mut row.text, label); row.selected = i == 0;
                if name == "files" { row.kind = if i == 0 { 2 } else if i == 1 { 1 } else { 0 }; row.size = 124; }
            }
        }
        if name == "tasks" || name == "expenses" {
            frame.app.id = if name == "tasks" { apps::TASKS } else { apps::EXPENSES };
            frame.app.organizer.total = if name == "tasks" { 1 } else { 1750 };
            for (i, (row, label)) in frame.app.organizer.rows.iter_mut().zip(if name == "tasks" { ["[x] Подготовить детали", "[ ] Собрать корпус", "Добавить"] } else { ["12.50 · Продукты", "5.00 · Транспорт", "Добавить"] }).enumerate() {
                row.len = apps::files::copy_text(&mut row.text, label); row.selected = i == 1;
            }
        }
        if name == "focus" { frame.app.id = apps::FOCUS; }
        if name == "phone" { frame.app.id = apps::PHONE; }
        if name == "minimal" { frame.watch_face = 3; }
        if name == "dashboard" { frame.watch_face = 4; }
        if name == "name-keyboard" {
            frame.name_editing = true;
            frame.name_draft[..4].copy_from_slice("ми".as_bytes());
            frame.name_draft_len = 4;
            frame.keyboard.suggestions = ["Минск", "", ""];
            frame.keyboard.focus = (frame.keyboard.character_count() + 7) as u8;
        }
        if name == "weather" { frame.app.id = watch_runtime_check::apps::WEATHER; }
        if name == "world" { frame.app.id = watch_runtime_check::apps::WORLD; }
        if name == "calendar" {
            frame.app.id = watch_runtime_check::apps::CALENDAR;
            frame.app.tools.month = 2026 * 12 + 10;
            frame.app.tools.today = Some((2026, 11, 2));
        }
        if name == "converter" {
            frame.app.id = watch_runtime_check::apps::CONVERTER;
            frame.app.tools.amount = 10;
        }
        if name == "counter" {
            frame.app.id = watch_runtime_check::apps::COUNTER;
            frame.app.tools.count = 42;
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
        frame.time_minutes = 23*60+29; frame.time_seconds=37;
        frame.date_day=1;frame.date_month=11;
        if name=="analog" {frame.watch_face=1;}
        if name=="complications" {frame.watch_face=2;}
        if name=="power" {frame.power_menu=true;}
        if name=="media-playing" {frame.media_known=true;frame.media_playing=true;frame.media_progress=420;frame.animation_phase=100;}
        let mut surface = material::Surface::new();
        if name=="splash" {surface.boot_frame(255);}
        else if name=="fastboot" {surface.boot_menu_frame(1,2);}
        else if name=="recovery" {surface.boot_menu_frame(2,1);}
        else {surface.render(frame);}
        if name == "translucent-status" {
            if let Some(image) = &image {
                watch_runtime_check::ui_resources::service(image);
                surface.render(frame);
            }
        }
        let mut file = fs::File::create(format!("{out}/{name}.ppm")).unwrap();
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
