// Модуль калькулятора выражений.
//! Expression calculator with precedence, parentheses and bounded nesting.
use core::fmt::Write;
use heapless::String;
/// Порядок клавиш калькулятора; индекс используется контроллером ввода.
pub const KEYS: [&str; 24] = [
    "C", "DEL", "(", ")", "7", "8", "9", "/", "4", "5", "6", "*", "1", "2", "3", "-", "0", ".",
    "%", "+", "+/-", "sqrt", "x2", "=",
];
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Снимок данных для отображения компонента калькулятора выражений.
pub struct View {
    /// Текущее математическое выражение.
    pub expression: [u8; 80],
    /// Число значащих байтов в соответствующем текстовом буфере.
    pub len: u8,
    /// Сообщение или его код для вывода пользователю.
    pub message: &'static str,
}
impl Default for View {
    /// Задаёт исходное состояние нового экземпляра.
    fn default() -> Self {
        Self {
            expression: [0; 80],
            len: 0,
            message: "",
        }
    }
}
/// Состояние математического выражения, результата и повторной операции компонента калькулятора выражений.
pub struct Calculator {
    /// Текущее математическое выражение.
    expression: String<96>,
    /// Подготовленный результат или очередь исходящих данных.
    output: String<32>,
    /// Признак завершения текущей операции.
    finished: bool,
    /// Сообщение или его код для вывода пользователю.
    message: &'static str,
    /// Операция и аргумент для повторного нажатия «=».
    repeat: Option<(u8, f64)>,
    /// Последний числовой результат вычисления.
    answer: f64,
}
impl Default for Calculator {
    /// Задаёт исходное состояние нового экземпляра.
    fn default() -> Self {
        Self {
            expression: String::new(),
            output: String::try_from("0").unwrap(),
            finished: false,
            message: "",
            repeat: None,
            answer: 0.0,
        }
    }
}
/// Форматирует ограниченный результат и убирает незначащие нули.
fn format_number(value: f64) -> Result<String<32>, &'static str> {
    if !value.is_finite() || value.abs() > 999_999_999_999.0 {
        return Err("Число вне диапазона");
    }
    let mut s = String::new();
    write!(s, "{:.8}", value).map_err(|_| "Число вне диапазона")?;
    while s.ends_with('0') {
        s.pop();
    }
    if s.ends_with('.') {
        s.pop();
    }
    if s == "-0" {
        s.clear();
        let _ = s.push('0');
    }
    Ok(s)
}
#[derive(Clone, Copy)]
/// Числовой операнд и признак процентного представления компонента калькулятора выражений.
struct Value {
    /// Числовое значение разобранного фрагмента выражения.
    n: f64,
    /// Признак процентного операнда для сложения/вычитания.
    percent: bool,
}
/// Разборщик выражения с ограничением вложенности компонента калькулятора выражений.
struct Parser<'a> {
    /// Разбираемая последовательность символов/байтов.
    s: &'a [u8],
    /// Текущая позиция чтения в последовательности.
    pos: usize,
    /// Глубина вложенности для ограничения разбора.
    depth: u8,
    /// Операция и аргумент для повторного нажатия «=».
    repeat: Option<(u8, f64)>,
}
impl Parser<'_> {
    /// Возвращает следующий символ без смещения позиции разбора.
    fn peek(&self) -> u8 {
        self.s.get(self.pos).copied().unwrap_or(0)
    }
    /// Разбирает +/−; процент справа трактуется относительно левого значения.
    fn add(&mut self) -> Result<Value, &'static str> {
        let mut left = self.multiply()?;
        while matches!(self.peek(), b'+' | b'-') {
            let op = self.peek();
            self.pos += 1;
            let right = self.multiply()?;
            let amount = if right.percent {
                left.n * right.n
            } else {
                right.n
            };
            left.n = if op == b'+' {
                left.n + amount
            } else {
                left.n - amount
            };
            left.percent = false;
            self.repeat = Some((op, amount));
        }
        Ok(left)
    }
    /// Разбирает умножение и деление с проверкой делителя.
    fn multiply(&mut self) -> Result<Value, &'static str> {
        let mut left = self.unary()?;
        while matches!(self.peek(), b'*' | b'/') {
            let op = self.peek();
            self.pos += 1;
            let right = self.unary()?;
            if op == b'/' && right.n == 0.0 {
                return Err("Деление на 0");
            }
            left.n = if op == b'*' {
                left.n * right.n
            } else {
                left.n / right.n
            };
            left.percent = false;
            self.repeat = Some((op, right.n));
        }
        Ok(left)
    }
    /// Разбирает знак числа и постфиксный процент.
    fn unary(&mut self) -> Result<Value, &'static str> {
        let mut negative = false;
        while matches!(self.peek(), b'+' | b'-') {
            negative ^= self.peek() == b'-';
            self.pos += 1;
        }
        let mut v = self.primary()?;
        if negative {
            v.n = -v.n;
        }
        while self.peek() == b'%' {
            self.pos += 1;
            v.n /= 100.0;
            v.percent = true;
        }
        Ok(v)
    }
    /// Разбирает число, скобки или квадратный корень с ограничением вложенности.
    fn primary(&mut self) -> Result<Value, &'static str> {
        let square_root = self.s[self.pos..].starts_with(b"sqrt(");
        if square_root || self.peek() == b'(' {
            if self.depth >= 12 {
                return Err("Слишком много скобок");
            }
            self.pos += if square_root { 5 } else { 1 };
            self.depth += 1;
            let mut v = self.add()?;
            self.depth -= 1;
            if self.peek() != b')' {
                return Err("Закройте скобку");
            }
            self.pos += 1;
            if square_root {
                if v.n < 0.0 {
                    return Err("Корень из отрицательного");
                }
                if v.n != 0.0 {
                    let mut root = v.n.max(1.0);
                    for _ in 0..64 {
                        let next = (root + v.n / root) / 2.0;
                        if next == root {
                            break;
                        }
                        root = next;
                    }
                    v.n = root;
                }
                v.percent = false;
            }
            return Ok(v);
        }
        let start = self.pos;
        while self.peek().is_ascii_digit() || self.peek() == b'.' {
            self.pos += 1;
        }
        if start == self.pos {
            return Err("Проверьте выражение");
        }
        let text =
            core::str::from_utf8(&self.s[start..self.pos]).map_err(|_| "Проверьте выражение")?;
        let n = text.parse::<f64>().map_err(|_| "Проверьте число")?;
        Ok(Value { n, percent: false })
    }
}
/// Разбирает всё выражение и проверяет результат вычисления.
fn evaluate(s: &str) -> Result<(f64, Option<(u8, f64)>), &'static str> {
    let mut p = Parser {
        s: s.as_bytes(),
        pos: 0,
        depth: 0,
        repeat: None,
    };
    let value = p.add()?.n;
    if p.pos != s.len() {
        return Err("Проверьте выражение");
    }
    format_number(value)?;
    Ok((value, p.repeat))
}
impl Calculator {
    /// Возвращает подготовленный результат калькулятора без повторного вычисления.
    pub fn text(&self) -> &str {
        &self.output
    }
    /// Собирает снимок состояния для интерфейса.
    pub fn view(&self) -> View {
        let mut v = View {
            message: self.message,
            ..View::default()
        };
        let mut start = self.expression.len().saturating_sub(v.expression.len());
        while !self.expression.is_char_boundary(start) {
            start += 1;
        }
        let bytes = &self.expression.as_bytes()[start..];
        v.expression[..bytes.len()].copy_from_slice(bytes);
        v.len = bytes.len() as u8;
        v
    }
    /// Выполняет действие выбранной клавиши и обновляет состояние ввода.
    pub fn press(&mut self, key: u8) {
        let Some(label) = KEYS.get(key as usize) else {
            return;
        };
        if *label == "C" {
            *self = Self::default();
            return;
        }
        self.message = "";
        if *label == "=" {
            let result = if self.finished {
                if let Some((op, right)) = self.repeat {
                    let value = match op {
                        b'+' => self.answer + right,
                        b'-' => self.answer - right,
                        b'*' => self.answer * right,
                        b'/' if right != 0.0 => self.answer / right,
                        _ => f64::NAN,
                    };
                    Ok((value, self.repeat))
                } else {
                    Ok((self.answer, None))
                }
            } else if self.expression.is_empty() {
                Ok((0.0, None))
            } else {
                evaluate(&self.expression)
            };
            match result.and_then(|(v, r)| format_number(v).map(|s| (v, r, s))) {
                Ok((value, repeat, output)) => {
                    self.answer = value;
                    self.repeat = repeat;
                    self.output = output;
                    self.finished = true;
                }
                Err(e) => {
                    self.message = e;
                    self.finished = false;
                }
            }
            return;
        }
        if self.finished {
            let continued = matches!(
                *label,
                "+" | "-" | "*" | "/" | "%" | "+/-" | "sqrt" | "x2" | "DEL"
            );
            self.expression.clear();
            if continued {
                let _ = self.expression.push_str(&self.output);
            }
            self.finished = false;
            self.repeat = None;
        }
        match *label {
            "DEL" => {
                self.expression.pop();
                if self.expression.is_empty() { self.output.clear(); let _ = self.output.push('0'); }
            }
            "+/-" | "sqrt" | "x2" => {
                let current = if self.expression.is_empty() {
                    "0"
                } else {
                    &self.expression
                };
                let mut next = String::<96>::new();
                let ok = match *label {
                    "+/-" => write!(next, "-({})", current),
                    "sqrt" => write!(next, "sqrt({})", current),
                    _ => write!(next, "({})*({})", current, current),
                };
                if ok.is_ok() {
                    self.expression = next;
                } else {
                    self.message = "Лимит выражения";
                }
            }
            _ => {
                if self.expression.push_str(label).is_err() {
                    self.message = "Лимит выражения";
                }
            }
        }
        if let Ok((value, _)) = evaluate(&self.expression) {
            if let Ok(s) = format_number(value) {
                self.output = s;
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    /// Вычисляет тестовое выражение и возвращает форматированный результат.
    fn compute(text: &str) -> String<32> {
        format_number(evaluate(text).unwrap().0).unwrap()
    }
    #[test]
    /// Проверяет приоритет операций, скобки, унарный знак, проценты и ошибки выражений.
    fn precedence_parentheses_unary_and_percent() {
        assert_eq!(compute("2+3*4"), "14");
        assert_eq!(compute("(2+3)*4"), "20");
        assert_eq!(compute("200+10%"), "220");
        assert_eq!(compute("200*10%"), "20");
        assert_eq!(compute("sqrt(81)+-2.5"), "6.5");
        assert_eq!(evaluate("1/0").unwrap_err(), "Деление на 0");
        assert!(evaluate("(2+3").is_err());
        assert!(evaluate("2..3").is_err());
    }
    #[test]
    /// Проверяет повторное «=», очистку результата и удаление последнего символа.
    fn repeated_equals_and_recovery() {
        let mut c = Calculator::default();
        for key in [14, 19, 13, 23, 23] {
            c.press(key);
        } // 3 + 2 = =
        assert_eq!(c.text(), "7");
        c.press(0);
        assert_eq!(c.text(), "0");
        c.press(13); c.press(1); assert_eq!(c.text(), "0");
    }
}
