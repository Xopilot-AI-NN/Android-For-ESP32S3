// Логика приложения Calendar: события энкодера и вычисления.
use crate::apps::utilities::*;
use crate::runtime::RuntimeEvent;
impl Tools {
    /// Готовит календарь и при наличии RTC выбирает текущий месяц.
    pub fn calendar_open(&mut self) {
        self.today = crate::rtc::calendar_date();
        self.calendar_today();
    }
    /// Возвращает календарь к текущему месяцу, если дата известна.
    fn calendar_today(&mut self) {
        if let Some((year, month, _)) = self.today {
            self.month =
                (i32::from(year) * 12 + i32::from(month) - 1).clamp(1900 * 12, 2100 * 12 - 1);
        }
    }
    /// Обрабатывает смену месяца и возврат к сегодняшней дате.
    pub fn calendar_event(&mut self, event: RuntimeEvent) {
        match event {
            RuntimeEvent::Rotate(d) => {
                self.month = self.month.saturating_add(d).clamp(1900 * 12, 2100 * 12 - 1)
            }
            RuntimeEvent::Press => {
                self.today = crate::rtc::calendar_date();
                self.calendar_today();
            }
            _ => {}
        }
    }

}
