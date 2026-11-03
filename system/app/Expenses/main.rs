// Журнал расходов: отдельный файл и начальная конфигурация общей модели.
use super::*;
impl Ledger {
    /// Создаёт журнал расходов с отдельным файлом данных.
    pub fn expenses() -> Self {
        let mut s = Self::default();
        s.expenses = true;
        s
    }
    /// Возвращает путь файла данных текущего приложения.
    pub(super) fn path(&self) -> &str {
        if self.expenses {
            "/expenses.txt"
        } else {
            "/tasks.txt"
        }
    }
}
