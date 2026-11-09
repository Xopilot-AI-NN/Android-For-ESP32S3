// Модуль проводника и построчного редактора.
//! Explorer and line-oriented UTF-8 editor. No disk operations run in paint/input.
use crate::{
    file_store::{self as fs, Entry, Op},
    runtime::RuntimeEvent,
    text_input::{Action, Keyboard, KeyboardView},
};
use alloc::{
    string::{String, ToString},
    vec::Vec,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Одна видимая строка списка компонента проводника и построчного редактора.
pub struct Row {
    /// Текстовое содержимое в ограниченном буфере.
    pub text: [u8; 64],
    /// Число значащих байтов в соответствующем текстовом буфере.
    pub len: u8,
    /// Вид записи или пакета; допустимые значения определяет формат компонента.
    pub kind: u8,
    /// Размер содержимого в байтах.
    pub size: u32,
    /// Выбранный элемент либо признак его выделения.
    pub selected: bool,
}
impl Default for Row {
    /// Задаёт исходное состояние нового экземпляра.
    fn default() -> Self {
        Self {
            text: [0; 64],
            len: 0,
            kind: 0,
            size: 0,
            selected: false,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Снимок данных для отображения компонента проводника и построчного редактора.
pub struct View {
    /// Экран: 0 каталог, 1 действия, 2 имя, 3 документ, 4 строка, 5 несохранённое, 6 удаление, 8 создание.
    pub mode: u8,
    /// Заголовок текущего экрана или документа.
    pub title: [u8; 80],
    /// Число значащих байтов буфера title.
    pub title_len: u8,
    /// Подготовленные видимые строки списка.
    pub rows: [Row; 6],
    /// Количество элементов или выполненных действий в текущем контексте.
    pub count: u16,
    /// Индекс текущей цели навигации.
    pub cursor: u16,
    /// Входные данные или временно введённые символы.
    pub input: [u8; 96],
    /// Число значащих байтов буфера input.
    pub input_len: u8,
    /// Текущее количество байтов пользовательского ввода.
    pub bytes: u32,
    /// Действующее ограничение размера ввода.
    pub limit: u32,
    /// Состояние общей RU/EN-клавиатуры.
    pub keyboard: KeyboardView,
    /// Ошибка последней операции или признак ошибки.
    pub error: u8,
    /// Сообщение или его код для вывода пользователю.
    pub message: &'static str,
    /// Признак ещё выполняющейся операции.
    pub busy: bool,
    /// Признак изменений, которые ещё не подтверждены записью.
    pub dirty: bool,
    /// Число файлов/папок, без кнопок нижней панели.
    pub items: u8,
    /// 0: создать, 1: действия, 2: вставить, 3: вверх; 255: фокус в списке.
    pub toolbar: u8,
    /// Имя элемента, к которому относятся кнопки панели действий.
    pub selected_name: [u8; 64],
    /// Число значащих байтов буфера selected.
    pub selected_len: u8,
    pub selected_size: u32,
    pub first_row: u16,
    /// Содержимое или признак доступности буфера копирования/перемещения.
    pub clipboard: bool,
    /// Признак наличия родительской папки.
    pub can_up: bool,
}
impl Default for View {
    /// Задаёт исходное состояние нового экземпляра.
    fn default() -> Self {
        Self {
            mode: 0,
            title: [0; 80],
            title_len: 0,
            rows: [Row::default(); 6],
            count: 0,
            cursor: 0,
            input: [0; 96],
            input_len: 0,
            bytes: 0,
            limit: 0,
            keyboard: KeyboardView::default(),
            error: 0,
            message: "",
            busy: false,
            dirty: false,
            items: 0,
            toolbar: 255,
            selected_size: 0,
            first_row: 0,
            selected_name: [0; 64],
            selected_len: 0,
            clipboard: false,
            can_up: false,
        }
    }
}
/// Копирует подпись в буфер, не разрывая символ UTF-8.
pub fn copy_text<const N: usize>(dst: &mut [u8; N], value: &str) -> u8 {
    let mut end = value.len().min(N);
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    dst[..end].copy_from_slice(&value.as_bytes()[..end]);
    end as u8
}
#[derive(Clone, Copy)]
/// Назначение ожидающего ответа для перехода интерфейса компонента проводника и построчного редактора.
enum Purpose {
    Mount,
    Read,
    Save,
    New,
    Mutation,
    Paste,
}
/// Контроллер пользовательских событий и переходов компонента проводника и построчного редактора.
pub struct Controller {
    /// Экран: 0 каталог, 1 действия, 2 имя, 3 документ, 4 строка, 5 несохранённое, 6 удаление, 8 создание.
    mode: u8,
    /// Индекс текущей цели навигации.
    cursor: usize,
    /// Текущая папка проводника.
    dir: String,
    /// Кэш элементов открытого каталога.
    listing: Vec<Entry>,
    /// Выбранный элемент либо признак его выделения.
    selected: String,
    /// Содержимое или признак доступности буфера копирования/перемещения.
    clipboard: Option<(String, bool)>,
    /// Полный открытый UTF-8-документ в пределах лимита.
    document: String,
    /// Черновик ввода; его изменение ещё не означает запись на носитель.
    draft: String,
    /// Путь открытого документа.
    file: String,
    /// Контрольная сумма содержимого для обнаружения повреждений.
    crc: Option<u32>,
    /// Признак изменений, которые ещё не подтверждены записью.
    dirty: bool,
    /// Выбранная строка документа.
    line: usize,
    /// Состояние общей RU/EN-клавиатуры.
    keyboard: Keyboard,
    /// Операция ввода имени: создание файла, папки либо переименование.
    name_action: u8,
    /// Необработанный запрос или ожидающий результат.
    pending: Option<(u32, Purpose)>,
    /// Сообщение или его код для вывода пользователю.
    message: &'static str,
    /// Ошибка последней операции или признак ошибки.
    error: u8,
    /// Признак открытия нового документа через отдельный редактор.
    editor_launch: bool,
    menu_line: usize,
    /// Последняя учтённая ревизия данных для обновления кэша.
    revision: u32,
}
impl Default for Controller {
    /// Задаёт исходное состояние нового экземпляра.
    fn default() -> Self {
        Self {
            mode: 0,
            cursor: 0,
            dir: "/".to_string(),
            listing: Vec::new(),
            selected: String::new(),
            clipboard: None,
            document: String::new(),
            draft: String::new(),
            file: String::new(),
            crc: None,
            dirty: false,
            line: 0,
            keyboard: Keyboard::default(),
            name_action: 0,
            pending: None,
            message: "",
            error: 0,
            editor_launch: false,
            menu_line: 0,
            revision: 0,
        }
    }
}
impl Controller {
    /// Возвращает только сохранённый текст, пригодный для явной отправки.
    pub fn saved_text(&self) -> Option<&str> {
        (!self.dirty && self.mode != 4 && self.crc.is_some()).then_some(self.document.as_str())
    }
    /// Открывает проводник/редактор, сохраняя незавершённый пользовательский ввод.
    pub fn open(&mut self, editor: bool) {
        if self.dirty
            || self.pending.is_some()
            || (matches!(self.mode, 2 | 4) && !self.draft.is_empty())
        {
            return;
        }
        self.editor_launch = editor;
        self.mode = 0;
        self.cursor = 0;
        self.message = "";
        self.refresh();
        if fs::catalog().is_none() {
            self.queue(Op::Mount, Purpose::Mount);
        } else if editor {
            self.name(0);
        }
    }
    /// Ставит операцию в очередь и запоминает её идентификатор.
    fn queue(&mut self, op: Op, purpose: Purpose) {
        match fs::request(op) {
            Ok(id) => {
                self.pending = Some((id, purpose));
                self.message = "Чтение/сохранение…";
                self.error = 0;
            }
            Err(e) => self.message = e.label(),
        }
    }
    /// Обновляет кэш списка по последнему каталогу.
    fn refresh(&mut self) {
        self.revision = fs::revision();
        self.listing.clear();
        if let Some(entries) = fs::catalog() {
            self.listing.extend(
                entries
                    .iter()
                    .filter(|e| e.kind != 0 && fs::parent(&e.path) == self.dir)
                    .cloned(),
            );
            self.listing
                .sort_by(|a, b| b.kind.cmp(&a.kind).then_with(|| a.path.cmp(&b.path)));
        }
        if !self
            .listing
            .iter()
            .any(|e| e.path.as_str() == self.selected)
        {
            self.selected = self
                .listing
                .first()
                .map(|e| e.path.to_string())
                .unwrap_or_default();
        }
    }
    /// Выполняет очередной неблокирующий шаг обработки и проверяет готовые результаты.
    pub fn poll(&mut self) {
        if self.pending.is_none() && self.mode == 0 && self.revision != fs::revision() {
            self.refresh();
            self.cursor = self.cursor.min(self.count().saturating_sub(1));
        }
        let Some((token, purpose)) = self.pending else {
            return;
        };
        let Some(reply) = fs::take_reply(token) else {
            return;
        };
        self.pending = None;
        self.refresh();
        match reply.result {
            Err(e) => {
                self.message = e.label();
                self.error = 3;
            }
            Ok(data) => {
                self.message = "";
                match purpose {
                    Purpose::Mount if self.editor_launch => {
                        self.editor_launch = false;
                        self.name(0);
                    }
                    Purpose::Read => match String::from_utf8(data.unwrap_or_default()) {
                        Ok(text)
                            if !text
                                .chars()
                                .any(|c| c.is_control() && c != '\n' && c != '\r' && c != '\t') =>
                        {
                            self.document = text;
                            self.crc =
                                Some(crate::file_store::content_crc(self.document.as_bytes()));
                            self.dirty = false;
                            self.mode = 3;
                            self.cursor = 0;
                        }
                        _ => {
                            self.mode = 0;
                            self.message = "Файл не является текстом UTF-8";
                        }
                    },
                    Purpose::New => {
                        self.document.clear();
                        self.crc = Some(0);
                        self.dirty = false;
                        self.mode = 3;
                        self.cursor = 0;
                    }
                    Purpose::Save => {
                        self.crc = Some(crate::file_store::content_crc(self.document.as_bytes()));
                        self.dirty = false;
                        self.mode = 3;
                        self.message = "Сохранено";
                    }
                    Purpose::Mutation | Purpose::Paste => {
                        if matches!(purpose, Purpose::Paste)
                            && self.clipboard.as_ref().is_some_and(|(_, cut)| *cut)
                        {
                            self.clipboard = None;
                        }
                        self.browse_again();
                    }
                    _ => {}
                }
            }
        }
    }
    /// Подготавливает ввод имени файла, папки или переименования.
    fn name(&mut self, action: u8) {
        self.name_action = action;
        self.draft = if action == 2 {
            fs::basename(&self.selected).to_string()
        } else {
            String::new()
        };
        self.keyboard = Keyboard::default();
        self.keyboard.suggest(&self.draft);
        self.mode = 2;
        self.error = 0;
    }

    /// Возвращается на предыдущий экран; сообщает, был ли переход обработан.
    pub fn back(&mut self) -> bool {
        if self.mode == 9 {
            self.mode = 3;
            self.cursor = 0;
            return true;
        }
        if self.mode == 10 {
            self.browse_again();
            return true;
        }
        if self.pending.is_some() {
            return true;
        }
        match self.mode {
            2 | 1 | 6 | 8 => {
                self.browse_again();
            }
            4 => {
                self.mode = 3;
                self.cursor = self.line;
            }
            3 if self.dirty => {
                self.mode = 5;
                self.cursor = 2;
            }
            3 => {
                self.browse_again();
            }
            5 => {
                self.mode = 3;
                self.cursor = 0;
            }
            _ if self.dir != "/" => {
                self.dir = fs::parent(&self.dir).to_string();
                self.cursor = 0;
                self.refresh();
            }
            _ => return false,
        }
        true
    }
    /// Обрабатывает событие интерфейса и выполняет соответствующий переход состояния.
    pub fn event(&mut self, event: RuntimeEvent) {
        self.poll();
        // A physical multi-click must obey the same I/O gate as ordinary input.
        if self.pending.is_some() {
            return;
        }
        let event = match event {
            RuntimeEvent::Click(n) if self.mode == 0 => {
                if self.cursor >= self.listing.len() {
                    self.browse_press();
                } else {
                    self.selected = self.listing[self.cursor].path.to_string();
                    match n {
                        2 => self.browse_press(),
                        3 => {
                            self.mode = 1;
                            self.cursor = 0;
                        }
                        _ => {}
                    }
                }
                return;
            }
            RuntimeEvent::Click(_) if self.mode == 3 && self.cursor >= self.lines() => {
                RuntimeEvent::Press
            }
            RuntimeEvent::Click(n) if self.mode == 3 => {
                if n == 2 {
                    self.edit_line(self.cursor.min(self.lines()));
                } else if n == 3 {
                    self.menu_line = self.cursor.min(self.lines());
                    self.mode = 9;
                    self.cursor = 0;
                }
                return;
            }
            RuntimeEvent::Click(_) => RuntimeEvent::Press,
            other => other,
        };
        if self.mode == 2 || self.mode == 4 {
            match event {
                RuntimeEvent::Rotate(d) => self.keyboard.rotate(d),
                RuntimeEvent::Press => {
                    self.error = 0;
                    match self.keyboard.press() {
                        Action::Character(c) => {
                            let limit = if self.mode == 2 {
                                fs::MAX_PATH
                            } else {
                                fs::MAX_FILE
                            };
                            if self.draft.len() + c.len_utf8() <= limit
                                && (self.mode == 4 || !c.is_control())
                            {
                                self.draft.push(c);
                            } else {
                                self.error = 1;
                            }
                        }
                        Action::Backspace => {
                            self.draft.pop();
                        }
                        Action::Suggestion(word) => {
                            let start = self
                                .draft
                                .char_indices()
                                .rev()
                                .find(|(_, c)| !c.is_alphabetic() && *c != '-')
                                .map_or(0, |(i, c)| i + c.len_utf8());
                            let limit = if self.mode == 2 {
                                fs::MAX_PATH
                            } else {
                                fs::MAX_FILE
                            };
                            if start + word.len() + 1 <= limit {
                                self.draft.truncate(start);
                                self.draft.push_str(word);
                                self.draft.push(' ');
                            } else {
                                self.error = 1;
                            }
                        }
                        Action::Cancel => {
                            self.back();
                        }
                        Action::Submit => self.submit(),
                        _ => {}
                    }
                    self.keyboard.suggest(&self.draft);
                }
                _ => {}
            }
            return;
        }
        let count = self.count();
        match event {
            RuntimeEvent::Rotate(d) => {
                self.cursor = (self.cursor as i32 + d.rem_euclid(count as i32))
                    .rem_euclid(count as i32) as usize;
                if self.mode == 0 {
                    if let Some(e) = self.listing.get(self.cursor) {
                        self.selected = e.path.to_string();
                    }
                }
            }
            RuntimeEvent::Press => match self.mode {
                0 => self.browse_press(),
                1 => self.context_press(),
                9 => match self.cursor {
                    0 => self.edit_line(self.menu_line),
                    1 => self.edit_line(self.lines()),
                    2 => self.save(),
                    _ => {
                        self.mode = 3;
                        self.cursor = 0;
                    }
                },
                10 => self.browse_again(),
                8 => match self.cursor {
                    0 => self.name(1),
                    1 => self.name(0),
                    2 => {
                        self.name(0);
                        self.draft = "note.txt".into();
                    }
                    3 => {
                        self.name(0);
                        self.draft = "config.json".into();
                    }
                    _ => self.browse_again(),
                },
                3 if self.cursor < self.lines() => self.edit_line(self.cursor),
                3 if self.cursor == self.lines() => self.edit_line(self.lines()),
                3 if self.cursor == self.lines() + 1 => self.save(),
                3 => {
                    self.back();
                }
                5 => match self.cursor {
                    0 => self.save(),
                    1 => {
                        self.dirty = false;
                        self.browse_again();
                    }
                    _ => {
                        self.mode = 3;
                        self.cursor = 0;
                    }
                },
                6 if self.cursor == 0 => {
                    self.queue(Op::Remove(self.selected.clone()), Purpose::Mutation)
                }
                6 => {
                    self.mode = 0;
                    self.cursor = 0;
                }
                _ => {}
            },
            _ => {}
        }
    }
    /// Проверяет введённые данные и применяет подтверждённую правку.
    fn submit(&mut self) {
        if self.mode == 4 {
            let (start, end) = self.range(self.line);
            let separator = usize::from(self.line >= self.lines());
            if self.document.len() - (end - start) + self.draft.len() + separator > fs::MAX_FILE {
                self.error = 1;
                return;
            }
            let mut next = self.document[..start].to_string();
            if separator != 0 {
                next.push('\n');
            }
            next.push_str(&self.draft);
            next.push_str(&self.document[end..]);
            self.document = next;
            self.dirty = true;
            self.mode = 3;
            self.cursor = self.line.min(self.lines());
        } else {
            let name = self.draft.trim();
            let name = if self.name_action == 0 && !name.contains('.') {
                alloc::format!("{}.txt", name)
            } else {
                name.to_string()
            };
            if self.draft.trim().is_empty() {
                self.error = 2;
                return;
            }
            let target = match fs::join(&self.dir, &name) {
                Ok(path) => path,
                Err(e) => {
                    self.message = e.label();
                    self.error = 2;
                    return;
                }
            };
            match self.name_action {
                0 => {
                    self.file = target.clone();
                    let initial = if target.ends_with(".json") {
                        b"{}\n".to_vec()
                    } else {
                        Vec::new()
                    };
                    self.queue(Op::Create(target, initial), Purpose::New);
                }
                1 => self.queue(Op::Mkdir(target), Purpose::Mutation),
                _ => self.queue(Op::Rename(self.selected.clone(), target), Purpose::Mutation),
            }
        }
    }
    /// Открывает выбранный элемент либо выполняет кнопку панели проводника.
    fn browse_press(&mut self) {
        if let Some(e) = self.listing.get(self.cursor) {
            self.selected = e.path.to_string();
            self.cursor = 0;
            self.context_press();
        } else {
            match self.cursor.saturating_sub(self.listing.len()) {
                0 => {
                    self.mode = 8;
                    self.cursor = 0;
                }
                1 => {
                    if self.selected.is_empty() {
                        self.message = "Сначала выберите файл или папку";
                    } else {
                        self.mode = 1;
                        self.cursor = 1;
                    }
                }
                2 => {
                    if let Some((from, cut)) = self.clipboard.clone() {
                        match fs::join(&self.dir, fs::basename(&from)) {
                            Ok(to) => {
                                self.queue(
                                    if cut {
                                        Op::Rename(from, to)
                                    } else {
                                        Op::Copy(from, to)
                                    },
                                    Purpose::Paste,
                                );
                            }
                            Err(e) => self.message = e.label(),
                        }
                    } else {
                        self.message = "Буфер копирования пуст";
                    }
                }
                _ => {
                    if !self.back() {
                        self.message = "Вы в корне документов";
                    }
                }
            }
        }
    }
    /// Выполняет действие над выбранным файлом или папкой.
    fn context_press(&mut self) {
        match self.cursor {
            0 => {
                let dir = self
                    .listing
                    .iter()
                    .any(|e| e.path.as_str() == self.selected && e.kind == 2);
                if dir {
                    self.dir = self.selected.clone();
                    self.mode = 0;
                    self.cursor = 0;
                    self.refresh();
                } else {
                    self.file = self.selected.clone();
                    if crate::apps::gallery::is_image(&self.file) {
                        crate::apps::gallery::request_open(&self.file);
                    } else {
                        self.queue(Op::Read(self.file.clone()), Purpose::Read);
                    }
                }
            }
            1 => self.name(2),
            2 | 3 => {
                self.clipboard = Some((self.selected.clone(), self.cursor == 3));
                self.browse_again();
                self.message = "Выберите папку и Вставить";
            }
            5 => {
                self.mode = 10;
                self.cursor = 0;
                self.message = "Свойства файла";
            }
            4 => {
                self.mode = 6;
                self.cursor = 1;
            }
            _ => {
                self.browse_again();
            }
        }
    }
    /// Возвращает фокус к тому же элементу, чтобы панель не действовала на другой файл.
    fn browse_again(&mut self) {
        self.mode = 0;
        self.cursor = self
            .listing
            .iter()
            .position(|e| e.path.as_str() == self.selected)
            .unwrap_or(0);
    }
    /// Возвращает число доступных целей навигации.
    fn count(&self) -> usize {
        match self.mode {
            1 => 7,
            9 => 4,
            10 => 1,
            3 => self.lines() + 3,
            5 => 3,
            6 => 2,
            8 => 5,
            _ => self.listing.len() + 4,
        }
    }
    /// Собирает снимок состояния для интерфейса.
    pub fn view(&self) -> View {
        let mut v = View {
            mode: self.mode,
            cursor: self.cursor as u16,
            count: self.count() as u16,
            keyboard: self.keyboard.view,
            bytes: if self.mode == 3 {
                self.document.len()
            } else {
                self.draft.len()
            } as u32,
            limit: if self.mode == 2 {
                fs::MAX_PATH as u32
            } else {
                fs::MAX_FILE as u32
            },
            error: self.error,
            message: self.message,
            busy: self.pending.is_some(),
            dirty: self.dirty,
            ..View::default()
        };
        let title = match self.mode {
            2 => {
                if self.name_action == 1 {
                    "Новая папка"
                } else if self.name_action == 2 {
                    "Переименовать"
                } else {
                    "Новый файл"
                }
            }
            4 => "Изменить строку",
            1 | 6 => fs::basename(&self.selected),
            3 | 5 => fs::basename(&self.file),
            _ => &self.dir,
        };
        v.title_len = copy_text(&mut v.title, title);
        v.items = self.listing.len() as u8;
        v.clipboard = self.clipboard.is_some();
        v.can_up = self.dir != "/";
        v.selected_size = self
            .listing
            .iter()
            .find(|e| e.path.as_str() == self.selected)
            .map_or(0, |e| e.size);
        v.selected_len = copy_text(&mut v.selected_name, fs::basename(&self.selected));
        if self.mode == 0 && self.cursor >= self.listing.len() {
            v.toolbar = (self.cursor - self.listing.len()) as u8;
        }
        let mut start = self.draft.len().saturating_sub(v.input.len());
        while !self.draft.is_char_boundary(start) {
            start += 1;
        }
        v.input_len = copy_text(&mut v.input, &self.draft[start..]);
        let list_cursor = if self.mode == 0 && self.cursor >= self.listing.len() {
            self.listing
                .iter()
                .position(|e| e.path.as_str() == self.selected)
                .unwrap_or(0)
        } else {
            self.cursor
        };
        let visible_count = if self.mode == 0 {
            self.listing.len()
        } else {
            self.count()
        };
        let window = if self.mode == 0 { 4 } else { 6 };
        let start = list_cursor
            .saturating_sub(1)
            .min(visible_count.saturating_sub(window));
        v.first_row = start as u16;
        for (row, target) in v.rows.iter_mut().zip(start..start + window) {
            if target >= visible_count {
                break;
            }
            row.selected = target == self.cursor;
            let text = match self.mode {
                1 => [
                    "Открыть",
                    "Переименовать",
                    "Копировать",
                    "Вырезать",
                    "Удалить",
                    "Свойства",
                    "Назад",
                ][target],
                5 => ["Сохранить", "Не сохранять", "Отмена"][target],
                6 => ["Удалить навсегда", "Отмена"][target],
                9 => ["Изменить строку", "Добавить строку", "Сохранить", "Назад"][target],
                10 => "Назад",
                8 => [
                    "Папку",
                    "Файл",
                    "Текстовый файл (.txt)",
                    "JSON файл (.json)",
                    "Назад",
                ][target],
                3 => {
                    if target < self.lines() {
                        self.document
                            .split('\n')
                            .nth(target)
                            .filter(|s| !s.is_empty())
                            .unwrap_or("(пустая строка)")
                    } else {
                        ["Добавить строку", "Сохранить", "Назад"][target - self.lines()]
                    }
                }
                _ => {
                    if let Some(e) = self.listing.get(target) {
                        row.kind = e.kind;
                        row.size = e.size;
                        fs::basename(&e.path)
                    } else {
                        ["Новый файл", "Новая папка", "Вставить", "На уровень выше"]
                            [target - self.listing.len()]
                    }
                }
            };
            row.len = copy_text(&mut row.text, text);
        }
        v
    }
}

impl Drop for Controller {
    /// Освобождает принадлежащие экземпляру ресурсы и незавершённые запросы.
    fn drop(&mut self) {
        if let Some((token, _)) = self.pending {
            fs::cancel(token);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    /// Выполняет одну отложенную дисковую операцию и доставляет ответ тестовому контроллеру.
    fn serve(c: &mut Controller, disk: &impl embedded_sdmmc::BlockDevice) {
        fs::service(disk);
        c.poll();
    }
    /// Проверяет прямое открытие папки и применение кнопок панели к выбранному файлу.
    #[test]
    fn browser_opens_directly_and_toolbar_keeps_selected_file() {
        fs::reset_for_test();
        let disk = crate::file_store::tests::fixture();
        let mut c = Controller::default();
        c.open(false);
        serve(&mut c, &disk);
        for operation in [
            Op::Mkdir("/Folder".into()),
            Op::Create("/n.txt".into(), b"note".to_vec()),
        ] {
            let token = fs::request(operation).unwrap();
            fs::service(&disk);
            fs::take_reply(token).unwrap();
            c.poll();
        }
        c.event(RuntimeEvent::Press);
        assert_eq!(c.dir, "/Folder");
        assert!(c.back());
        c.event(RuntimeEvent::Rotate(1));
        assert_eq!(c.selected, "/n.txt");
        c.event(RuntimeEvent::Rotate(2));
        assert_eq!(c.view().toolbar, 1);
        c.event(RuntimeEvent::Press);
        assert_eq!(c.mode, 1);
        c.event(RuntimeEvent::Rotate(1));
        c.event(RuntimeEvent::Press); // Копировать.
        c.event(RuntimeEvent::Rotate(-1));
        c.event(RuntimeEvent::Press);
        assert_eq!(c.dir, "/Folder");
        c.event(RuntimeEvent::Rotate(2));
        c.event(RuntimeEvent::Press);
        serve(&mut c, &disk);
        assert!(c.listing.iter().any(|e| e.path == "/Folder/n.txt"));
        c.cursor = c.listing.len();
        c.event(RuntimeEvent::Press);
        assert_eq!(c.mode, 8);
        c.event(RuntimeEvent::Press);
        assert_eq!(c.mode, 2);
    }
    #[test]
    /// Проверяет сохранение нескольких UTF-8-строк и чтение после повторного открытия хранилища.
    fn editor_saves_utf8_lines_and_restores_from_disk() {
        fs::reset_for_test();
        let disk = crate::file_store::tests::fixture();
        let mut c = Controller::default();
        c.open(false);
        serve(&mut c, &disk);
        c.name(0);
        c.draft = "Заметка".to_string();
        c.submit();
        serve(&mut c, &disk);
        assert_eq!(c.mode, 3);
        c.edit_line(0);
        c.draft = "Привет\nмир".to_string();
        c.submit();
        assert!(c.dirty);
        c.save();
        serve(&mut c, &disk);
        assert!(!c.dirty);
        drop(c);
        fs::reset_for_test();
        let restored = fs::Fs::mount(&disk).unwrap();
        assert_eq!(
            restored.read(&disk, "/Заметка.txt").unwrap(),
            "Привет\nмир".as_bytes()
        );
    }
    #[test]
    fn physical_clicks_select_open_and_show_document_menu() {
        fs::reset_for_test();
        let disk = crate::file_store::tests::fixture();
        let token = fs::request(Op::Create("/n.txt".into(), b"line\nnext".to_vec())).unwrap();
        fs::service(&disk);
        fs::take_reply(token).unwrap();
        let mut c = Controller::default();
        c.open(false);
        serve(&mut c, &disk);
        c.event(RuntimeEvent::Click(1));
        assert_eq!(c.mode, 0);
        c.event(RuntimeEvent::Click(3));
        assert_eq!(c.mode, 1);
        c.back();
        c.event(RuntimeEvent::Click(2));
        serve(&mut c, &disk);
        assert_eq!(c.mode, 3);
        c.event(RuntimeEvent::Click(1));
        assert_eq!(c.mode, 3);
        c.event(RuntimeEvent::Click(3));
        assert_eq!(c.mode, 9);
        c.back();
        c.event(RuntimeEvent::Click(2));
        assert_eq!(c.mode, 4);
        drop(c);
        fs::reset_for_test();
    }
    #[test]
    fn json_creation_preserves_extension_and_starts_with_valid_json() {
        fs::reset_for_test();
        let disk = crate::file_store::tests::fixture();
        let mut c = Controller::default();
        c.open(false);
        serve(&mut c, &disk);
        c.name(0);
        c.draft = "config.json".into();
        c.submit();
        serve(&mut c, &disk);
        assert_eq!(c.file, "/config.json");
        drop(c);
        fs::reset_for_test();
        assert_eq!(
            fs::Fs::mount(&disk)
                .unwrap()
                .read(&disk, "/config.json")
                .unwrap(),
            b"{}\n"
        );
    }
}

#[path = "../TextEditor/main.rs"]
mod editor;
