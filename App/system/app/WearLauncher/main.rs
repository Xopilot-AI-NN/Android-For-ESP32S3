// Модуль основных экранов часов.
//! pages main Material surface implementation.
use super::*;

impl Surface {

    /// Рисует сетку приложений с прокруткой к выбранной строке.
    pub(super) fn launcher(&mut self, f: MaterialFrame, t: Theme) {
        let count = crate::apps::COUNT as usize;
        let cur = usize::from(f.cursor).min(count);
        // Follow the focused row vertically instead of replacing six-icon pages.
        let rows = count.div_ceil(3);
        let first_row = (cur.min(count - 1) / 3)
            .saturating_sub(1)
            .min(rows.saturating_sub(2));
        let positions = [
            (50, 75),
            (120, 75),
            (190, 75),
            (50, 151),
            (120, 151),
            (190, 151),
        ];
        for (cell, (x, y)) in positions.into_iter().enumerate() {
            let i = first_row * 3 + cell;
            if i >= count {
                break;
            }
            let selected = i == cur;
            self.circle(
                x,
                y,
                32,
                Self::focus_fill(
                    Self::app_accent(crate::apps::launcher_id(i as u8)),
                    selected,
                ),
            );
            self.app_icon(crate::apps::launcher_id(i as u8), x, y, rgb565(8, 20, 17));
        }
        self.text_center(
            199,
            if cur < count {
                crate::apps::LABELS[usize::from(crate::apps::launcher_id(cur as u8))]
            } else {
                ""
            },
            1,
            t.on_surface,
        );
        self.scroll_indicator(cur as u8, crate::apps::COUNT + 1, t);
        self.round_rect(
            55,
            237,
            130,
            35,
            17,
            Self::focus_fill(t.surface_high, cur == count),
        );
        self.text_center(248, "List view", 1, Self::focus_ink(t, cur == count));
    }

    /// Рисует прокручиваемый список установленных приложений.
    pub(super) fn list_launcher(&mut self, f: MaterialFrame, t: Theme) {
        self.photo_header(f, "Apps", t);
        let count = crate::apps::COUNT as usize;
        let cur = usize::from(f.cursor).min(count);
        let start = cur.saturating_sub(1).min(count - 2);
        for row in 0..3 {
            let index = start + row;
            let y = 62 + row as i32 * 66;
            if index == count {
                self.round_rect(
                    55,
                    y,
                    130,
                    60,
                    30,
                    Self::focus_fill(t.surface_high, cur == count),
                );
                self.text_center(y + 12, "Grid view", 1, Self::focus_ink(t, cur == count));
                self.symbol(Symbol::Grid, 120, y + 43, Self::focus_ink(t, cur == count));
            } else {
                self.app_row(
                    y,
                    crate::apps::LABELS[usize::from(crate::apps::launcher_id(index as u8))],
                    index == cur,
                    crate::apps::launcher_id(index as u8),
                    t,
                );
            }
        }
        self.scroll_indicator(cur as u8, crate::apps::COUNT + 1, t);
    }
}
