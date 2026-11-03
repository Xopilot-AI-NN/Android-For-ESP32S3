// Редактирование документа: строки UTF-8, клавиатура и очередь сохранения.
use super::*;
impl Controller {
    /// Подсчитывает строки документа, включая пустые.
    pub(super) fn lines(&self) -> usize {
        self.document.split('\n').count()
    }
    /// Находит байтовые границы выбранной строки.
    pub(super) fn range(&self, index: usize) -> (usize, usize) {
        let mut start = 0;
        for (i, line) in self.document.split('\n').enumerate() {
            if i == index {
                return (start, start + line.len());
            }
            start += line.len() + 1;
        }
        (self.document.len(), self.document.len())
    }
    /// Открывает копию строки в общей клавиатуре, не записывая её сразу на диск.
    pub(super) fn edit_line(&mut self, line: usize) {
        self.line = line;
        let (start, end) = self.range(line);
        self.draft = self.document[start..end].to_string();
        self.keyboard = Keyboard::default();
        self.keyboard.suggest(&self.draft);
        self.mode = 4;
        self.error = 0;
        self.keyboard.view.allow_newline = true;
    }
    /// Ставит сохранение пользовательских изменений в дисковую очередь.
    pub(super) fn save(&mut self) {
        self.queue(
            Op::Write(
                self.file.clone(),
                self.document.as_bytes().to_vec(),
                self.crc,
            ),
            Purpose::Save,
        );
    }
}
