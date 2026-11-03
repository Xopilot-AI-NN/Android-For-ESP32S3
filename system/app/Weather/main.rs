// Модуль погоды и поиска города.
//! Weather state and the shared on-watch city keyboard. Networking lives in net/weather_client.
use crate::{
    runtime::RuntimeEvent,
    text_input::{Action, Keyboard, KeyboardView},
};
use core::{
    cell::RefCell,
    sync::atomic::{AtomicBool, Ordering},
};
use critical_section::Mutex;
use heapless::String;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Согласованный снимок состояния компонента погоды и поиска города.
pub struct Snapshot {
    /// UTF-8-имя города в ограниченном буфере.
    pub city: [u8; 48],
    /// Число значащих байтов буфера city.
    pub city_len: u8,
    /// Температура в десятых долях градуса Цельсия.
    pub temp: i16,
    /// Ощущаемая температура в десятых долях градуса Цельсия.
    pub feels: i16,
    /// Относительная влажность в процентах.
    pub humidity: u8,
    /// Скорость ветра в десятых долях м/с.
    pub wind: u16,
    /// Код погоды или подтверждения текущего протокола.
    pub code: u8,
    /// Широта выбранного места в формате погодного клиента.
    pub lat: i32,
    /// Долгота выбранного места в формате погодного клиента.
    pub lon: i32,
    /// Признак того, что данные получены и прошли необходимые проверки.
    pub valid: bool,
    /// Признак приблизительного местоположения по внешнему IP.
    pub approximate: bool,
    /// Код загрузки/ошибки погоды для интерфейса; 2 означает успешный ответ.
    pub status: u8,
    /// Время последнего успешного обновления данных.
    pub updated: u64,
}
impl Snapshot {
    /// Создаёт пустую запись без пользовательских данных.
    const fn empty() -> Self {
        Self {
            city: [0; 48],
            city_len: 0,
            temp: 0,
            feels: 0,
            humidity: 0,
            wind: 0,
            code: 0,
            lat: 0,
            lon: 0,
            valid: false,
            approximate: false,
            status: 0,
            updated: 0,
        }
    }
}
impl Default for Snapshot {
    /// Задаёт исходное состояние нового экземпляра.
    fn default() -> Self {
        Self::empty()
    }
}
#[derive(Clone)]
/// Отложенная пользовательская операция сервиса компонента погоды и поиска города.
pub enum Request {
    Location,
    Search(String<64>),
    Refresh,
}
/// Состояние, совместно используемое сервисом и интерфейсом компонента погоды и поиска города.
struct Shared {
    /// Содержимое текущего пакета, страницы или модели.
    data: Snapshot,
    /// Необработанный запрос или ожидающий результат.
    pending: Option<Request>,
    /// Монотонное время/интервал last_request в миллисекундах.
    last_request_ms: u64,
}
/// Общее состояние сервиса, доступ к которому защищён critical_section.
static STATE: Mutex<RefCell<Shared>> = Mutex::new(RefCell::new(Shared {
    data: Snapshot::empty(),
    pending: None,
    last_request_ms: 0,
}));
/// Атомарный признак готовности сетевого соединения.
static ONLINE: AtomicBool = AtomicBool::new(false);
/// Обновляет признак доступности сети для погодных запросов.
pub fn online(value: bool) {
    ONLINE.store(value, Ordering::Relaxed);
}
/// Возвращает согласованный снимок текущего состояния.
pub fn snapshot() -> Snapshot {
    critical_section::with(|cs| STATE.borrow_ref(cs).data)
}
/// Ставит запрос в очередь для обработки владельцем ресурса.
pub fn request(request: Request) {
    critical_section::with(|cs| {
        let mut s = STATE.borrow_ref_mut(cs);
        if !ONLINE.load(Ordering::Relaxed) {
            s.data.status = 3;
            return;
        }
        let now = esp_hal::time::Instant::now()
            .duration_since_epoch()
            .as_millis() as u64;
        if s.last_request_ms != 0 && now.saturating_sub(s.last_request_ms) < 3000 {
            return;
        }
        s.last_request_ms = now;
        s.pending = Some(request);
        s.data.status = 1;
    });
}
/// Забирает следующий отложенный запрос, удаляя его из очереди.
pub fn take_request() -> Option<Request> {
    critical_section::with(|cs| STATE.borrow_ref_mut(cs).pending.take())
}
/// Сохраняет код ошибки текущей операции.
pub fn fail(status: u8) {
    critical_section::with(|cs| STATE.borrow_ref_mut(cs).data.status = status);
}
/// Публикует проверенный прогноз и отмечает смену местоположения для сохранения.
pub fn publish(mut data: Snapshot) {
    data.valid = true;
    data.status = 2;
    data.updated = esp_hal::time::Instant::now()
        .duration_since_epoch()
        .as_secs();
    let changed = critical_section::with(|cs| {
        let mut state = STATE.borrow_ref_mut(cs);
        let changed = location_bytes(state.data) != location_bytes(data);
        state.data = data;
        changed
    });
    if changed {
        critical_section::with(|cs| {
            *DIRTY_LOCATION.borrow_ref_mut(cs) = Some(location_bytes(data))
        });
    }
}
/// Копирует безопасное имя города в ограниченный UTF-8-буфер.
pub fn city(data: &mut Snapshot, name: &str) {
    data.city.fill(0);
    let mut n = 0;
    for ch in name.chars().filter(|c| !c.is_control()) {
        let size = ch.len_utf8();
        if n + size > 48 {
            break;
        }
        ch.encode_utf8(&mut data.city[n..n + size]);
        n += size;
    }
    data.city_len = n as u8;
}
/// Переводит погодный код в понятное описание.
pub fn description(code: u8) -> &'static str {
    match code {
        0 => "Ясно",
        1 | 2 => "Переменная облачность",
        3 => "Облачно",
        45 | 48 => "Туман",
        51..=57 => "Морось",
        61..=67 => "Дождь",
        71..=77 => "Снег",
        80..=82 => "Ливень",
        85 | 86 => "Снегопад",
        95..=99 => "Гроза",
        _ => "Погодные условия",
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Структура данных компонента погоды и поиска города.
pub struct EditorView {
    /// Признак активности режима.
    pub active: bool,
    /// Текстовое содержимое в ограниченном буфере.
    pub text: [u8; 64],
    /// Число значащих байтов в соответствующем текстовом буфере.
    pub len: u8,
    /// Состояние общей RU/EN-клавиатуры.
    pub keyboard: KeyboardView,
    /// Индекс текущей цели навигации.
    pub cursor: u8,
}
impl Default for EditorView {
    /// Задаёт исходное состояние нового экземпляра.
    fn default() -> Self {
        Self {
            active: false,
            text: [0; 64],
            len: 0,
            keyboard: KeyboardView::default(),
            cursor: 0,
        }
    }
}
#[derive(Default)]
/// Контроллер пользовательских событий и переходов компонента погоды и поиска города.
pub struct Controller {
    /// Признак открытой клавиатуры редактора.
    pub editing: bool,
    /// Черновик ввода; его изменение ещё не означает запись на носитель.
    draft: String<64>,
    /// Состояние общей RU/EN-клавиатуры.
    keyboard: Keyboard,
    /// Индекс текущей цели навигации.
    cursor: u8,
}
impl Controller {
    /// Собирает снимок состояния для интерфейса.
    pub fn view(&self) -> EditorView {
        let mut v = EditorView {
            active: self.editing,
            keyboard: self.keyboard.view,
            cursor: self.cursor,
            ..EditorView::default()
        };
        v.len = self.draft.len() as u8;
        v.text[..self.draft.len()].copy_from_slice(self.draft.as_bytes());
        v
    }
    /// Отменяет ещё ожидающую операцию или незабранный результат.
    pub fn cancel(&mut self) -> bool {
        let active = self.editing;
        self.editing = false;
        active
    }
    /// Обрабатывает событие интерфейса и выполняет соответствующий переход состояния.
    pub fn event(&mut self, event: RuntimeEvent) {
        if self.editing {
            match event {
                RuntimeEvent::Rotate(d) => self.keyboard.rotate(d),
                RuntimeEvent::Back | RuntimeEvent::Home | RuntimeEvent::Swipe(2) => {
                    self.editing = false
                }
                RuntimeEvent::Press => match self.keyboard.press() {
                    Action::Character(c) => {
                        let _ = self.draft.push(c);
                    }
                    Action::Suggestion(word) => {
                        crate::text_input::complete(&mut self.draft, word);
                    }
                    Action::Backspace => {
                        self.draft.pop();
                    }
                    Action::Cancel => self.editing = false,
                    Action::Submit => {
                        if !self.draft.trim().is_empty() {
                            request(Request::Search(self.draft.clone()));
                            self.editing = false;
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
            self.keyboard.suggest(&self.draft);
            return;
        }
        match event {
            RuntimeEvent::Rotate(d) => {
                self.cursor = (i32::from(self.cursor) + d.rem_euclid(3)).rem_euclid(3) as u8
            }
            RuntimeEvent::Press => match self.cursor {
                0 => {
                    self.editing = true;
                    self.keyboard.suggest(&self.draft);
                }
                1 => request(Request::Location),
                _ => request(Request::Refresh),
            },
            _ => {}
        }
    }
}

// Only location preferences are persistent; forecasts stay in RAM and show their age.
const LOCATION_SLOT: u32 = 74;
/// Тег формата записи LOCATION для проверки типа данных.
const LOCATION_TAG: u32 = 0x574c_4f43;
/// Параметр DIRTY_LOCATION компонента погоды и поиска города.
static DIRTY_LOCATION: Mutex<RefCell<Option<[u8; 64]>>> = Mutex::new(RefCell::new(None));
/// Кодирует выбранное местоположение для постоянного хранения.
fn location_bytes(data: Snapshot) -> [u8; 64] {
    let mut raw = [0; 64];
    raw[0] = 1;
    raw[1] = u8::from(data.approximate);
    raw[2] = data.city_len;
    raw[4..8].copy_from_slice(&data.lat.to_le_bytes());
    raw[8..12].copy_from_slice(&data.lon.to_le_bytes());
    raw[12..60].copy_from_slice(&data.city);
    raw
}
/// Восстанавливает последнее успешно сохранённое местоположение.
pub fn restore_location<D: embedded_sdmmc::BlockDevice>(
    dev: &D,
    pager: Option<&crate::page_store::PageStore>,
) {
    let Some(pager) = pager else {
        return;
    };
    let mut raw = [0; 64];
    if pager.read(dev, LOCATION_SLOT, LOCATION_TAG, &mut raw) != Ok(64)
        || raw[0] != 1
        || raw[1] > 1
        || raw[2] > 48
    {
        return;
    }
    let lat = i32::from_le_bytes(raw[4..8].try_into().unwrap());
    let lon = i32::from_le_bytes(raw[8..12].try_into().unwrap());
    if !(-90000000..=90000000).contains(&lat)
        || !(-180000000..=180000000).contains(&lon)
        || core::str::from_utf8(&raw[12..12 + usize::from(raw[2])]).is_err()
    {
        return;
    }
    critical_section::with(|cs| {
        let mut s = STATE.borrow_ref_mut(cs);
        s.data.lat = lat;
        s.data.lon = lon;
        s.data.approximate = raw[1] != 0;
        s.data.city_len = raw[2];
        s.data.city.copy_from_slice(&raw[12..60]);
    });
}
/// Сохраняет location в выделенную запись постоянного хранилища.
pub fn save_location<D: embedded_sdmmc::BlockDevice>(dev: &D) {
    let Some(raw) = critical_section::with(|cs| DIRTY_LOCATION.borrow_ref_mut(cs).take()) else {
        return;
    };
    let ok = crate::page_store::PageStore::open_base(dev)
        .ok()
        .is_some_and(|p| p.write(dev, LOCATION_SLOT, LOCATION_TAG, &raw).is_ok());
    if !ok {
        crate::watch_println!("Weather: location preference was not saved");
    }
}
