//! Five clock tabs. The crown selects content immediately; it never starts an
//! alarm/timer just by moving focus through the dock.
use super::*;
impl Surface {
    pub(super) fn clock_dock(&mut self, f: MaterialFrame, t: Theme) {
        let tab = usize::from(f.cursor.min(4));
        self.photo_header(f, "Часы", t);
        if tab == 4 && f.bedtime_editor != 0 {
            let labels = [
                alloc::format!(
                    "Расписание: {}",
                    if f.bedtime_auto {
                        "вкл."
                    } else {
                        "выкл."
                    }
                ),
                alloc::format!(
                    "С {}{}",
                    alloc::format!("{:02}:{:02}", f.bedtime_start / 60, f.bedtime_start % 60),
                    if f.bedtime_editor == 2 {
                        " · ±15 мин"
                    } else {
                        ""
                    }
                ),
                alloc::format!(
                    "По {}{}",
                    alloc::format!("{:02}:{:02}", f.bedtime_end / 60, f.bedtime_end % 60),
                    if f.bedtime_editor == 3 {
                        " · ±15 мин"
                    } else {
                        ""
                    }
                ),
                alloc::format!("Яркость: {}% · AOD", f.bedtime_brightness),
                alloc::string::String::from("Готово"),
            ];
            for (i, label) in labels.iter().enumerate() {
                self.action_chip(
                    64 + i as i32 * 38,
                    34,
                    label,
                    f.bedtime_cursor as usize == i,
                    t,
                );
            }
            self.text_center_raw(262, "Местное время · ежедневно", 1, t.on_surface_variant);
            return;
        }
        self.round_rect(12, 62, 216, 128, 22, t.surface);
        let (minutes, valid) = match tab {
            0 => (f.alarm_minutes, true),
            1 => (
                (i32::from(f.time_minutes) - crate::rtc::timezone_minutes()
                    + i32::from(f.app.world_offset) * 60)
                    .rem_euclid(1440) as u16,
                f.time_valid,
            ),
            _ => (0, true),
        };
        let text = match tab {
            0 | 1 => Self::time_text(minutes),
            2 => Self::mmss_text(if f.timer_secs > 0 {
                f.timer_secs
            } else {
                f.timer_duration_secs
            }),
            _ => Self::mmss_text(f.stopwatch_secs),
        };
        if tab < 4 {
            if valid {
                self.clock_text(80, &text, t.primary);
            } else {
                self.text_center_raw(94, "--:--", 3, t.primary);
            }
            let status = match tab {
                0 => {
                    if f.alarm_enabled {
                        "Включён · изменить"
                    } else {
                        "Выключен · настроить"
                    }
                }
                1 => "Нажатие: часовой пояс",
                2 => {
                    if f.timer_secs > 0 {
                        "Идёт отсчёт · настройка"
                    } else {
                        "Нажатие: выбрать таймер"
                    }
                }
                _ => {
                    if f.stopwatch_running {
                        "Нажатие: пауза"
                    } else {
                        "Нажатие: пуск"
                    }
                }
            };
            if tab == 1 {
                self.text_center_raw(
                    68,
                    &alloc::format!("UTC{:+}", f.app.world_offset),
                    1,
                    t.on_surface_variant,
                );
            }
            self.text_center_raw(158, status, 1, t.on_surface_variant);
        } else {
            self.text_center_raw(
                81,
                if f.bedtime_active {
                    "Включён"
                } else {
                    "Выключен"
                },
                3,
                t.primary,
            );
            self.text_center_raw(
                128,
                &alloc::format!("Яркость {}% · AOD", f.bedtime_brightness),
                1,
                t.on_surface_variant,
            );
            self.text_center_raw(
                151,
                &alloc::format!(
                    "{}–{} · {}",
                    alloc::format!("{:02}:{:02}", f.bedtime_start / 60, f.bedtime_start % 60),
                    alloc::format!("{:02}:{:02}", f.bedtime_end / 60, f.bedtime_end % 60),
                    if f.bedtime_auto {
                        "авто"
                    } else {
                        "выкл."
                    }
                ),
                1,
                t.on_surface_variant,
            );
            self.text_center_raw(171, "OK: вкл. · 2×: настройка", 1, t.on_surface_variant);
        }
        if tab == 3 {
            self.text_center_raw(181, "2×: сброс", 1, t.on_surface_variant);
        }
        let labels = [
            "Будильники",
            "Мировые часы",
            "Таймеры",
            "Секундомер",
            "Режим сна",
        ];
        self.text_center_raw(202, labels[tab], 1, t.primary);
        for i in 0..5 {
            let x = 10 + i as i32 * 45;
            let selected = i == tab;
            let bg = if selected { t.primary } else { t.surface_high };
            let ink = Self::focus_ink(t, selected);
            self.round_rect(x, 226, 40, 44, 12, bg);
            if i < 4 {
                self.symbol(
                    [Symbol::Alarm, Symbol::World, Symbol::Timer, Symbol::Clock][i],
                    x + 20,
                    248,
                    ink,
                );
            } else {
                // Crescent uses existing primitives, without another icon asset.
                self.circle(x + 19, 248, 10, ink);
                self.circle(x + 24, 244, 9, bg);
            }
        }
    }
}
