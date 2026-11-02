//! Shared crown keyboard. Editing policy (save/connect/cancel) stays with the caller.
const EN: &str = "qwertyuiopasdfghjklzxcvbnm";
const RU: &str = "йцукенгшщзхъфывапролджэячсмитьбюё";
const SYMBOLS: [&str; 2] = ["1234567890!@#$%^&*()_+-=[]", "{}.,:;?'\"/\\|<>`~"];
pub const SHIFT: usize = 0;
pub const MODE: usize = 1;
pub const LANGUAGE: usize = 2;
pub const DELETE: usize = 3;
pub const CANCEL: usize = 4;
pub const SPACE: usize = 5;
pub const SUBMIT: usize = 6;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct KeyboardView {
    pub english: bool,
    pub upper: bool,
    pub page: u8, // 0 letters, 1 digits/symbols, 2 additional symbols
    pub focus: u8,
}
impl KeyboardView {
    pub fn characters(self) -> &'static str {
        match self.page {
            1 => SYMBOLS[0],
            2 => SYMBOLS[1],
            _ if self.english => EN,
            _ => RU,
        }
    }
    pub fn character_count(self) -> usize {
        self.characters().chars().count()
    }
    pub fn rows(self) -> [usize; 3] {
        match self.page {
            1 => [10, 10, 6],
            2 => [9, 7, 0],
            _ if self.english => [10, 9, 7],
            _ => [12, 11, 10],
        }
    }
    pub fn character(self, index: usize) -> Option<char> {
        let ch = self.characters().chars().nth(index)?;
        Some(if self.upper && self.page == 0 {
            ch.to_uppercase().next().unwrap_or(ch)
        } else {
            ch
        })
    }
    pub fn selected_action(self, action: usize) -> bool {
        usize::from(self.focus) == self.character_count() + action
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    Character(char),
    Backspace,
    Submit,
    Cancel,
    None,
}
#[derive(Clone, Debug, Default)]
pub struct Keyboard {
    pub view: KeyboardView,
}
impl Keyboard {
    pub fn english() -> Self {
        Self {
            view: KeyboardView {
                english: true,
                ..Default::default()
            },
        }
    }
    pub fn rotate(&mut self, delta: i32) {
        self.view.focus = (i32::from(self.view.focus) + delta)
            .rem_euclid(self.view.character_count() as i32 + 7) as u8;
    }
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
