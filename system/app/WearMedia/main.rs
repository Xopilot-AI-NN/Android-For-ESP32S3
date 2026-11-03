// Модуль основных экранов часов.
//! pages main Material surface implementation.
use super::*;

impl Surface {

    /// Отправляет или рисует управление медиаплеером в текущем компоненте.
    pub(super) fn media(&mut self, f: MaterialFrame, t: Theme) {
        self.photo_header(f, "Media controls", t);
        self.text_center(
            69,
            if f.bt_status == 3 {
                "Phone connected"
            } else {
                "Pair your phone to play"
            },
            1,
            t.on_surface_variant,
        );
        let cream = t.primary;
        let ink = t.on_primary;
        for (i, x) in [(0, 39), (2, 201)] {
            self.circle(x, 151, 26, Self::focus_fill(cream, f.cursor == i));
            self.symbol(
                if i == 0 {
                    Symbol::Previous
                } else {
                    Symbol::Next
                },
                x,
                151,
                ink,
            );
        }
        self.scallop(
            120,
            151,
            Self::focus_fill(cream, f.cursor == 1),
            t.surface_high,
        );
        self.symbol(
            if f.media_playing {
                Symbol::Pause
            } else {
                Symbol::Play
            },
            120,
            151,
            ink,
        );
        if f.media_known {
            self.round_rect(36, 198, 168, 4, 2, t.surface_high);
            self.round_rect(
                36,
                198,
                (168 * i32::from(f.media_progress) / 1000).max(2),
                4,
                2,
                t.primary,
            );
            if f.media_playing {
                for i in 0..3 {
                    let h = 3 + ((u16::from(f.animation_phase) / 20 + i * 3) % 7) as i32;
                    self.round_rect(111 + i as i32 * 7, 230 - h, 3, h, 1, t.primary);
                }
            }
        }
        // HID reports do not contain playback state: Play/Pause is one toggle,
        // and the watch must not fabricate a paused/playing status or metadata.
        self.text_center(
            208,
            if f.media_known {
                if f.media_playing {
                    "Воспроизведение"
                } else {
                    "Пауза"
                }
            } else {
                "Play / pause"
            },
            1,
            t.on_surface_variant,
        );
        for (i, x, label) in [(3, 43, "Vol -"), (4, 120, "Vol +"), (5, 197, "Back")] {
            self.round_rect(
                x - 33,
                237,
                66,
                31,
                15,
                if f.cursor == i { cream } else { t.surface_high },
            );
            self.text_center_x(
                x,
                247,
                label,
                1,
                if f.cursor == i { ink } else { t.on_surface },
            );
        }
    }
}
