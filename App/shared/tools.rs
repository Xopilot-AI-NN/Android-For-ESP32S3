// Модель локального календаря.
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
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Tools {
    pub month: i32,
    pub today: Option<(u16, u8, u8)>,
}
impl Default for Tools {
    fn default() -> Self {
        Self {
            month: 2000 * 12,
            today: None,
        }
    }
}

#[path = "../system/app/Calendar/main.rs"]
mod calendar;

/// Длина григорианского месяца.
pub fn days_in_month(year: i32, month: u8) -> u8 {
    match month {
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}
/// Понедельник=0.
pub fn first_weekday(year: i32, month: u8) -> u8 {
    let offsets = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let y = year - i32::from(month < 3);
    ((y + y / 4 - y / 100 + y / 400 + offsets[month as usize - 1] + 7) % 7) as u8
}
