//! Локальная галерея читает JPEG/PNG через файловую службу, без Google и сети.
use crate::{
    file_store::{self as fs, Op},
    runtime::RuntimeEvent,
};
use alloc::{
    string::{String, ToString},
    sync::Arc,
    vec::Vec,
};
use core::cell::RefCell;
use critical_section::Mutex;
static PIXELS: Mutex<RefCell<Option<Arc<Vec<u16>>>>> = Mutex::new(RefCell::new(None));
// Intent holds only a path; a multi-megabyte photo never enters this mailbox.
static OPEN: Mutex<RefCell<Option<String>>> = Mutex::new(RefCell::new(None));
/// Открытие изображения из проводника передаёт путь, а не буфер всего файла.
pub fn request_open(path: &str) {
    critical_section::with(|cs| *OPEN.borrow_ref_mut(cs) = Some(path.into()));
}
/// Забирает intent ровно один раз при переключении приложения из Files.
pub fn take_open() -> Option<String> {
    critical_section::with(|cs| OPEN.borrow_ref_mut(cs).take())
}
/// Даёт отрисовщику разделяемый preview без копирования 96 КиБ на каждый кадр.
pub fn pixels() -> Option<Arc<Vec<u16>>> {
    critical_section::with(|cs| PIXELS.borrow_ref(cs).clone())
}
/// Отбирает кандидатов по расширению; содержимое затем проверяет декодер.
pub fn is_image(path: &str) -> bool {
    let p = path.to_ascii_lowercase();
    p.ends_with(".jpg") || p.ends_with(".jpeg") || p.ends_with(".png")
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Снимок UI: три имени в окне списка, состояние I/O и поколение картинки.
pub struct View {
    pub names: [[u8; 64]; 3],
    pub lengths: [u8; 3],
    pub cursor: u8,
    pub total: u8,
    pub image: bool,
    pub busy: bool,
    pub revision: u32,
    pub message: &'static str,
}
impl Default for View {
    fn default() -> Self {
        Self {
            names: [[0; 64]; 3],
            lengths: [0; 3],
            cursor: 0,
            total: 0,
            image: false,
            busy: false,
            revision: 0,
            message: "",
        }
    }
}
#[derive(Default)]
/// Владеет навигацией и токеном запроса; диск обслуживает отдельная служба.
pub struct Controller {
    list: Vec<String>,
    cursor: usize,
    image: bool,
    pending: Option<(u32, bool)>,
    requested: Option<String>,
    revision: u32,
    message: &'static str,
}
impl Controller {
    /// Отменяет отложенный read и освобождает preview при уходе из приложения.
    pub fn close(&mut self) {
        if let Some((id, _)) = self.pending.take() {
            fs::cancel(id);
        }
        self.requested = None;
        critical_section::with(|cs| *PIXELS.borrow_ref_mut(cs) = None);
    }
    /// Начинает просмотр каталога без автоматического открытия первого изображения.
    pub fn open(&mut self) {
        self.image = false;
        self.cursor = 0;
        self.refresh();
    }
    /// Открывает конкретный файл после готовности каталога хранилища.
    pub fn open_path(&mut self, path: String) {
        self.open();
        self.requested = Some(path);
        self.poll();
    }
    /// Читает опубликованный снимок метаданных; дисковый I/O здесь отсутствует.
    fn refresh(&mut self) {
        if let Some(catalog) = fs::catalog() {
            self.list = catalog
                .iter()
                .filter(|e| e.kind == 1 && is_image(&e.path))
                .map(|e| e.path.to_string())
                .collect();
            self.cursor = self.cursor.min(self.list.len().saturating_sub(1));
        }
    }
    /// Заказывает ограниченный RGB565-preview вместо загрузки исходного фото в RAM.
    fn load(&mut self) {
        if self.pending.is_some() {
            return;
        }
        let Some(path) = self.list.get(self.cursor) else {
            return;
        };
        match fs::request(Op::Thumbnail(path.clone())) {
            Ok(id) => {
                self.pending = Some((id, true));
                self.image = true;
                self.message = "Открываю изображение…";
            }
            Err(e) => self.message = e.label(),
        }
    }
    /// Забирает ответ по своему токену и публикует только полностью готовый preview.
    pub fn poll(&mut self) {
        if let Some((id, image)) = self.pending {
            if let Some(reply) = fs::take_reply(id) {
                self.pending = None;
                match reply.result {
                    Ok(Some(data)) if image && data.len() == 240 * 200 * 2 => {
                        let data = Arc::new(
                            data.chunks_exact(2)
                                .map(|b| u16::from_le_bytes([b[0], b[1]]))
                                .collect(),
                        );
                        critical_section::with(|cs| *PIXELS.borrow_ref_mut(cs) = Some(data));
                        self.revision = self.revision.wrapping_add(1);
                        self.message = "";
                    }
                    Ok(Some(_)) if image => {
                        self.image = false;
                        self.message = "Некорректный размер preview";
                    }
                    Ok(_) => {
                        self.refresh();
                        self.message = "";
                    }
                    Err(_) => {
                        self.message = "Не удалось прочитать изображение";
                        self.image = false;
                    }
                }
            }
        }
        if fs::catalog().is_none() && self.pending.is_none() {
            if let Ok(id) = fs::request(Op::Mount) {
                self.pending = Some((id, false));
            }
        }
        if let Some(path) = self.requested.take() {
            self.refresh();
            if let Some(index) = self.list.iter().position(|p| *p == path) {
                self.cursor = index;
                self.load();
            } else if fs::catalog().is_none() {
                self.requested = Some(path);
            }
        }
    }
    /// Колесо листает список или фотографии; во время чтения запрос не дублируется.
    pub fn event(&mut self, event: RuntimeEvent) {
        self.refresh();
        if self.pending.is_some() {
            return;
        }
        match event {
            RuntimeEvent::Rotate(d) if !self.list.is_empty() => {
                self.cursor = (self.cursor as i32 + d).rem_euclid(self.list.len() as i32) as usize;
                if self.image {
                    self.load();
                }
            }
            RuntimeEvent::Press => self.load(),
            _ => {}
        }
    }
    /// Сначала возвращает к списку; false разрешает router выйти из приложения.
    pub fn back(&mut self) -> bool {
        if self.image {
            self.image = false;
            true
        } else {
            false
        }
    }
    /// Собирает ограниченные UTF-8-подписи без владения изображением в UI-снимке.
    pub fn view(&self) -> View {
        let mut v = View {
            cursor: self.cursor as u8,
            total: self.list.len() as u8,
            image: self.image,
            busy: self.pending.is_some(),
            revision: self.revision,
            message: self.message,
            ..View::default()
        };
        let start = if self.image {
            self.cursor
        } else {
            self.cursor
                .saturating_sub(1)
                .min(self.list.len().saturating_sub(3))
        };
        for i in 0..3 {
            if let Some(path) = self.list.get(start + i) {
                v.lengths[i] = crate::apps::files::copy_text(&mut v.names[i], fs::basename(path));
            }
        }
        v
    }
}
impl Drop for Controller {
    fn drop(&mut self) {
        if let Some((id, _)) = self.pending {
            fs::cancel(id);
        }
    }
}
