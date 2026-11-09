// Обработка экранов Wi-Fi: выбор сети, клавиатура, запрос подключения.
use super::*;
#[cfg(feature = "rhai-runtime")]
impl Runtime {
    /// Обрабатывает ввод Wi-Fi-редактора и возвращает признак обработки.
    pub(super) fn dispatch_wifi_ui(&mut self, event: RuntimeEvent) -> bool {
        let screen = (self.state & 0x1f) as u8;
        let set_sc = |state: i32, sc: u8, cur: u8| -> i32 {
            (state & !0x1ff) | sc as i32 | ((cur as i32) << 5)
        };
        match screen {
            23 => match event {
                RuntimeEvent::Rotate(d) => {
                    let count = self.wifi_ui.aps.len() as i32 + 1; // + Back
                    let cur = ((self.state >> 5) & 0x0f) as i32;
                    let next = (cur + d).rem_euclid(count.max(1));
                    self.state = set_sc(self.state, 23, next as u8);
                    if next < self.wifi_ui.aps.len() as i32 {
                        self.wifi_ui.selected = next as u8;
                    }
                    true
                }
                RuntimeEvent::Press => {
                    let cur = ((self.state >> 5) & 0x0f) as usize;
                    if cur < self.wifi_ui.aps.len() {
                        self.wifi_ui.selected = cur as u8;
                        self.wifi_ui.password.clear();
                        self.wifi_ui.keyboard = crate::text_input::Keyboard::english();
                        self.wifi_ui.keyboard_error = 0;
                        self.state = set_sc(self.state, 24, 0);
                    } else {
                        self.state = set_sc(self.state, 12, 1);
                    }
                    true
                }
                RuntimeEvent::Swipe(2) | RuntimeEvent::Back => {
                    self.state = set_sc(self.state, 12, 1);
                    true
                }
                RuntimeEvent::Sync => true,
                _ => false,
            },
            24 => match event {
                RuntimeEvent::Rotate(delta) => {
                    self.wifi_ui.keyboard.rotate(delta);
                    true
                }
                RuntimeEvent::Press => {
                    use crate::text_input::Action;
                    self.wifi_ui.keyboard_error = 0;
                    match self.wifi_ui.keyboard.press() {
                        Action::Character(ch) => {
                            if self.wifi_ui.password.push(ch).is_err() {
                                self.wifi_ui.keyboard_error = 1;
                            }
                        }
                        Action::Backspace => {
                            self.wifi_ui.password.pop();
                        }
                        Action::Submit => {
                            self.wifi_ui.pending_connect = true;
                            self.wifi_ui.link_state = 1;
                            self.state = set_sc(self.state, 25, 0);
                        }
                        Action::Cancel => {
                            self.state = set_sc(self.state, 23, self.wifi_ui.selected)
                        }
                        Action::Suggestion(_) | Action::None => {}
                    }
                    true
                }
                RuntimeEvent::Swipe(2) | RuntimeEvent::Back => {
                    self.state = set_sc(self.state, 23, self.wifi_ui.selected);
                    true
                }
                RuntimeEvent::Home => {
                    self.state = set_sc(self.state, 1, 0);
                    true
                }
                RuntimeEvent::Sync => true,
                _ => false,
            },
            25 => match event {
                RuntimeEvent::Press | RuntimeEvent::Swipe(2) | RuntimeEvent::Back => {
                    self.state = set_sc(self.state, 12, 1);
                    true
                }
                RuntimeEvent::Home => {
                    self.state = set_sc(self.state, 1, 0);
                    true
                }
                RuntimeEvent::Sync => true,
                _ => false,
            },
            _ => false,
        }
    }
}
