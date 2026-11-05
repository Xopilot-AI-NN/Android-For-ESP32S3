//! Bounded native flow layout, fed by the on-watch HTML reader. No JavaScript or DOM in the rasterizer.
use alloc::{string::String, vec::Vec};
use serde_json::Value;
const MAX_LINES: usize = 256;
#[derive(Clone, Copy)]
pub enum Action {
    Link(u16),
    Image(u16),
}
pub struct Line {
    pub text: String,
    pub kind: u8,
    pub indent: u8,
    pub action: Option<Action>,
}
impl Line {
    pub fn height(&self) -> i32 {
        match self.kind {
            1 => 31,
            2 => 23,
            4 => 42,
            6 => 12,
            _ => 18,
        }
    }
}
#[derive(Default)]
pub struct Layout {
    pub lines: Vec<Line>,
}
impl Layout {
    pub fn build(value: &Value) -> Self {
        let mut out = Self::default();
        if let Some(blocks) = value["blocks"].as_array() {
            for block in blocks {
                let index = block["index"].as_u64().unwrap_or(0).min(65535) as u16;
                let (kind, action) = match block["type"].as_str().unwrap_or("") {
                    "heading" if block["level"].as_u64() == Some(1) => (1, None),
                    "heading" => (2, None),
                    "link" => (3, Some(Action::Link(index))),
                    "image" => (4, Some(Action::Image(index))),
                    "code" => (5, None),
                    "rule" => (6, None),
                    _ if block["bold"] == true => (2, None),
                    _ => (0, None),
                };
                let indent = block["indent"].as_u64().unwrap_or(0).min(3) as u8;
                let text = block["text"].as_str().unwrap_or("");
                if matches!(kind, 4 | 6) {
                    out.lines.push(Line {
                        text: text.chars().take(48).collect(),
                        kind,
                        indent,
                        action,
                    });
                } else {
                    let width = if kind == 1 {
                        105
                    } else {
                        210 - i32::from(indent) * 8
                    };
                    wrap(text, width, |text| {
                        out.lines.push(Line {
                            text,
                            kind,
                            indent,
                            action,
                        });
                        out.lines.len() < MAX_LINES
                    });
                }
                if out.lines.len() >= MAX_LINES {
                    break;
                }
            }
        }
        out
    }
}
// Emit directly into the layout. Building a temporary Vec for the entire
// paragraph defeated MAX_LINES before the caller could stop consuming it.
fn wrap(text: &str, limit: i32, mut emit: impl FnMut(String) -> bool) {
    let mut line = String::new();
    let mut width = 0;
    for word in text.split_whitespace() {
        let mut size = 0;
        for c in word.chars() {
            size += crate::material::body_character_width(c);
            if size > limit {
                break; // Only need to know whether this word fits.
            }
        }
        if !line.is_empty() && width + 4 + size > limit {
            if !emit(core::mem::take(&mut line)) {
                return;
            }
            width = 0;
        }
        if !line.is_empty() {
            line.push(' ');
            width += crate::material::body_character_width(' ');
        }
        for c in word.chars() {
            let advance = crate::material::body_character_width(c);
            if !line.is_empty() && (width + advance > limit || line.len() + c.len_utf8() > 112) {
                if !emit(core::mem::take(&mut line)) {
                    return;
                }
                width = 0;
            }
            line.push(c);
            width += advance;
        }
    }
    if !line.is_empty() {
        emit(line);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn long_paragraphs_stop_at_the_budget_without_losing_link_actions() {
        let layout = Layout::build(&serde_json::json!({"blocks":[
            {"index":7,"type":"link","text":"Очень длинная ссылка ".repeat(4000)},
            {"index":8,"type":"paragraph","text":"Must not be laid out"}
        ]}));
        assert_eq!(layout.lines.len(), MAX_LINES);
        assert!(
            layout
                .lines
                .iter()
                .all(|line| matches!(line.action, Some(Action::Link(7))))
        );
        let mut first = Vec::new();
        wrap("один два три четыре", 30, |line| {
            first.push(line);
            false
        });
        assert_eq!(first.len(), 1);
        assert!(
            first[0]
                .chars()
                .map(crate::material::body_character_width)
                .sum::<i32>()
                <= 30
        );
    }

    #[test]
    fn layout_keeps_unicode_links_headings_and_a_hard_line_budget() {
        let layout = Layout::build(&serde_json::json!({"blocks":[
            {"index":0,"type":"heading","level":1,"text":"Большой заголовок"},
            {"index":1,"type":"link","text":"Открыть пример","target":"https://example.org"},
            {"index":2,"type":"paragraph","text":"😀Привет ".repeat(900)},
            {"index":3,"type":"image","text":"Фото"}]}));
        assert!(layout.lines.len() <= MAX_LINES);
        assert_eq!(layout.lines[0].kind, 1);
        assert!(
            layout
                .lines
                .iter()
                .any(|line| matches!(line.action, Some(Action::Link(1))))
        );
        for line in &layout.lines {
            assert!(line.text.len() <= 112);
        }
    }
}
