//! Bounded four-operation calculator. Math runs on a crown press, never per frame.
use core::fmt::Write;
use heapless::String;
pub const KEYS: [&str; 20] = [
    "C", "DEL", "+/-", "/", "7", "8", "9", "*", "4", "5", "6", "-", "1", "2", "3", "+", "0", ".",
    "%", "=",
];
pub struct Calculator {
    input: String<32>,
    value: f64,
    op: Option<u8>,
    fresh: bool,
    error: bool,
}
impl Default for Calculator {
    fn default() -> Self {
        let mut input = String::new();
        let _ = input.push('0');
        Self {
            input,
            value: 0.0,
            op: None,
            fresh: true,
            error: false,
        }
    }
}
impl Calculator {
    pub fn text(&self) -> &str {
        &self.input
    }
    fn result(&mut self, value: f64) {
        self.input.clear();
        self.error = !value.is_finite() || value.abs() > 999_999_999_999.0;
        if self.error {
            let _ = self.input.push_str("Ошибка");
            self.op = None;
        } else {
            let _ = write!(self.input, "{:.4}", value);
            while self.input.ends_with('0') {
                self.input.pop();
            }
            if self.input.ends_with('.') {
                self.input.pop();
            }
            if self.input == "-0" {
                self.input.clear();
                let _ = self.input.push('0');
            }
        }
        self.fresh = true;
    }
    fn calculate(&mut self, right: f64) -> f64 {
        match self.op.take() {
            Some(b'+') => self.value + right,
            Some(b'-') => self.value - right,
            Some(b'*') => self.value * right,
            Some(b'/') => {
                if right == 0.0 {
                    f64::NAN
                } else {
                    self.value / right
                }
            }
            _ => right,
        }
    }
    pub fn press(&mut self, key: u8) {
        let Some(label) = KEYS.get(usize::from(key)) else {
            return;
        };
        if key == 0 {
            *self = Self::default();
            return;
        }
        if self.error {
            *self = Self::default();
        }
        match key {
            1 => {
                self.input.pop();
                if self.input.is_empty() || self.input == "-" {
                    self.input.clear();
                    let _ = self.input.push('0');
                }
                self.fresh = false;
            }
            2 => {
                let v = -self.input.parse::<f64>().unwrap_or(0.0);
                self.result(v);
                self.fresh = false;
            }
            3 | 7 | 11 | 15 => {
                let right = self.input.parse().unwrap_or(0.0);
                if !self.fresh || self.op.is_none() {
                    let v = self.calculate(right);
                    self.result(v);
                    self.value = v;
                }
                if !self.error {
                    self.op = Some(label.as_bytes()[0]);
                    self.fresh = true;
                }
            }
            18 => {
                let v = self.input.parse::<f64>().unwrap_or(0.0) / 100.0;
                self.result(v);
                self.fresh = false;
            }
            19 => {
                let v = self.calculate(self.input.parse().unwrap_or(0.0));
                self.result(v);
                self.value = v;
            }
            _ => {
                if self.fresh {
                    self.input.clear();
                    self.fresh = false;
                }
                if *label == "." {
                    if self.input.contains('.') {
                        return;
                    }
                    if self.input.is_empty() {
                        let _ = self.input.push('0');
                    }
                } else if self.input == "0" {
                    self.input.clear();
                }
                if self.input.len() < 14 {
                    let _ = self.input.push_str(label);
                }
            }
        }
    }
}
