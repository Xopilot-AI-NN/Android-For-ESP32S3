#[path = "catalog.rs"]
pub mod catalog;
#[path = "../system/app/Gallery/main.rs"]
pub mod gallery;
#[path = "../system/app/Gmail/main.rs"]
pub mod gmail;
#[path = "../system/app/GoogleCalendar/main.rs"]
pub mod google_calendar;
#[path = "../system/app/GoogleSearch/main.rs"]
pub mod google_search;
#[path = "../system/app/BluetoothKeyboard/main.rs"]
pub mod keyboard_app;
#[path = "../shared/remote.rs"]
pub mod remote;
#[path = "../system/app/Translator/main.rs"]
pub mod translator;
// Модуль нативных приложений.
// Native utilities use an overlay, preserving the on-card framework screen ABI.
#[path = "../system/app/Calculator/main.rs"]
pub mod calculator;
pub use crate::storage_info as storage;
#[path = "../system/app/Files/main.rs"]
pub mod files;
#[path = "../system/app/Tasks/main.rs"]
pub mod organizer;
#[path = "../shared/tools.rs"]
pub mod utilities;
use crate::runtime::RuntimeEvent;
/// Стабильный ID приложения NONE; сохраняется при перестановке меню.
pub const NONE: u8 = 255;
pub const GALLERY: u8 = 23;
pub const BT_KEYBOARD: u8 = 24;
pub const GOOGLE: u8 = 25;
pub const GOOGLE_CALENDAR: u8 = 26;
pub const GMAIL: u8 = 27;
pub const TRANSLATOR: u8 = 28;
/// Стабильный ID приложения CALCULATOR; сохраняется при перестановке меню.
pub const CALCULATOR: u8 = 6;
/// Стабильный ID приложения FILES; сохраняется при перестановке меню.
pub const FILES: u8 = 7;
/// Стабильный ID приложения SHARE; сохраняется при перестановке меню.
pub const SHARE: u8 = 8;
/// Стабильный ID приложения DIAGNOSTICS; сохраняется при перестановке меню.
pub const DIAGNOSTICS: u8 = 9;
/// Стабильный ID приложения MEMORY; сохраняется при перестановке меню.
pub const MEMORY: u8 = 11;
/// Стабильный ID приложения WEATHER; сохраняется при перестановке меню.
pub const WEATHER: u8 = 12;
/// Стабильный ID приложения WORLD; сохраняется при перестановке меню.
pub const WORLD: u8 = 13;
// ID 14 зарезервирован после удаления «Дыхание»; новые приложения его не занимают.
/// Стабильный ID приложения CALENDAR; сохраняется при перестановке меню.
pub const CALENDAR: u8 = 15;
/// Стабильный ID приложения CONVERTER; сохраняется при перестановке меню.
pub const CONVERTER: u8 = 16;
/// Стабильный ID приложения COUNTER; сохраняется при перестановке меню.
pub const COUNTER: u8 = 17;
/// Стабильный ID приложения EDITOR; сохраняется при перестановке меню.
pub const EDITOR: u8 = 18;
/// Стабильный ID приложения TASKS; сохраняется при перестановке меню.
pub const TASKS: u8 = 19;
/// Стабильный ID приложения EXPENSES; сохраняется при перестановке меню.
pub const EXPENSES: u8 = 20;
/// Стабильный ID приложения FOCUS; сохраняется при перестановке меню.
pub const FOCUS: u8 = 21;
/// Стабильный ID приложения PHONE; сохраняется при перестановке меню.
pub const PHONE: u8 = 22;
/// Количество элементов соответствующей таблицы/меню.
pub const COUNT: u8 = 19;
/// Stable IDs survive launcher reordering and saved system images.
pub const LAUNCHER: [u8; COUNT as usize] = catalog::LAUNCHER;
/// Преобразует позицию меню в стабильный идентификатор приложения.
pub fn launcher_id(index: u8) -> u8 {
    LAUNCHER.get(usize::from(index)).copied().unwrap_or(NONE)
}
/// Названия приложений по их стабильным ID.
pub const LABELS: [&str; 29] = catalog::LABELS;
/// Возвращает системный экран для приложения, если оно использует старый ABI.
pub fn route(index: u8) -> Option<u8> {
    catalog::COMPONENTS
        .get(usize::from(index))
        .filter(|entry| entry.id == index && !entry.package.is_empty())
        .and_then(|entry| entry.screen)
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Снимок данных для отображения компонента нативных приложений.
pub struct View {
    pub id: u8,
    pub gallery: gallery::View,
    pub keyboard_app: keyboard_app::View,
    pub remote: remote::View,
    /// Индекс текущей цели навигации.
    pub cursor: u8,
    /// Текстовое содержимое в ограниченном буфере.
    pub text: [u8; 32],
    /// Число значащих байтов в соответствующем текстовом буфере.
    pub len: u8,
    /// Время работы устройства с начала запуска.
    pub uptime: u64,
    /// Свободная память, КиБ.
    pub free_kib: u32,
    /// Свободная внутренняя RAM, КиБ.
    pub internal_kib: u32,
    /// Время/интервал script в микросекундах.
    pub script_us: u64,
    /// Число повторно использованных результатов кэша.
    pub cache_hits: u32,
    /// Снимок настроек и доступности страничного хранилища.
    pub memory: crate::swap_config::Snapshot,
    /// Последний проверенный погодный снимок.
    pub weather: crate::weather::Snapshot,
    /// Состояние ввода города.
    pub weather_editor: crate::weather::EditorView,
    /// Выбранный часовой пояс мирового времени, часы относительно UTC.
    pub world_offset: i8,
    /// Прошедшее время дыхательной сессии, секунды.
    pub breathe_elapsed: u16,
    /// Признак активной дыхательной сессии.
    pub breathing: bool,
    /// Состояния календаря, конвертера и счётчика.
    pub tools: utilities::Tools,
    /// Состояние проводника и текстового редактора.
    pub files: files::View,
    /// Состояние выражения и результата калькулятора.
    pub calculator: calculator::View,
    /// Снимок задач или расходов для интерфейса.
    pub organizer: organizer::View,
    /// Индекс выбранной клавиши.
    pub focus: organizer::FocusView,
    /// Состояние подтверждённого KDE-моста.
    pub phone: crate::companion::View,
}
impl Default for View {
    /// Задаёт исходное состояние нового экземпляра.
    fn default() -> Self {
        Self {
            id: NONE,
            gallery: gallery::View::default(),
            keyboard_app: keyboard_app::View::default(),
            remote: remote::View::default(),
            cursor: 0,
            text: [0; 32],
            len: 0,
            uptime: 0,
            free_kib: 0,
            internal_kib: 0,
            script_us: 0,
            cache_hits: 0,
            memory: crate::swap_config::Snapshot::default(),
            weather: crate::weather::Snapshot::default(),
            weather_editor: crate::weather::EditorView::default(),
            world_offset: 3,
            breathe_elapsed: 0,
            breathing: false,
            tools: utilities::Tools::default(),
            files: files::View::default(),
            calculator: calculator::View::default(),
            organizer: organizer::View::default(),
            focus: organizer::FocusView::default(),
            phone: crate::companion::View::default(),
        }
    }
}
/// Рабочее состояние компонента компонента нативных приложений.
pub struct State {
    /// Подготовленный снимок данных для интерфейса.
    view: View,
    /// Состояние выражения и результата калькулятора.
    calculator: calculator::Calculator,
    /// Последний проверенный погодный снимок.
    weather: crate::weather::Controller,
    /// Выбранный часовой пояс мирового времени, часы относительно UTC.
    world_offset: i8,

    /// Состояния календаря, конвертера и счётчика.
    tools: utilities::Tools,
    /// Состояние проводника и текстового редактора.
    files: files::Controller,
    /// Контроллер списка задач с отдельным файлом.
    tasks: organizer::Ledger,
    /// Выбор журнала расходов вместо списка задач.

    /// Индекс выбранной клавиши.
    focus: organizer::Focus,
    gallery: gallery::Controller,
    keyboard_app: keyboard_app::Controller,
    google: google_search::Controller,
    google_calendar: google_calendar::Controller,
    gmail: gmail::Controller,
    translator: translator::Controller,
    gallery_from_files: bool,
}
impl Default for State {
    /// Задаёт исходное состояние нового экземпляра.
    fn default() -> Self {
        Self {
            view: View::default(),
            calculator: calculator::Calculator::default(),
            weather: crate::weather::Controller::default(),
            world_offset: 3,

            tools: utilities::Tools::default(),
            files: files::Controller::default(),
            tasks: organizer::Ledger::default(),

            focus: organizer::Focus::default(),
            gallery: gallery::Controller::default(),
            keyboard_app: keyboard_app::Controller::default(),
            google: google_search::Controller::default(),
            google_calendar: google_calendar::Controller::default(),
            gmail: gmail::Controller::default(),
            translator: translator::Controller::default(),
            gallery_from_files: false,
        }
    }
}
impl State {
    /// Возвращает идентификатор активного приложения без построения снимка.
    pub fn id(&self) -> u8 {
        self.view.id
    }
    /// Определяет, требует ли текущее состояние удержать экран включённым.
    pub fn keep_awake(&self) -> bool {
        self.focus.keep_awake()
    }
    /// Сообщает, активен ли соответствующий режим.
    pub fn active(&self) -> bool {
        self.view.id != NONE
    }
    /// Закрывает активный режим и освобождает связанные временные состояния.
    pub fn close(&mut self) {
        if self.view.id == GALLERY {
            self.gallery.close();
        }
        self.view.id = NONE;
        self.weather.cancel();
    }
    /// Открывает выбранный режим и подготавливает его состояние.
    pub fn open(&mut self, id: u8) {
        if !catalog::LAUNCHER.contains(&id) && !matches!(id, DIAGNOSTICS | MEMORY | WORLD) {
            return;
        }
        self.view.id = id;
        self.gallery_from_files = false;
        match id {
            GALLERY => self.gallery.open(),
            BT_KEYBOARD => self.keyboard_app.open(),
            GOOGLE => self.google.open(),
            GOOGLE_CALENDAR => self.google_calendar.open(),
            GMAIL => self.gmail.open(),
            TRANSLATOR => self.translator.open(),
            _ => {}
        }
        if matches!(id, FILES | EDITOR) {
            self.files.open(id == EDITOR);
        }
        if id == TASKS {
            self.tasks.open();
        }

        if id == CALENDAR {
            self.tools.calendar_open();
        }

        self.view.cursor = if id == MEMORY {
            crate::swap_config::CHOICES
                .iter()
                .position(|&m| m == crate::swap_config::snapshot().selected)
                .unwrap_or(0) as u8
        } else {
            0
        };
        self.refresh_text();
    }
    /// Рисует экран cancel с актуальным состоянием и фокусом.
    pub fn cancel_editor(&mut self) -> bool {
        if self.view.id == GALLERY {
            if self.gallery_from_files {
                self.view.id = FILES;
                self.gallery_from_files = false;
                true
            } else {
                self.gallery.back()
            }
        } else if self.view.id == BT_KEYBOARD {
            self.keyboard_app.back()
        } else if self.view.id == GOOGLE {
            self.google.back()
        } else if self.view.id == GOOGLE_CALENDAR {
            self.google_calendar.back()
        } else if self.view.id == GMAIL {
            self.gmail.back()
        } else if self.view.id == TRANSLATOR {
            self.translator.back()
        } else if matches!(self.view.id, FILES | EDITOR) {
            self.files.back()
        } else if self.view.id == TASKS {
            self.tasks.back()
        } else {
            self.view.id == WEATHER && self.weather.cancel()
        }
    }
    /// Выполняет очередной неблокирующий шаг обработки и проверяет готовые результаты.
    pub fn poll(&mut self) {
        self.google.poll();
        self.google_calendar.poll();
        self.gmail.poll();
        self.translator.poll();
        self.files.poll();
        self.tasks.poll();
        self.gallery.poll();
        if let Some(path) = gallery::take_open() {
            self.view.id = GALLERY;
            self.gallery_from_files = true;
            self.gallery.open_path(path);
        }
        self.focus.poll();
    }
    /// Собирает снимок состояния для интерфейса.
    pub fn view(&self) -> View {
        let mut view = self.view;
        view.tools = self.tools;
        match view.id {
            GALLERY => view.gallery = self.gallery.view(),
            BT_KEYBOARD => view.keyboard_app = self.keyboard_app.view(),
            GOOGLE => view.remote = self.google.view(),
            GOOGLE_CALENDAR => view.remote = self.google_calendar.view(),
            GMAIL => view.remote = self.gmail.view(),
            TRANSLATOR => view.remote = self.translator.view(),
            _ => {}
        }
        if matches!(view.id, FILES | EDITOR) {
            view.files = self.files.view();
        }
        if view.id == CALCULATOR {
            view.calculator = self.calculator.view();
        }
        if view.id == TASKS {
            view.organizer = self.tasks.view();
        }

        if view.id == FOCUS {
            view.focus = self.focus.view();
        }
        if view.id == PHONE {
            view.phone = crate::companion::view();
        }
        if view.id == CALENDAR {
            view.tools.today = crate::rtc::calendar_date();
        }
        if view.id == WEATHER {
            view.weather = crate::weather::snapshot();
            view.weather_editor = self.weather.view();
        }
        view.world_offset = self.world_offset;
        view
    }
    /// Общая точка аккаунта из настроек.
    pub fn open_account(&mut self) {
        self.open(GOOGLE);
        self.google.account();
    }
    /// Системная BLE-служба забирает отчёты, приложение не владеет контроллером радио.
    pub fn bluetooth_action(&mut self) -> Option<crate::runtime::BluetoothUiAction> {
        if self.view.id == BT_KEYBOARD {
            self.keyboard_app.take()
        } else {
            None
        }
    }
    /// Обновляет ограниченный буфер результата калькулятора для интерфейса.
    fn refresh_text(&mut self) {
        let text = self.calculator.text();
        self.view.text.fill(0);
        self.view.len = text.len().min(32) as u8;
        self.view.text[..usize::from(self.view.len)]
            .copy_from_slice(&text.as_bytes()[..usize::from(self.view.len)]);
    }
    /// Work is performed only on input; utility pages have no polling tasks.
    pub fn event(&mut self, event: RuntimeEvent) -> Option<u8> {
        match self.view.id {
            GALLERY => {
                self.gallery.event(event);
                return None;
            }
            BT_KEYBOARD => {
                self.keyboard_app.event(event);
                return None;
            }
            GOOGLE => {
                self.google.event(event);
                return None;
            }
            GOOGLE_CALENDAR => {
                self.google_calendar.event(event);
                return None;
            }
            GMAIL => {
                self.gmail.event(event);
                return None;
            }
            TRANSLATOR => {
                self.translator.event(event);
                return None;
            }
            TASKS => {
                self.tasks.event(event);
                return None;
            }

            FOCUS => {
                self.focus.event(event);
                return None;
            }
            PHONE => {
                crate::companion::event(event, self.files.saved_text());
                return None;
            }
            FILES | EDITOR => {
                self.files.event(event);
                return None;
            }
            CALENDAR => {
                self.tools.calendar_event(event);
                return None;
            }

            _ => {}
        }
        if self.view.id == WEATHER {
            self.weather.event(event);
            return None;
        }
        if self.view.id == WORLD {
            match event {
                RuntimeEvent::Rotate(d) => {
                    self.world_offset = ((i32::from(self.world_offset) + 12 + d.rem_euclid(27))
                        .rem_euclid(27)
                        - 12) as i8
                }
                RuntimeEvent::Press => self.world_offset = crate::rtc::timezone_hours(),
                _ => {}
            }
            return None;
        }
        let count = match self.view.id {
            CALCULATOR => calculator::KEYS.len() as i32,
            FILES => i32::from(crate::wallpaper::builtin_count()),
            SHARE => 3,
            MEMORY => 5,
            _ => 5,
        };
        match event {
            RuntimeEvent::Rotate(delta) => {
                self.view.cursor =
                    (i32::from(self.view.cursor) + delta.rem_euclid(count)).rem_euclid(count) as u8
            }
            RuntimeEvent::Swipe(delta) => {
                self.view.cursor =
                    (i32::from(self.view.cursor) + delta.signum()).rem_euclid(count) as u8
            }
            RuntimeEvent::Press if self.view.id == CALCULATOR => {
                self.calculator.press(self.view.cursor);
                self.refresh_text();
            }
            RuntimeEvent::Press if self.view.id == FILES => return Some(self.view.cursor + 1),
            RuntimeEvent::Press if self.view.id == MEMORY => crate::swap_config::request(
                crate::swap_config::CHOICES[usize::from(self.view.cursor)],
            ),
            _ => {}
        }
        None
    }
}
