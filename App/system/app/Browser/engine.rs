//! Bounded native flow layout, fed by the on-watch HTML reader. No JavaScript or DOM in the rasterizer.
use alloc::{string::String, vec::Vec};
use serde_json::Value;
const MAX_LINES: usize = 256;
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Link(u16),
    Image(u16),
}
pub struct Line {
    pub text: String,
    pub kind: u8,
    pub indent: u8,
    pub action: Option<Action>,
    gap: i32,
}
impl Line {
    pub fn height(&self) -> i32 {
        self.gap
            + match self.kind {
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
        let Some(blocks) = value["blocks"].as_array() else {
            return out;
        };
        let mut start = 0;
        while start < blocks.len() && out.lines.len() < MAX_LINES {
            let block = &blocks[start];
            let kind = match block["type"].as_str().unwrap_or("") {
                "heading" if block["level"].as_u64() == Some(1) => 1,
                "heading" => 2,
                "image" => 4,
                "code" => 5,
                "rule" => 6,
                _ => 0,
            };
            let indent = block["indent"].as_u64().unwrap_or(0).min(3) as u8;
            let mut end = start + 1;
            // Anchor boundaries are inline: keep the surrounding sentence on
            // the same line. Distinct links still get separate selectable rows.
            if !matches!(kind, 4 | 6) && block["flow"].is_u64() {
                while end < blocks.len()
                    && blocks[end]["flow"] == block["flow"]
                    && !matches!(blocks[end]["type"].as_str(), Some("image" | "rule"))
                {
                    end += 1;
                }
            }
            if matches!(kind, 4 | 6) {
                out.lines.push(Line {
                    text: block["text"]
                        .as_str()
                        .unwrap_or("")
                        .chars()
                        .take(48)
                        .collect(),
                    kind,
                    indent,
                    gap: 6,
                    action: (kind == 4).then_some(Action::Image(index(block))),
                });
            } else {
                let mut flow = Flow {
                    lines: &mut out.lines,
                    text: String::new(),
                    width: 0,
                    action: None,
                    kind,
                    indent,
                    limit: if kind == 1 {
                        105
                    } else {
                        210 - i32::from(indent) * 8
                    },
                };
                if block["list"] == true {
                    flow.segment("-", None);
                }
                for segment in &blocks[start..end] {
                    let action =
                        (segment["type"] == "link").then_some(Action::Link(index(segment)));
                    flow.segment(segment["text"].as_str().unwrap_or(""), action);
                }
                flow.emit();
                if let Some(last) = out.lines.last_mut() {
                    last.gap = if kind == 1 { 8 } else { 6 };
                }
            }
            start = end;
        }
        out
    }
}
fn index(block: &Value) -> u16 {
    block["index"].as_u64().unwrap_or(0).min(65535) as u16
}
struct Flow<'a> {
    lines: &'a mut Vec<Line>,
    text: String,
    width: i32,
    action: Option<Action>,
    kind: u8,
    indent: u8,
    limit: i32,
}
impl Flow<'_> {
    fn emit(&mut self) {
        if !self.text.is_empty() && self.lines.len() < MAX_LINES {
            self.lines.push(Line {
                text: core::mem::take(&mut self.text),
                kind: if self.action.is_some() && self.kind == 0 {
                    3
                } else {
                    self.kind
                },
                indent: self.indent,
                action: self.action,
                gap: 0,
            });
        }
        self.width = 0;
        self.action = None;
    }
    fn segment(&mut self, text: &str, action: Option<Action>) {
        if action.is_some() && self.action.is_some() && action != self.action {
            self.emit();
        }
        for word in text.split_whitespace() {
            if self.lines.len() >= MAX_LINES {
                return;
            }
            let size: i32 = word
                .chars()
                .map(crate::material::body_character_width)
                .sum();
            if !self.text.is_empty() && self.width + 4 + size > self.limit {
                self.emit();
            }
            if !self.text.is_empty() && !word.starts_with(['.', ',', '!', '?', ':', ';', ')', ']'])
            {
                self.text.push(' ');
                self.width += crate::material::body_character_width(' ');
            }
            for c in word.chars() {
                let advance = crate::material::body_character_width(c);
                if !self.text.is_empty()
                    && (self.width + advance > self.limit || self.text.len() + c.len_utf8() > 112)
                {
                    self.emit();
                }
                if self.lines.len() >= MAX_LINES {
                    return;
                }
                self.text.push(c);
                self.width += advance;
                self.action = self.action.or(action);
            }
        }
    }
}
// Emit directly into the layout. Building a temporary Vec for the entire
// paragraph defeated MAX_LINES before the caller could stop consuming it.
#[cfg(test)]
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
    fn inline_links_keep_the_sentence_and_distinct_targets_remain_selectable() {
        let layout = Layout::build(&serde_json::json!({"blocks":[
            {"flow":1,"type":"paragraph","text":"Hello"},
            {"flow":1,"type":"link","index":0,"text":"world"},
            {"flow":1,"type":"paragraph","text":"!"},
            {"flow":1,"type":"link","index":1,"text":"Next"},
            {"flow":2,"type":"link","index":2,"text":"Item","list":true}
        ]}));
        assert_eq!(layout.lines[0].text, "Hello world!");
        assert!(matches!(layout.lines[0].action, Some(Action::Link(0))));
        assert!(matches!(layout.lines[1].action, Some(Action::Link(1))));
        assert_eq!(layout.lines[2].text, "- Item");
        assert!(layout.lines[1].gap > 0);
    }
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
