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
pub struct Snapshot {
    pub city: [u8; 48],
    pub city_len: u8,
    pub temp: i16,
    pub feels: i16,
    pub humidity: u8,
    pub wind: u16,
    pub code: u8,
    pub lat: i32,
    pub lon: i32,
    pub valid: bool,
    pub approximate: bool,
    pub status: u8,
    pub updated: u64,
}
impl Snapshot {
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
    fn default() -> Self {
        Self::empty()
    }
}
#[derive(Clone)]
pub enum Request {
    Location,
    Search(String<64>),
    Refresh,
}
struct Shared {
    data: Snapshot,
    pending: Option<Request>,
    last_request_ms: u64,
}
static STATE: Mutex<RefCell<Shared>> = Mutex::new(RefCell::new(Shared {
    data: Snapshot::empty(),
    pending: None,
    last_request_ms: 0,
}));
static ONLINE: AtomicBool = AtomicBool::new(false);
pub fn online(value: bool) {
    ONLINE.store(value, Ordering::Relaxed);
}
pub fn snapshot() -> Snapshot {
    critical_section::with(|cs| STATE.borrow_ref(cs).data)
}
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
pub fn take_request() -> Option<Request> {
    critical_section::with(|cs| STATE.borrow_ref_mut(cs).pending.take())
}
pub fn fail(status: u8) {
    critical_section::with(|cs| STATE.borrow_ref_mut(cs).data.status = status);
}
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
pub struct EditorView {
    pub active: bool,
    pub text: [u8; 64],
    pub len: u8,
    pub keyboard: KeyboardView,
    pub cursor: u8,
}
impl Default for EditorView {
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
pub struct Controller {
    pub editing: bool,
    draft: String<64>,
    keyboard: Keyboard,
    cursor: u8,
}
impl Controller {
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
    pub fn cancel(&mut self) -> bool {
        let active = self.editing;
        self.editing = false;
        active
    }
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
            return;
        }
        match event {
            RuntimeEvent::Rotate(d) => {
                self.cursor = (i32::from(self.cursor) + d.rem_euclid(3)).rem_euclid(3) as u8
            }
            RuntimeEvent::Press => match self.cursor {
                0 => self.editing = true,
                1 => request(Request::Location),
                _ => request(Request::Refresh),
            },
            _ => {}
        }
    }
}

// Only location preferences are persistent; forecasts stay in RAM and show their age.
const LOCATION_SLOT: u32 = 74;
const LOCATION_TAG: u32 = 0x574c_4f43;
static DIRTY_LOCATION: Mutex<RefCell<Option<[u8; 64]>>> = Mutex::new(RefCell::new(None));
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
