// Модуль календаря, конвертера и счётчика.
//! Small offline tools; crown input is the only source of edits.
#[cfg(test)]
use crate::runtime::RuntimeEvent;

/// Параметр MONTHS компонента календаря, конвертера и счётчика.
pub const MONTHS: [&str; 12] = [
    "Январь",
    "Февраль",
    "Март",
    "Апрель",
    "Май",
    "Июнь",
    "Июль",
    "Август",
    "Сентябрь",
    "Октябрь",
    "Ноябрь",
    "Декабрь",
];
/// Параметр CONVERSIONS компонента календаря, конвертера и счётчика.
pub const CONVERSIONS: [(&str, &str); 6] = [
    ("км", "мили"),
    ("°C", "°F"),
    ("кг", "фунты"),
    ("см", "дюймы"),
    ("л", "галл. США"),
    ("км/ч", "м/с"),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Состояния календаря, конвертера и ручного счётчика компонента календаря, конвертера и счётчика.
pub struct Tools {
    /// Год и месяц, объединённые в линейный индекс месяцев.
    pub month: i32,
    /// Подтверждённая календарная дата: год, месяц и день.
    pub today: Option<(u16, u8, u8)>,
    /// Количество элементов или выполненных действий в текущем контексте.
    pub count: u32,
    /// Индекс выбранной операции ручного счётчика.
    pub counter_action: u8,
    /// Первое подтверждение сброса уже получено; требуется повторное нажатие.
    pub reset_armed: bool,
    /// Индекс выбранной пары единиц.
    pub conversion: u8,
    /// Признак обратного направления преобразования.
    pub reverse: bool,
    /// Входное число или сумма в единицах конкретной модели.
    pub amount: i32,
    /// Индекс выбранной строки конвертера.
    pub converter_action: u8,
    /// Колёсико меняет число вместо перехода между строками.
    pub editing_amount: bool,
}
impl Default for Tools {
    /// Задаёт исходное состояние нового экземпляра.
    fn default() -> Self {
        Self {
            month: 2000 * 12,
            today: None,
            count: 0,
            counter_action: 0,
            reset_armed: false,
            conversion: 0,
            reverse: false,
            amount: 1,
            converter_action: 0,
            editing_amount: false,
        }
    }
}

/// Определяет длину месяца с учётом високосных лет.
pub fn days_in_month(year: i32, month: u8) -> u8 {
    match month {
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}
/// Monday = 0, Sunday = 6. Gregorian calendar.
pub fn first_weekday(year: i32, month: u8) -> u8 {
    let offsets = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let y = year - i32::from(month < 3);
    ((y + y / 4 - y / 100 + y / 400 + offsets[usize::from(month - 1)] + 7) % 7) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    /// Проверяет длины месяцев, високосные годы и положение первого дня недели.
    fn gregorian_month_lengths_and_weekdays() {
        assert_eq!(days_in_month(1900, 2), 28);
        assert_eq!(days_in_month(2000, 2), 29);
        assert_eq!(days_in_month(2024, 2), 29);
        assert_eq!(first_weekday(2026, 11), 6);
        assert_eq!(first_weekday(2000, 1), 5);
    }
    #[test]
    /// Проверяет отрицательную температуру, обратное преобразование и округление единиц.
    fn unit_results_include_negative_temperature_and_reverse_conversion() {
        let mut t = Tools::default();
        assert_eq!(t.result().as_str(), "0.62");
        t.conversion = 1;
        t.amount = -40;
        assert_eq!(t.result().as_str(), "-40.00");
        t.reverse = true;
        assert_eq!(t.result().as_str(), "-40.00");
        t.amount = 32;
        assert_eq!(t.result().as_str(), "0.00");
        t.conversion = 4;
        t.amount = 1;
        assert_eq!(t.result().as_str(), "3.79");
        t.amount = 100_000;
        assert_eq!(t.result().as_str(), "378541.18");
    }
    #[test]
    /// Проверяет подтверждение сброса счётчика и отсутствие выхода ниже нуля.
    fn counter_reset_needs_confirmation_and_never_underflows() {
        let mut t = Tools::default();
        t.counter_event(RuntimeEvent::Press);
        t.counter_event(RuntimeEvent::Rotate(2));
        t.counter_event(RuntimeEvent::Press);
        assert_eq!(t.count, 1);
        t.counter_event(RuntimeEvent::Press);
        assert_eq!(t.count, 0);
        t.counter_event(RuntimeEvent::Rotate(-1));
        t.counter_event(RuntimeEvent::Press);
        assert_eq!(t.count, 0);
        t.calendar_event(RuntimeEvent::Rotate(i32::MAX));
        assert_eq!(t.month, 2100 * 12 - 1);
    }
}

#[path = "../system/app/Calendar/main.rs"]
mod calendar;

#[path = "../system/app/Counter/main.rs"]
mod counter;

#[path = "../system/app/Converter/main.rs"]
mod converter;
