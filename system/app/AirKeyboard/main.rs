//! Air Keyboard stage 1: explicit encoder test input, no simulated IMU tracking.
mod editor;
pub mod keys;
use crate::{
    file_store as fs,
    runtime::{BluetoothUiAction, RuntimeEvent},
};
use editor::Editor;
use keys::Key;
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Output {
    #[default]
    Local,
    Hid,
    Mirror,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct View {
    pub text: [u8; 1024],
    pub len: u16,
    pub caret: u16,
    pub selected: u8,
    pub russian: bool,
    pub symbols: bool,
    pub shift: bool,
    pub inline: bool,
    pub menu: bool,
    pub menu_index: u8,
    pub preview: Option<char>,
    pub output: Output,
    pub armed: bool,
    pub ready: bool,
    pub modifiers: u8,
    pub clear_confirm: bool,
    pub discard_confirm: bool,
    pub message: &'static str,
}
impl Default for View {
    fn default() -> Self {
        Self {
            text: [0; 1024],
            len: 0,
            caret: 0,
            selected: 0,
            russian: true,
            symbols: false,
            shift: false,
            inline: false,
            menu: false,
            menu_index: 0,
            preview: None,
            output: Output::Local,
            armed: false,
            ready: false,
            modifiers: 0,
            clear_confirm: false,
            discard_confirm: false,
            message: "MPU6050 нет · тест энкодером",
        }
    }
}
#[derive(Clone, Copy)]
enum Job {
    Load,
    Preferences,
    Export,
}
pub struct Controller {
    editor: Editor,
    pub(crate) selected: usize,
    russian: bool,
    symbols: bool,
    shift: bool,
    pub(crate) inline: bool,
    menu: bool,
    menu_index: u8,
    output: Output,
    armed: bool,
    ready: bool,
    held: bool,
    active: bool,
    modifiers: u8,
    clear_confirm: bool,
    discard_confirm: bool,
    loaded: bool,
    dirty: bool,
    touched: bool,
    pending: Option<(u32, Job)>,
    queue: heapless::Vec<BluetoothUiAction, 8>,
    message: &'static str,
}
impl Default for Controller {
    fn default() -> Self {
        Self {
            editor: Editor::default(),
            selected: 0,
            russian: true,
            symbols: false,
            shift: false,
            inline: false,
            menu: false,
            menu_index: 0,
            output: Output::Local,
            armed: false,
            ready: false,
            held: false,
            active: false,
            modifiers: 0,
            clear_confirm: false,
            discard_confirm: false,
            loaded: false,
            dirty: false,
            touched: false,
            pending: None,
            queue: heapless::Vec::new(),
            message: "MPU6050 нет · тест энкодером",
        }
    }
}
impl Controller {
    pub fn open(&mut self) {
        self.active = true;
        self.menu = false;
        self.armed = false;
        self.held = false;
        self.queue.clear();
        self.clear_confirm = false;
        self.discard_confirm = false;
        self.message = "MPU6050 нет · тест энкодером";
    }
    pub fn close(&mut self) {
        if let Some((token, _)) = self.pending.take() {
            fs::cancel(token);
        }
        self.active = false;
        self.armed = false;
        self.held = false;
        self.queue.clear();
        self.editor = Editor::default();
        self.modifiers = 0;
    }
    pub fn connection(&mut self, ready: bool) {
        self.ready = ready;
        if !ready {
            self.armed = false;
            self.queue.clear();
        }
    }
    fn start(&mut self, op: fs::Op, job: Job) -> bool {
        if self.pending.is_some() {
            self.message = "Дождитесь записи";
            return false;
        }
        match fs::request(op) {
            Ok(token) => {
                self.pending = Some((token, job));
                self.message = "Операция с хранилищем…";
                true
            }
            Err(_) => {
                self.message = "Хранилище занято или закрыто";
                false
            }
        }
    }
    pub fn poll(&mut self) {
        if !self.active {
            return;
        }
        if !fs::external_access() {
            self.close();
            return;
        }
        if let Some((token, job)) = self.pending {
            if let Some(reply) = fs::take_reply(token) {
                self.pending = None;
                match job {
                    Job::Load => {
                        self.loaded = true;
                        self.message = "MPU6050 нет · тест энкодером";
                        if !self.touched {
                            if let Ok(Some(data)) = reply.result {
                                if data.len() == 6
                                    && &data[..4] == b"AK01"
                                    && data[4] <= 1
                                    && data[5] <= 1
                                {
                                    self.inline = data[4] != 0;
                                    self.russian = data[5] != 0;
                                    self.selected = 0;
                                }
                            }
                        }
                    }
                    Job::Preferences => {
                        self.message = if reply.result.is_ok() {
                            "Настройки сохранены"
                        } else {
                            "Не удалось сохранить настройки"
                        };
                    }
                    Job::Export => {
                        self.message = if reply.result.is_ok() {
                            "Создан /AirKeyboard.txt"
                        } else {
                            "Файл уже есть или запись недоступна"
                        };
                    }
                }
            }
        }
        if self.pending.is_none() {
            if !self.loaded {
                self.start(
                    fs::Op::ReadRange("/AirKeyboard.cfg".into(), 0, 7),
                    Job::Load,
                );
            } else if self.dirty {
                let data = alloc::vec![
                    b'A',
                    b'K',
                    b'0',
                    b'1',
                    self.inline as u8,
                    self.russian as u8
                ];
                if self.start(
                    fs::Op::Write("/AirKeyboard.cfg".into(), data, None),
                    Job::Preferences,
                ) {
                    self.dirty = false;
                }
            }
        }
    }
    fn preference_changed(&mut self) {
        self.touched = true;
        self.dirty = true;
        self.selected = 0;
    }
    pub fn hover(&mut self, index: usize) {
        self.selected = index.min(keys::count(self.russian, self.symbols) - 1);
        self.clear_confirm = false;
    }
    /// Future gesture input must call both edges. Repeated down samples cannot
    /// type twice; this prototype calls the same contract from an encoder press.
    pub fn confirm(&mut self, down: bool) {
        if !down {
            self.held = false;
            return;
        }
        if self.held || !self.active || self.menu || !fs::external_access() {
            return;
        }
        self.held = true;
        let key = keys::key(self.russian, self.symbols, self.selected, self.shift);
        match key {
            Key::Shift => {
                self.shift = !self.shift;
                return;
            }
            Key::Language => {
                self.russian = !self.russian;
                self.preference_changed();
                return;
            }
            Key::Symbols => {
                self.symbols = !self.symbols;
                self.selected = 0;
                return;
            }
            _ => {}
        }
        if self.output == Output::Mirror && self.modifiers == 0 {
            if let Some(c) = keys::character(key) {
                if self.editor.text.len() + c.len_utf8() > editor::LIMIT
                    || self.editor.text.try_reserve(c.len_utf8()).is_err()
                {
                    self.message = "Лимит текста: 1024 байта";
                    return;
                }
            }
        }
        if self.output != Output::Local {
            if !self.armed || !self.ready {
                self.message = "HID выключен: включите отправку в меню";
                return;
            }
            let Some(report) = keys::hid(key, self.modifiers) else {
                self.message = "Клавиша не поддерживается HID";
                return;
            };
            if self.queue.is_full() {
                self.message = "Очередь HID занята";
                return;
            }
            // Mirror is only a record of our own actions. There is no text
            // synchronization or delivery acknowledgement from the host editor.
            let _ = self.queue.push(BluetoothUiAction::Keyboard(report));
            self.message = "Клавиша в очереди HID";
        }
        if self.output != Output::Hid && self.modifiers == 0 {
            if let Some(c) = keys::character(key) {
                if !self.editor.insert(c) {
                    self.message = "Лимит текста: 1024 байта";
                }
            } else {
                match key {
                    Key::Backspace => self.editor.backspace(),
                    Key::Left => self.editor.left(),
                    Key::Right => self.editor.right(),
                    Key::Up => self.editor.vertical(false),
                    Key::Down => self.editor.vertical(true),
                    _ => {}
                }
            }
        }
        self.shift = false;
    }
    pub fn back(&mut self) -> bool {
        if self.menu {
            self.menu = false;
            self.clear_confirm = false;
            self.discard_confirm = false;
            true
        } else if !self.editor.text.is_empty() {
            self.menu = true;
            self.menu_index = 9;
            self.message = "Экспортируйте текст или подтвердите выход";
            true
        } else {
            false
        }
    }
    pub fn take(&mut self) -> Option<BluetoothUiAction> {
        if !self.active || !fs::external_access() {
            self.armed = false;
            self.queue.clear();
            return None;
        }
        if self.queue.is_empty() {
            None
        } else {
            Some(self.queue.remove(0))
        }
    }
    /// true asks the native router to close this app.
    pub fn event(&mut self, event: RuntimeEvent) -> bool {
        if !self.active || !fs::external_access() {
            return false;
        }
        if matches!(event, RuntimeEvent::Click(2)) {
            self.menu = !self.menu;
            self.menu_index = 0;
            self.clear_confirm = false;
            self.discard_confirm = false;
            self.held = false;
            return false;
        }
        let delta = match event {
            RuntimeEvent::Rotate(d) => Some(d),
            RuntimeEvent::Swipe(d) => Some(d.signum()),
            _ => None,
        };
        if let Some(delta) = delta {
            if self.menu {
                self.menu_index =
                    (i64::from(self.menu_index) + i64::from(delta)).rem_euclid(10) as u8;
                self.clear_confirm = false;
                self.discard_confirm = false;
            } else {
                self.hover(
                    (self.selected as i64 + i64::from(delta)).rem_euclid(keys::count(
                        self.russian,
                        self.symbols,
                    )
                        as i64) as usize,
                );
            }
        } else if matches!(event, RuntimeEvent::Press | RuntimeEvent::Click(_)) {
            if !self.menu {
                self.confirm(true);
                self.confirm(false);
            } else {
                match self.menu_index {
                    0 => {
                        self.inline = !self.inline;
                        self.preference_changed();
                    }
                    1 => {
                        self.russian = !self.russian;
                        self.preference_changed();
                    }
                    2 => {
                        self.output = match self.output {
                            Output::Local => Output::Hid,
                            Output::Hid => Output::Mirror,
                            Output::Mirror => Output::Local,
                        };
                        self.armed = false;
                        self.queue.clear();
                        self.modifiers = 0;
                    }
                    3 => {
                        self.armed = self.output != Output::Local && self.ready && !self.armed;
                        self.message = if self.armed {
                            "HID включён · раскладка на приёмнике"
                        } else {
                            "HID выключен или нет подключения"
                        };
                    }
                    4 if !self.editor.text.is_empty() && !self.discard_confirm => {
                        self.discard_confirm = true;
                        self.message = "Сопряжение закроет текст: нажмите ещё раз";
                    }
                    4 => {
                        self.armed = false;
                        self.queue.clear();
                        let _ = self.queue.push(BluetoothUiAction::Pair);
                    }
                    5 => {
                        self.start(
                            fs::Op::Create(
                                "/AirKeyboard.txt".into(),
                                self.editor.text.as_bytes().to_vec(),
                            ),
                            Job::Export,
                        );
                    }
                    6 if self.clear_confirm => {
                        self.editor = Editor::default();
                        self.clear_confirm = false;
                    }
                    6 => {
                        self.clear_confirm = true;
                        self.message = "Нажмите ещё раз: очистить текст";
                    }
                    7 => {
                        self.modifiers = if self.output == Output::Local {
                            0
                        } else {
                            match self.modifiers {
                                0 => 1,
                                1 => 4,
                                4 => 5,
                                _ => 0,
                            }
                        };
                    }
                    8 => self.message = "Калибровка: сначала установите MPU6050",
                    9 if !self.editor.text.is_empty() && !self.discard_confirm => {
                        self.discard_confirm = true;
                        self.message = "Нажмите ещё раз: закрыть без сохранения";
                    }
                    9 => return true,
                    _ => {}
                }
            }
        }
        false
    }
    pub fn view(&self) -> View {
        let mut v = View {
            len: self.editor.text.len() as u16,
            caret: self.editor.caret as u16,
            selected: self.selected as u8,
            russian: self.russian,
            symbols: self.symbols,
            shift: self.shift,
            inline: self.inline,
            menu: self.menu,
            menu_index: self.menu_index,
            preview: if self.active && !self.menu {
                match keys::key(self.russian, self.symbols, self.selected, self.shift) {
                    Key::Character(c) => Some(c),
                    Key::Space => Some(' '),
                    _ => None,
                }
            } else {
                None
            },
            output: self.output,
            armed: self.armed,
            ready: self.ready,
            modifiers: self.modifiers,
            clear_confirm: self.clear_confirm,
            discard_confirm: self.discard_confirm,
            message: self.message,
            ..View::default()
        };
        v.text[..self.editor.text.len()].copy_from_slice(self.editor.text.as_bytes());
        v
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preferences_round_trip_and_export_contains_only_committed_text() {
        fs::reset_for_test();
        let disk = fs::tests::fixture();
        let mut c = Controller::default();
        c.open();
        c.poll();
        fs::service(&disk);
        c.poll();
        c.inline = true;
        c.russian = false;
        c.preference_changed();
        c.poll();
        fs::service(&disk);
        c.poll();
        c.close();
        let mut restored = Controller::default();
        restored.open();
        restored.poll();
        fs::service(&disk);
        restored.poll();
        assert!(restored.inline);
        assert!(!restored.russian);
        assert!(!restored.armed);
        assert_eq!(restored.output, Output::Local);
        for ch in "Привет\nмир".chars() {
            assert!(restored.editor.insert(ch));
        }
        restored.hover(0); // q is only the temporary next character.
        restored.menu = true;
        restored.menu_index = 5;
        restored.event(RuntimeEvent::Press);
        fs::service(&disk);
        restored.poll();
        let files = fs::Fs::mount(&disk).unwrap();
        assert_eq!(
            files.read(&disk, "/AirKeyboard.txt").unwrap(),
            "Привет\nмир".as_bytes()
        );
        restored.editor.insert('!');
        restored.event(RuntimeEvent::Press);
        fs::service(&disk);
        restored.poll();
        assert_eq!(
            files.read(&disk, "/AirKeyboard.txt").unwrap(),
            "Привет\nмир".as_bytes()
        );
        restored.close();
        fs::reset_for_test();
    }
    #[test]
    fn back_preserves_an_unsaved_draft_until_exit_is_confirmed() {
        fs::set_external_access(true);
        let mut c = Controller::default();
        c.open();
        c.editor.insert('Я');
        assert!(c.back());
        assert_eq!(c.menu_index, 9);
        assert!(!c.event(RuntimeEvent::Press));
        assert_eq!(c.editor.text, "Я");
        assert!(c.event(RuntimeEvent::Press));
        c.close();
        assert!(c.editor.text.is_empty());
    }

    #[test]
    fn hover_is_not_text_or_hid_and_a_hold_commits_once() {
        fs::set_external_access(true);
        let mut c = Controller::default();
        c.open();
        c.hover(0);
        assert_eq!(c.view().preview, Some('й'));
        assert_eq!(c.view().len, 0);
        assert!(c.take().is_none());
        c.confirm(true);
        c.confirm(true);
        assert_eq!(c.editor.text, "й");
        c.confirm(false);
        c.confirm(true);
        assert_eq!(c.editor.text, "йй");
        c.close();
        assert!(c.take().is_none());
        assert_eq!(c.view().len, 0);
    }
    #[test]
    fn hid_requires_explicit_arming_and_disconnect_never_replays_keys() {
        fs::set_external_access(true);
        let mut c = Controller::default();
        c.open();
        c.output = Output::Mirror;
        c.connection(true);
        c.confirm(true);
        c.confirm(false);
        assert!(c.take().is_none());
        assert!(c.editor.text.is_empty());
        c.armed = true;
        c.confirm(true);
        c.confirm(false);
        match c.take().unwrap() {
            BluetoothUiAction::Keyboard(r) => assert_eq!(r[2], 20),
            _ => panic!(),
        }
        assert_eq!(c.editor.text, "й");
        c.confirm(true);
        c.confirm(false);
        c.connection(false);
        assert!(c.take().is_none());
        c.connection(true);
        assert!(!c.armed);
        assert_eq!(
            keys::hid(Key::Character('Ж'), 0).unwrap(),
            [2, 0, 51, 0, 0, 0, 0, 0]
        );
    }
}
