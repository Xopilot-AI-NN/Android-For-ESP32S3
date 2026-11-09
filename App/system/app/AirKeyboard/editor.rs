//! Confirmed text only. Hover never enters the document or exported bytes.
use alloc::string::String;
pub const LIMIT: usize = 1024;
#[derive(Default)]
pub struct Editor {
    pub text: String,
    pub caret: usize, // UTF-8 byte boundary; arrows move Unicode scalar values.
}
impl Editor {
    pub fn insert(&mut self, c: char) -> bool {
        if self.text.len() + c.len_utf8() > LIMIT || self.text.try_reserve(c.len_utf8()).is_err() {
            return false;
        }
        self.text.insert(self.caret, c);
        self.caret += c.len_utf8();
        true
    }
    pub fn left(&mut self) {
        self.caret = self.text[..self.caret]
            .char_indices()
            .next_back()
            .map_or(0, |(i, _)| i);
    }
    pub fn right(&mut self) {
        if let Some(c) = self.text[self.caret..].chars().next() {
            self.caret += c.len_utf8();
        }
    }
    pub fn backspace(&mut self) {
        let end = self.caret;
        self.left();
        self.text.replace_range(self.caret..end, "");
    }
    pub fn vertical(&mut self, down: bool) {
        let start = self.text[..self.caret].rfind('\n').map_or(0, |i| i + 1);
        let column = self.text[start..self.caret].chars().count();
        let target = if down {
            let Some(end) = self.text[self.caret..].find('\n') else {
                return;
            };
            self.caret + end + 1
        } else {
            if start == 0 {
                return;
            }
            self.text[..start - 1].rfind('\n').map_or(0, |i| i + 1)
        };
        let end = self.text[target..]
            .find('\n')
            .map_or(self.text.len(), |i| target + i);
        self.caret = self.text[target..end]
            .char_indices()
            .nth(column)
            .map_or(end, |(i, _)| target + i);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unicode_insertion_navigation_and_capacity_are_atomic() {
        let mut e = Editor::default();
        for c in "А😀Б\nвг".chars() {
            assert!(e.insert(c));
        }
        e.caret = "А😀".len();
        assert!(e.insert('я'));
        assert_eq!(e.text, "А😀яБ\nвг");
        e.backspace();
        e.left();
        assert_eq!(e.caret, "А".len());
        e.vertical(true);
        assert_eq!(e.caret, "А😀Б\nв".len());
        e.vertical(false);
        assert_eq!(e.caret, "А".len());
        e.text = "a".repeat(LIMIT);
        e.caret = 0;
        assert!(!e.insert('Ж'));
        assert_eq!(e.caret, 0);
        assert_eq!(e.text.len(), LIMIT);
    }
}
