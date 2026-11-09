// Модуль задач, расходов и рабочих сессий.
//! Durable task list and expense ledger, plus a monotonic work/break session.
use super::files::{Row, copy_text};
use crate::{
    file_store::{self as fs, Error, Op},
    runtime::RuntimeEvent,
    text_input::{Action, Keyboard, KeyboardView},
};
use alloc::{
    string::{String, ToString},
    vec::Vec,
};
#[derive(Clone)]
/// Разобранная запись журнала или пользовательского списка компонента задач, расходов и рабочих сессий.
struct Record {
    /// Текстовое содержимое в ограниченном буфере.
    text: String,
    /// Входное число или сумма в единицах конкретной модели.
    amount: i64,
    /// Признак выполненной задачи либо завершённой операции.
    done: bool,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Снимок данных для отображения компонента задач, расходов и рабочих сессий.
pub struct View {
    /// Подготовленные видимые строки списка.
    pub rows: [Row; 3],
    /// Индекс текущей цели навигации.
    pub cursor: u8,
    /// Количество элементов или выполненных действий в текущем контексте.
    pub count: u8,
    /// Признак открытой клавиатуры редактора.
    pub editing: bool,
    /// Признак экрана явного подтверждения удаления.
    pub confirming: bool,
    /// Признак подтверждения отказа от несохранённых изменений.
    pub discarding: bool,
    /// Входные данные или временно введённые символы.
    pub input: [u8; 96],
    /// Число значащих байтов буфера input.
    pub input_len: u8,
    /// Текущее количество байтов пользовательского ввода.
    pub bytes: u16,
    /// Состояние общей RU/EN-клавиатуры.
    pub keyboard: KeyboardView,
    /// Ошибка последней операции или признак ошибки.
    pub error: u8,
    /// Сообщение или его код для вывода пользователю.
    pub message: &'static str,
    /// Итог расходов в копейках либо число выполненных задач.
    pub total: i64,
    /// Признак изменений, которые ещё не подтверждены записью.
    pub dirty: bool,
    /// Признак ещё выполняющейся операции.
    pub busy: bool,
}
impl Default for View {
    /// Задаёт исходное состояние нового экземпляра.
    fn default() -> Self {
        Self {
            rows: [Row::default(); 3],
            cursor: 0,
            count: 3,
            editing: false,
            confirming: false,
            discarding: false,
            input: [0; 96],
            input_len: 0,
            bytes: 0,
            keyboard: KeyboardView::default(),
            error: 0,
            message: "",
            total: 0,
            dirty: false,
            busy: false,
        }
    }
}
/// Постоянный список задач либо расходов компонента задач, расходов и рабочих сессий.
pub struct Ledger {
    /// Выбор журнала расходов вместо списка задач.
    expenses: bool,
    /// Ограниченный список пользовательских записей.
    records: Vec<Record>,
    /// Индекс текущей цели навигации.
    cursor: usize,
    /// Текущий режим компонента; переходы выполняет обработчик событий.
    mode: u8,
    /// Выбранный элемент либо признак его выделения.
    selected: usize,
    /// Черновик ввода; его изменение ещё не означает запись на носитель.
    draft: String,
    /// Состояние общей RU/EN-клавиатуры.
    keyboard: Keyboard,
    /// Ошибка последней операции или признак ошибки.
    error: u8,
    /// Сообщение или его код для вывода пользователю.
    message: &'static str,
    /// Необработанный запрос или ожидающий результат.
    pending: Option<(u32, bool)>,
    /// Контрольная сумма содержимого для обнаружения повреждений.
    crc: Option<u32>,
    /// Признак изменений, которые ещё не подтверждены записью.
    dirty: bool,
    /// Признак успешного чтения начального состояния.
    loaded: bool,
}
impl Default for Ledger {
    /// Задаёт исходное состояние нового экземпляра.
    fn default() -> Self {
        Self {
            expenses: false,
            records: Vec::new(),
            cursor: 0,
            mode: 0,
            selected: 0,
            draft: String::new(),
            keyboard: Keyboard::default(),
            error: 0,
            message: "",
            pending: None,
            crc: None,
            dirty: false,
            loaded: false,
        }
    }
}
impl Ledger {
    /// Открывает выбранный режим и подготавливает его состояние.
    pub fn open(&mut self) {
        if !self.loaded && self.pending.is_none() {
            self.load();
        }
    }
    /// Загружает сохранённые данные из хранилища.
    fn load(&mut self) {
        self.queue(Op::Read(self.path().to_string()), false);
    }
    /// Ставит операцию в очередь и запоминает её идентификатор.
    fn queue(&mut self, op: Op, save: bool) {
        match fs::request(op) {
            Ok(id) => {
                self.pending = Some((id, save));
                self.message = "Чтение/сохранение…";
            }
            Err(e) => self.message = e.label(),
        }
    }
    /// Преобразует состояние в формат хранения или передачи.
    fn encode(&self) -> Vec<u8> {
        let mut s = String::new();
        for r in &self.records {
            s.push_str(&alloc::format!(
                "{}|{}\n",
                if self.expenses {
                    r.amount
                } else {
                    i64::from(r.done)
                },
                r.text
            ));
        }
        s.into_bytes()
    }
    /// Ставит сохранение пользовательских изменений в дисковую очередь.
    fn save(&mut self) {
        let bytes = self.encode();
        if bytes.len() > fs::MAX_FILE {
            self.message = "Слишком длинный список";
            return;
        }
        let op = if self.crc.is_some() {
            Op::Write(self.path().to_string(), bytes, self.crc)
        } else {
            Op::Create(self.path().to_string(), bytes)
        };
        self.queue(op, true);
    }
    /// Выполняет очередной неблокирующий шаг обработки и проверяет готовые результаты.
    pub fn poll(&mut self) {
        let Some((token, save)) = self.pending else {
            return;
        };
        let Some(reply) = fs::take_reply(token) else {
            return;
        };
        self.pending = None;
        match reply.result {
            Ok(_) if save => {
                self.crc = Some(crate::file_store::content_crc(&self.encode()));
                self.dirty = false;
                self.message = "Сохранено";
                self.loaded = true;
            }
            Ok(Some(data)) => {
                let parsed = core::str::from_utf8(&data).ok().and_then(|s| {
                    let mut records = Vec::new();
                    for line in s.lines().filter(|s| !s.is_empty()) {
                        let (amount, text) = line.split_once('|')?;
                        let amount = amount.parse::<i64>().ok()?;
                        if records.len() >= 24
                            || text.len() > 128
                            || text.chars().any(char::is_control)
                            || (!self.expenses && !matches!(amount, 0 | 1))
                            || amount.unsigned_abs() > 99_999_999
                        {
                            return None;
                        }
                        records.push(Record {
                            text: text.to_string(),
                            amount,
                            done: amount != 0,
                        });
                    }
                    Some(records)
                });
                if let Some(records) = parsed {
                    self.records = records;
                    self.crc = Some(crate::file_store::content_crc(&data));
                    self.loaded = true;
                    self.dirty = false;
                    self.message = "";
                    self.cursor = 0;
                } else {
                    self.message = "Ошибка формата списка";
                }
            }
            Err(Error::NotFound) if !save => {
                self.loaded = true;
                self.records.clear();
                self.crc = None;
                self.message = "";
            }
            Err(e) => self.message = e.label(),
            _ => {}
        }
    }
    /// Возвращается на предыдущий экран; сообщает, был ли переход обработан.
    pub fn back(&mut self) -> bool {
        if self.pending.is_some() {
            return true;
        }
        if self.mode != 0 {
            self.mode = 0;
            self.cursor = 0;
            true
        } else {
            false
        }
    }
    /// Обрабатывает событие интерфейса и выполняет соответствующий переход состояния.
    pub fn event(&mut self, event: RuntimeEvent) {
        self.poll();
        if self.pending.is_some() {
            return;
        }
        if self.mode == 1 {
            match event {
                RuntimeEvent::Rotate(d) => self.keyboard.rotate(d),
                RuntimeEvent::Press => {
                    self.error = 0;
                    match self.keyboard.press() {
                        Action::Character(c)
                            if self.draft.len() + c.len_utf8() <= 128
                                && !c.is_control()
                                && c != '|' =>
                        {
                            self.draft.push(c)
                        }
                        Action::Character(_) => self.error = 1,
                        Action::Backspace => {
                            self.draft.pop();
                        }
                        Action::Cancel => {
                            self.back();
                        }
                        Action::Suggestion(word) => {
                            let start = self.draft.rfind(' ').map_or(0, |i| i + 1);
                            if start + word.len() + 1 <= 128 {
                                self.draft.truncate(start);
                                self.draft.push_str(word);
                                self.draft.push(' ');
                            } else {
                                self.error = 1;
                            }
                        }
                        Action::Submit => {
                            let value = self.draft.trim();
                            let parsed = if self.expenses {
                                value.split_once(' ').and_then(|(number, text)| {
                                    parse_money(number).map(|amount| (text.trim(), amount))
                                })
                            } else {
                                Some((value, 0))
                            };
                            if let Some((text, amount)) = parsed.filter(|(s, _)| !s.is_empty()) {
                                if self.records.len() < 24 {
                                    self.records.push(Record {
                                        text: text.to_string(),
                                        amount,
                                        done: false,
                                    });
                                    self.dirty = true;
                                    self.mode = 0;
                                    self.cursor = self.records.len() - 1;
                                    self.save();
                                } else {
                                    self.message = "Лимит: 24 записи";
                                    self.mode = 0;
                                }
                            } else {
                                self.error = 2;
                                self.message = if self.expenses {
                                    "Введите: 12.50 продукты"
                                } else {
                                    "Введите название задачи"
                                };
                            }
                        }
                        _ => {}
                    }
                    self.keyboard.suggest(&self.draft);
                }
                _ => {}
            }
            return;
        }
        let count = if self.mode >= 2 {
            2
        } else {
            self.records.len() + if self.expenses { 3 } else { 4 }
        };
        match event {
            RuntimeEvent::Rotate(d) => {
                self.cursor = (self.cursor as i32 + d.rem_euclid(count as i32))
                    .rem_euclid(count as i32) as usize
            }
            RuntimeEvent::Press if self.mode >= 2 => {
                if self.cursor == 0 {
                    if self.mode == 2 {
                        self.records.remove(self.selected);
                        self.dirty = true;
                        self.save();
                    } else {
                        self.load();
                    }
                }
                self.mode = 0;
                self.cursor = 0;
            }
            RuntimeEvent::Press if self.cursor < self.records.len() => {
                if self.expenses {
                    self.selected = self.cursor;
                    self.mode = 2;
                    self.cursor = 1;
                } else {
                    self.records[self.cursor].done = !self.records[self.cursor].done;
                    self.dirty = true;
                    self.save();
                }
            }
            RuntimeEvent::Press => match self.cursor - self.records.len() {
                0 if self.loaded => {
                    self.draft.clear();
                    self.keyboard = Keyboard::default();
                    self.mode = 1;
                    self.error = 0;
                }
                0 => self.load(),
                1 if self.loaded => self.save(),
                2 if !self.expenses && self.loaded => {
                    self.records.retain(|r| !r.done);
                    self.dirty = true;
                    self.cursor = 0;
                    self.save();
                }
                _ if !self.dirty => self.load(),
                _ => {
                    self.mode = 3;
                    self.cursor = 1;
                }
            },
            _ => {}
        }
    }
    /// Собирает снимок состояния для интерфейса.
    pub fn view(&self) -> View {
        let mut v = View {
            editing: self.mode == 1,
            confirming: self.mode >= 2,
            discarding: self.mode == 3,
            cursor: self.cursor as u8,
            count: (if self.mode >= 2 {
                2
            } else {
                self.records.len() + if self.expenses { 3 } else { 4 }
            }) as u8,
            bytes: self.draft.len() as u16,
            keyboard: self.keyboard.view,
            error: self.error,
            message: self.message,
            total: if self.expenses {
                self.records.iter().map(|r| r.amount).sum()
            } else {
                self.records.iter().filter(|r| r.done).count() as i64
            },
            dirty: self.dirty,
            busy: self.pending.is_some(),
            ..View::default()
        };
        let mut start = self.draft.len().saturating_sub(96);
        while !self.draft.is_char_boundary(start) {
            start += 1;
        }
        v.input_len = copy_text(&mut v.input, &self.draft[start..]);
        let start = self
            .cursor
            .saturating_sub(1)
            .min(usize::from(v.count).saturating_sub(3));
        for (target, row) in (start..start + 3).zip(&mut v.rows) {
            if target >= usize::from(v.count) {
                break;
            }
            row.selected = target == self.cursor;
            let text = if self.mode >= 2 {
                [
                    if self.mode == 3 {
                        "Перечитать с диска"
                    } else {
                        "Удалить запись"
                    },
                    "Отмена",
                ][target]
                    .to_string()
            } else if let Some(r) = self.records.get(target) {
                if self.expenses {
                    alloc::format!("{} · {}", money(r.amount), r.text)
                } else {
                    alloc::format!("{} {}", if r.done { "[x]" } else { "[ ]" }, r.text)
                }
            } else {
                [
                    "Добавить",
                    "Сохранить",
                    if self.expenses {
                        "Перечитать"
                    } else {
                        "Очистить готовые"
                    },
                    "Перечитать",
                ][target - self.records.len()]
                .to_string()
            };
            row.len = copy_text(&mut row.text, &text);
        }
        v
    }
}
/// Форматирует целые копейки без двоичного округления.
pub fn money(amount: i64) -> String {
    alloc::format!(
        "{}{}.{:02}",
        if amount < 0 { "-" } else { "" },
        amount.unsigned_abs() / 100,
        amount.unsigned_abs() % 100
    )
}
/// Разбирает сумму с точкой или запятой в целое число копеек.
pub fn parse_money(raw: &str) -> Option<i64> {
    let negative = raw.starts_with('-');
    let value = raw.strip_prefix('-').unwrap_or(raw);
    let mut parts = value.split(['.', ',']);
    let integer = parts.next()?;
    if integer.is_empty() || !integer.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let mut amount = integer.parse::<i64>().ok()?.checked_mul(100)?;
    if let Some(fraction) = parts.next() {
        if fraction.is_empty()
            || fraction.len() > 2
            || !fraction.bytes().all(|b| b.is_ascii_digit())
        {
            return None;
        }
        amount = amount.checked_add(
            fraction.parse::<i64>().ok()? * if fraction.len() == 1 { 10 } else { 1 },
        )?;
    }
    if parts.next().is_some() || amount > 99_999_999 {
        return None;
    }
    Some(if negative { -amount } else { amount })
}
pub use focus::{Focus, FocusView};
#[path = "../Focus/main.rs"]
mod focus;
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    /// Проверяет точный разбор копеек и отказ при неправильной дроби или переполнении.
    fn money_uses_exact_cents_and_rejects_overflow() {
        assert_eq!(parse_money("12,50"), Some(1250));
        assert_eq!(parse_money("-0.5"), Some(-50));
        assert_eq!(parse_money("1.001"), None);
        assert_eq!(parse_money("99999999999999999999999999"), None);
        assert_eq!(money(-50), "-0.50");
    }
    #[test]
    /// Проверяет паузу, продолжение и переход рабочего этапа к перерыву.
    fn focus_pause_and_completion_preserve_remaining_time() {
        crate::time::set(100);
        let mut f = Focus::default();
        f.event(RuntimeEvent::Press);
        crate::time::set(130);
        f.event(RuntimeEvent::Press);
        assert_eq!(f.view().remaining, 1470);
        crate::time::set(200);
        assert_eq!(f.view().remaining, 1470);
        f.event(RuntimeEvent::Press);
        crate::time::set(1670);
        f.poll();
        assert_eq!(f.view().phase, 1);
        assert_eq!(f.view().sessions, 1);
        assert!(!f.view().running);
        assert_eq!(f.view().remaining, 300);
    }
}

impl Drop for Ledger {
    /// Освобождает принадлежащие экземпляру ресурсы и незавершённые запросы.
    fn drop(&mut self) {
        if let Some((token, _)) = self.pending {
            fs::cancel(token);
        }
    }
}

impl Ledger {
    fn path(&self) -> &str {
        "/tasks.txt"
    }
}
