//! Ignore clock ticks that do not change the visible page.
use super::MaterialFrame;
impl MaterialFrame {
    pub fn visual_key(mut self) -> Self {
        if self.aod {
            self.timer_secs = 0;
            self.stopwatch_secs = 0;
            self.lockout_secs = 0;
            self.media_progress = 0;
        }
        let native = self.app.id != crate::apps::NONE;
        // Hidden stopwatches and countdowns must not invalidate static pages.
        // Alerts retain their own clock_alert field and still repaint immediately.
        if self.aod || self.power_menu || native || !matches!(self.screen, 1 | 9 | 11) {
            self.timer_secs = 0;
            self.stopwatch_secs = 0;
        }
        if self.aod || native || self.screen != 10 {
            self.media_progress = 0;
        }
        let analog = self.screen == 1
            && self.watch_face == 1
            && self.show_seconds
            && !native
            && !self.power_menu
            && !self.aod;
        let animated_media =
            self.screen == 10 && self.media_known && self.media_playing && !self.power_menu;
        if !analog {
            self.time_seconds = 0;
        }
        if !analog && !animated_media {
            self.animation_phase = 0;
        }
        self
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn static_ticks_coalesce_but_animation_and_dimming_are_not_lost() {
        let mut a = MaterialFrame::parse("M3|5|0|70|0|0|0|0|1|1|0|0|600|0|0|0|1|1|3").unwrap();
        let mut b = a;
        b.animation_phase = 120;
        b.time_seconds = 37;
        assert_eq!(a.visual_key(), b.visual_key());
        b.display_brightness = Some(15);
        assert_ne!(a.visual_key(), b.visual_key());
        a.screen = 1;
        a.watch_face = 1;
        b = a;
        b.animation_phase = 120;
        assert_ne!(a.visual_key(), b.visual_key());
        a.screen = 10;
        a.media_known = true;
        a.media_playing = true;
        b = a;
        b.animation_phase = 120;
        assert_ne!(a.visual_key(), b.visual_key());
    }
}
