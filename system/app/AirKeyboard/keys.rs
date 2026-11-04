//! Logical keys and physical HID usages are separate from editable Unicode text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Key {
    Character(char),
    Shift,
    Language,
    Symbols,
    Space,
    Backspace,
    Enter,
    Left,
    Right,
    Up,
    Down,
    Tab,
    Escape,
}
pub fn row_text(russian: bool, symbols: bool, row: usize) -> &'static str {
    if symbols {
        ["1234567890", "!@#$%^&*()", "-_=+.,:;?/"][row]
    } else if russian {
        ["йцукенгшщзхъ", "фывапролджэ", "ячсмитьбю"][row]
    } else {
        ["qwertyuiop", "asdfghjkl", "zxcvbnm"][row]
    }
}
pub fn row_len(russian: bool, symbols: bool, row: usize) -> usize {
    if row < 3 {
        row_text(russian, symbols, row).chars().count()
    } else {
        6
    }
}
pub fn count(russian: bool, symbols: bool) -> usize {
    (0..5).map(|r| row_len(russian, symbols, r)).sum()
}
pub fn key(russian: bool, symbols: bool, mut index: usize, shift: bool) -> Key {
    for row in 0..5 {
        let n = row_len(russian, symbols, row);
        if index < n {
            return match row {
                0..=2 => {
                    let c = row_text(russian, symbols, row).chars().nth(index).unwrap();
                    Key::Character(if shift {
                        c.to_uppercase().next().unwrap_or(c)
                    } else {
                        c
                    })
                }
                3 => [
                    Key::Shift,
                    Key::Language,
                    Key::Symbols,
                    Key::Space,
                    Key::Backspace,
                    Key::Enter,
                ][index],
                _ => [
                    Key::Left,
                    Key::Right,
                    Key::Up,
                    Key::Down,
                    Key::Tab,
                    Key::Escape,
                ][index],
            };
        }
        index -= n;
    }
    Key::Escape
}
pub fn character(key: Key) -> Option<char> {
    match key {
        Key::Character(c) => Some(c),
        Key::Space => Some(' '),
        Key::Enter => Some('\n'),
        Key::Tab => Some('\t'),
        _ => None,
    }
}
pub fn hid(key: Key, modifiers: u8) -> Option<[u8; 8]> {
    let usage = match key {
        Key::Backspace => Some(42),
        Key::Enter => Some(40),
        Key::Tab => Some(43),
        Key::Escape => Some(41),
        Key::Left => Some(80),
        Key::Right => Some(79),
        Key::Up => Some(82),
        Key::Down => Some(81),
        _ => None,
    };
    if let Some(usage) = usage {
        return Some([modifiers, 0, usage, 0, 0, 0, 0, 0]);
    }
    let c = character(key)?;
    let lower = c.to_lowercase().next()?;
    // Standard ЙЦУКЕН physical positions. The receiving OS must use RU;
    // HID does not transmit Unicode or switch the host layout automatically.
    let mut report = if let Some(index) = "йцукенгшщзхъфывапролджэячсмитьбю"
        .chars()
        .position(|r| r == lower)
    {
        let physical = "qwertyuiop[]asdfghjkl;'zxcvbnm,.".as_bytes()[index] as char;
        let usage = match physical {
            '[' => 47,
            ']' => 48,
            ';' => 51,
            '\'' => 52,
            _ => crate::apps::keyboard_app::report(physical)?[2],
        };
        [
            if c.is_uppercase() { 2 } else { 0 },
            0,
            usage,
            0,
            0,
            0,
            0,
            0,
        ]
    } else {
        if let Some(report) = crate::apps::keyboard_app::report(c) {
            report
        } else {
            let (usage, shift) = match c {
                '@' => (31, true),
                '#' => (32, true),
                '$' => (33, true),
                '%' => (34, true),
                '^' => (35, true),
                '&' => (36, true),
                '*' => (37, true),
                '(' => (38, true),
                ')' => (39, true),
                '=' => (46, false),
                '+' => (46, true),
                ';' => (51, false),
                '\'' => (52, false),
                '[' => (47, false),
                ']' => (48, false),
                _ => return None,
            };
            [if shift { 2 } else { 0 }, 0, usage, 0, 0, 0, 0, 0]
        }
    };
    report[0] |= modifiers;
    Some(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_visible_character_has_a_physical_report() {
        for russian in [false, true] {
            for symbols in [false, true] {
                for i in 0..count(russian, symbols) {
                    let k = key(russian, symbols, i, false);
                    if character(k).is_some() {
                        assert!(hid(k, 0).is_some(), "{k:?}");
                    }
                }
            }
        }
        assert_eq!(hid(Key::Left, 1).unwrap(), [1, 0, 80, 0, 0, 0, 0, 0]);
        assert_eq!(hid(Key::Character('Ъ'), 0).unwrap()[2], 48);
    }
}
