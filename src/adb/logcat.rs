//! Streaming logcat options and formatting for the legacy ADB shell transport.
use crate::logging;
use core::fmt::Write;
use heapless::{String, Vec};
#[derive(Clone, Copy)]
pub enum Style {
    Threadtime,
    Brief,
    Raw,
}
#[derive(Clone)]
pub struct Filter {
    tag: String<24>,
    level: u8,
}
#[derive(Clone)]
pub struct Session {
    pub cursor: u64,
    pub end: Option<u64>,
    pub clear: bool,
    style: Style,
    filters: Vec<Filter, 8>,
    minimum: u8,
}
fn level(raw: &str) -> Option<u8> {
    match raw {
        "V" => Some(0),
        "D" => Some(1),
        "I" => Some(2),
        "W" => Some(3),
        "E" => Some(4),
        "F" => Some(5),
        "S" => Some(6),
        _ => None,
    }
}
/// Recognize the wrapper emitted by the real adb client, without a shell eval.
pub fn command(raw: &str) -> Option<&str> {
    let mut cmd = raw.trim();
    if let Some(rest) = cmd.strip_prefix("export ANDROID_LOG_TAGS=") {
        let (value, rest) = rest.split_once(';')?;
        if value.len() > 128 {
            return None;
        }
        cmd = rest.trim();
    }
    cmd = cmd.strip_prefix("exec ").unwrap_or(cmd).trim();
    (cmd == "logcat" || cmd.starts_with("logcat ")).then_some(cmd)
}
impl Session {
    pub fn parse(raw: &str) -> Result<Self, &'static str> {
        let cmd = command(raw).ok_or("not a logcat command")?;
        let (first, next) = logging::bounds();
        let mut this = Self {
            cursor: first,
            end: None,
            clear: false,
            style: Style::Threadtime,
            filters: Vec::new(),
            minimum: 2,
        };
        let mut args = cmd.split_ascii_whitespace().skip(1);
        while let Some(arg) = args.next() {
            match arg {
                "-d" => this.end = Some(next),
                "-c" => {
                    this.clear = true;
                    this.end = Some(next);
                }
                "-s" => this.minimum = 6,
                "-v" => {
                    this.style = match args.next() {
                        Some("threadtime" | "time") => Style::Threadtime,
                        Some("brief") => Style::Brief,
                        Some("raw") => Style::Raw,
                        _ => return Err("supported formats: threadtime, time, brief, raw"),
                    }
                }
                "-t" | "-T" => {
                    let count = args
                        .next()
                        .and_then(|s| s.parse::<u64>().ok())
                        .ok_or("expected a line count")?;
                    this.cursor = next.saturating_sub(count).max(first);
                    if arg == "-t" {
                        this.end = Some(next);
                    }
                }
                _ => {
                    let spec = arg.trim_matches(|c| c == '\'' || c == '"');
                    let (tag, priority) = spec.split_once(':').ok_or("use Tag:I or '*:W'")?;
                    let priority = level(priority).ok_or("invalid log priority")?;
                    if tag == "*" {
                        this.minimum = priority;
                    } else {
                        this.filters
                            .push(Filter {
                                tag: String::try_from(tag).map_err(|_| "tag too long")?,
                                level: priority,
                            })
                            .map_err(|_| "at most 8 tag filters")?;
                    }
                }
            }
        }
        if this.clear {
            logging::clear();
        }
        Ok(this)
    }
    pub fn finished(&self) -> bool {
        self.end.map(|end| self.cursor >= end).unwrap_or(false) || self.clear
    }
    pub fn next_line(&mut self) -> Option<String<256>> {
        // Bound work even when an entire ring consists of filtered records.
        for _ in 0..32 {
            if self.finished() {
                return None;
            }
            let record = logging::next(&mut self.cursor)?;
            let minimum = self
                .filters
                .iter()
                .find(|f| f.tag == record.tag)
                .map(|f| f.level)
                .unwrap_or(self.minimum);
            let rank = level(core::str::from_utf8(&[record.priority]).ok()?).unwrap_or(2);
            if minimum == 6 || rank < minimum {
                continue;
            }
            let mut line = String::new();
            match self.style {
                Style::Raw => {
                    let _ = write!(line, "{}\r\n", record.message);
                }
                Style::Brief => {
                    let _ = write!(
                        line,
                        "{}/{}(0): {}\r\n",
                        record.priority as char, record.tag, record.message
                    );
                }
                Style::Threadtime => {
                    let _ = write!(
                        line,
                        "{:02}-{:02} {:02}:{:02}:{:02}.{:03}     0     0 {} {:<20}: {}\r\n",
                        record.month,
                        record.day,
                        record.minutes / 60,
                        record.minutes % 60,
                        record.seconds,
                        record.uptime_ms % 1000,
                        record.priority as char,
                        record.tag,
                        record.message
                    );
                }
            }
            return Some(line);
        }
        None
    }
}
