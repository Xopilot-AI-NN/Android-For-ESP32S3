// Модуль общей клавиатуры.
//! Shared crown keyboard. Editing policy (save/connect/cancel) stays with the caller.
#[path = "text_input/suggestions.rs"]
mod suggestions;
pub use suggestions::complete;
/// Параметр EN компонента общей клавиатуры.
const EN: &str = "qwertyuiopasdfghjklzxcvbnm";
/// Параметр RU компонента общей клавиатуры.
const RU: &str = "йцукенгшщзхъфывапролджэячсмитьбюё";
/// Параметр SYMBOLS компонента общей клавиатуры.
const SYMBOLS: [&str; 2] = ["1234567890!@#$%^&*()_+-=[]", "{}.,:;?'\"/\\|<>`~"];
/// Индекс служебной клавиши регистра относительно конца раскладки.
pub const SHIFT: usize = 0;
/// Индекс переключения букв и символов.
pub const MODE: usize = 1;
/// Индекс переключения языка ввода.
pub const LANGUAGE: usize = 2;
/// Индекс удаления предыдущего символа.
pub const DELETE: usize = 3;
/// Индекс отмены ввода.
pub const CANCEL: usize = 4;
/// Индекс вставки пробела.
pub const SPACE: usize = 5;
/// Индекс подтверждения ввода.
pub const SUBMIT: usize = 6;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
/// Копируемый снимок раскладки, фокуса и подсказок компонента общей клавиатуры.
pub struct KeyboardView {
    /// Выбор английской или русской раскладки.
    pub english: bool,
    /// Признак верхнего регистра букв.
    pub upper: bool,
    /// Номер страницы букв или символов.
    pub page: u8, // 0 letters, 1 digits/symbols, 2 additional symbols
    /// Индекс выбранной клавиши.
    pub focus: u8,
    /// До шести доступных словарных подсказок.
    pub suggestions: [&'static str; 6],
    /// Разрешение клавиши NL только в редакторе документов.
    pub allow_newline: bool,
}
impl KeyboardView {
    /// Возвращает символы активной раскладки или страницы знаков.
    pub fn characters(self) -> &'static str {
        match self.page {
            1 => SYMBOLS[0],
            2 if self.allow_newline => "{}.,:;?'\"/\\|<>`~\n",
            2 => SYMBOLS[1],
            _ if self.english => EN,
            _ => RU,
        }
    }
    /// Подсчитывает символы активной раскладки.
    pub fn character_count(self) -> usize {
        self.characters().chars().count()
    }
    /// Определяет длины трёх рядов клавиатуры.
    pub fn rows(self) -> [usize; 3] {
        match self.page {
            1 => [10, 10, 6],
            2 if self.allow_newline => [9, 8, 0],
            2 => [9, 7, 0],
            _ if self.english => [10, 9, 7],
            _ => [12, 11, 10],
        }
    }
    /// Возвращает символ по индексу с учётом регистра.
    pub fn character(self, index: usize) -> Option<char> {
        let ch = self.characters().chars().nth(index)?;
        Some(if self.upper && self.page == 0 {
            ch.to_uppercase().next().unwrap_or(ch)
        } else {
            ch
        })
    }
    /// Проверяет, стоит ли фокус на служебной клавише.
    pub fn selected_action(self, action: usize) -> bool {
        usize::from(self.focus) == self.character_count() + action
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Результат активации клавиши компонента общей клавиатуры.
pub enum Action {
    Character(char),
    Backspace,
    Suggestion(&'static str),
    Submit,
    Cancel,
    None,
}
#[derive(Clone, Debug, Default)]
/// Контроллер общей клавиатуры компонента общей клавиатуры.
pub struct Keyboard {
    /// Подготовленный снимок данных для интерфейса.
    pub view: KeyboardView,
}
impl Keyboard {
    /// Создаёт клавиатуру с английской раскладкой.
    pub fn english() -> Self {
        Self {
            view: KeyboardView {
                english: true,
                ..Default::default()
            },
        }
    }
    /// Обновляет подсказки и оставляет фокус на видимой клавише.
    pub fn suggest(&mut self, text: &str) {
        self.view.suggestions = suggestions::candidates(text, self.view.english);
        // A completion can remove all candidates while the crown is on one.
        // Return focus to the space key instead of leaving an invisible target.
        if usize::from(self.view.focus) >= self.target_count() as usize {
            self.view.focus = (self.view.character_count() + SPACE) as u8;
        }
    }
    /// Считает символы, служебные клавиши и доступные подсказки.
    fn target_count(&self) -> i32 {
        self.view.character_count() as i32
            + 7
            + self
                .view
                .suggestions
                .iter()
                .filter(|s| !s.is_empty())
                .count() as i32
    }
    /// Перемещает фокус на число шагов колёсика с учётом границ.
    pub fn rotate(&mut self, delta: i32) {
        let count = self.target_count();
        self.view.focus =
            (i32::from(self.view.focus) + delta.rem_euclid(count)).rem_euclid(count) as u8;
    }
    /// Выполняет действие выбранной клавиши и обновляет состояние ввода.
    pub fn press(&mut self) -> Action {
        let count = self.view.character_count();
        let index = usize::from(self.view.focus);
        if index < count {
            return self
                .view
                .character(index)
                .map(Action::Character)
                .unwrap_or(Action::None);
        }
        let action = index - count;
        if action >= 7 {
            return self
                .view
                .suggestions
                .get(action - 7)
                .filter(|s| !s.is_empty())
                .map(|s| Action::Suggestion(s))
                .unwrap_or(Action::None);
        }
        match action {
            SHIFT => {
                if self.view.page == 0 {
                    self.view.upper = !self.view.upper;
                } else {
                    self.view.page = if self.view.page == 1 { 2 } else { 1 };
                }
            }
            MODE => self.view.page = if self.view.page == 0 { 1 } else { 0 },
            LANGUAGE => {
                self.view.english = !self.view.english;
                self.view.page = 0;
            }
            SPACE => return Action::Character(' '),
            DELETE => return Action::Backspace,
            SUBMIT => return Action::Submit,
            CANCEL => return Action::Cancel,
            _ => {}
        }
        // Keep the same service key focused when the alphabet changes size.
        self.view.focus = (self.view.character_count() + action.min(6)) as u8;
        Action::None
    }
}
