// Экран Share; состояние поступает из контроллера приложения.
use super::*;
#[allow(unused_imports)]
use core::fmt::Write;
impl Surface {
    /// Отрисовывает текущее состояние приложения без операций ввода-вывода.
    pub(super) fn share_page(&mut self, f: MaterialFrame, t: Theme) {
        use crate::apps::*;
        let a = f.app;
        self.photo_header(f, LABELS[usize::from(a.id)], t);

                let ip = f.wifi_ip;
                if let Some([a, b, c, d]) = ip {
                    let mut line = heapless::String::<48>::new();
                    let _ = write!(line, "{}.{}.{}.{}:5555", a, b, c, d);
                    self.utility_line(0, &line, t);
                    match f.app.cursor {
                        0 => {
                            self.utility_line(1, "ADB по Wi-Fi", t);
                            self.utility_line(2, "adb connect IP:5555", t);
                            self.utility_line(
                                3,
                                if f.adb {
                                    "ADB включён"
                                } else {
                                    "Включите ADB в настройках"
                                },
                                t,
                            );
                        }
                        1 => {
                            self.utility_line(1, "Загрузить PNG 240x280", t);
                            self.utility_line(2, "adb push image.png", t);
                            self.utility_line(3, "/sdcard/Wallpapers/", t);
                        }
                        _ => {
                            self.utility_line(1, "Скачать системные обои", t);
                            self.utility_line(2, "adb pull", t);
                            self.utility_line(3, "/sdcard/Wallpapers/1.png", t);
                        }
                    }
                } else {
                    self.utility_line(1, "Подключитесь к Wi-Fi", t);
                    self.utility_line(2, "Оба устройства в одной сети", t);
                }
                self.text_center(253, "Колесо: инструкция 1–3", 1, t.on_surface_variant);

    }
}
