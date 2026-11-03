//! Короткие серии нажатий для файлов и редактора; остальные экраны не получают задержку.
#[derive(Default)]
pub struct Clicks {
    count: u8,
    last: u64,
}
impl Clicks {
    /// Движение/длинное нажатие поглощает серию: действие не попадёт в другую строку.
    pub fn cancel(&mut self) {
        self.count = 0;
    }
    /// Тройной клик выдаётся сразу, один/два — после окна 350 мс от последнего отпускания.
    pub fn poll(&mut self, click: bool, now: u64) -> Option<u8> {
        if click {
            self.count = self.count.saturating_add(1);
            self.last = now;
        }
        if self.count == 3 || (self.count != 0 && !click && now.saturating_sub(self.last) >= 350) {
            let n = self.count;
            self.count = 0;
            Some(n)
        } else {
            None
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn singles_doubles_and_triples_do_not_open_before_the_series_ends() {
        let mut c = Clicks::default();
        assert_eq!(c.poll(true, 0), None);
        assert_eq!(c.poll(false, 349), None);
        assert_eq!(c.poll(false, 350), Some(1));
        assert_eq!(c.poll(true, 500), None);
        assert_eq!(c.poll(true, 700), None);
        assert_eq!(c.poll(false, 1050), Some(2));
        c.poll(true, 2000);
        c.poll(true, 2100);
        assert_eq!(c.poll(true, 2200), Some(3));
        c.poll(true, 3000);
        c.cancel();
        assert_eq!(c.poll(false, 4000), None);
    }
}
