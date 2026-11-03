// Маршрутизация событий и переходов приложений; системные службы выполняют команды отдельно.
use super::*;
#[cfg(feature = "rhai-runtime")]
impl Runtime {
    /// Обрабатывает пользовательский ввод с проверкой блокировки и текущего экрана.
    pub fn dispatch(&mut self, event: RuntimeEvent) -> Result<RuntimeFrame, RuntimeError> {
        let now_ms = Instant::now().duration_since_epoch().as_millis() as u64;
        let now = now_ms / 1000;
        if event == RuntimeEvent::Power {
            self.power_menu = !self.power_menu;
            self.power_cursor = 0;
            self.last_input_ms = now_ms;
            return self.render();
        }
        if self.power_menu {
            self.last_input_ms = now_ms;
            let count = if self.security.locked { 3 } else { 6 };
            match event {
                RuntimeEvent::Rotate(d) => {
                    self.power_cursor = (i32::from(self.power_cursor) + d).rem_euclid(count) as u8
                }
                RuntimeEvent::Press => {
                    self.system_action = match self.power_cursor {
                        0 => Some(SystemAction::PowerOff),
                        1 => Some(SystemAction::Reboot),
                        3 if !self.security.locked => Some(SystemAction::Recovery),
                        4 if !self.security.locked => Some(SystemAction::Fastboot),
                        _ => None,
                    };
                    if self.power_cursor == 2 && !self.security.locked {
                        self.native_app.close();
                        self.state = (self.state & !0x1ff) | 1;
                    }
                    self.power_menu = false;
                }
                RuntimeEvent::Back | RuntimeEvent::Home => self.power_menu = false,
                _ => {}
            }
            return self.render();
        }
        if self.security_saved[5] != 0 || self.security.encode() == self.security_saved {
            self.security.tick(now);
        }
        if event != RuntimeEvent::Sync {
            let asleep = self.display_should_sleep(now_ms);
            self.last_input_ms = now_ms;
            self.security.activity(now);
            if asleep {
                return self.render();
            }
        }
        let event = match event {
            RuntimeEvent::Rotate(d) if self.crown_reverse => {
                RuntimeEvent::Rotate(d.saturating_neg())
            }
            other => other,
        };
        if crate::startup::active() {
            return self.render();
        }
        if self.security.locked {
            // A timer/alarm may be dismissed without unlocking private screens.
            if clock::alert() != 0 && event == RuntimeEvent::Press {
                clock_action(6);
                return self.render();
            }
            self.security.event(event, now);
            if !self.security.locked {
                self.state = (self.state & !0x1ff) | 1;
            }
            return self.render();
        }
        let screen = (self.state & 0x1f) as u8;
        let was_bt_enabled = self.state & (1 << 22) != 0;
        let cursor = if matches!(screen, 2 | 26) { self.launcher_cursor } else { ((self.state >> 5) & 15) as u8 };
        // Native applications share one bounded router; the old SD script ABI stays intact.
        if self.native_app.active() {
            if matches!(event, RuntimeEvent::Back | RuntimeEvent::Swipe(2))
                && self.native_app.cancel_editor()
            {
                return self.render();
            }
            if matches!(
                event,
                RuntimeEvent::Home | RuntimeEvent::Back | RuntimeEvent::Swipe(2)
            ) {
                self.native_app.close();
                if event == RuntimeEvent::Home {
                    self.state = (self.state & !0x1ff) | 1;
                }
            } else if let Some(index) = self.native_app.event(event) {
                self.wallpaper_index = index;
                self.wallpaper_enabled = true;
            }
            return self.render();
        }
        if matches!(screen, 2 | 26) {
            match event {
                RuntimeEvent::Rotate(delta) => {
                    let count = i32::from(crate::apps::COUNT) + 1;
                    self.launcher_cursor = (i32::from(cursor) + delta.rem_euclid(count)).rem_euclid(count) as u8;
                    self.state = (self.state & !0x1e0) | (i32::from(self.launcher_cursor.min(15)) << 5);
                }
                RuntimeEvent::Swipe(direction @ (-1 | 1)) => {
                    let step = if screen == 26 { 3 } else { 1 };
                    let next = (i32::from(cursor) - direction * step)
                        .clamp(0, i32::from(crate::apps::COUNT));
                    self.launcher_cursor = next as u8;
                    self.state = (self.state & !0x1e0) | (next.min(15) << 5);
                }
                RuntimeEvent::Press if cursor == crate::apps::COUNT => {
                    self.launcher_grid = screen == 2;
                    self.launcher_cursor = 0;
                    self.state = (self.state & !0x1ff) | if self.launcher_grid { 26 } else { 2 };
                }
                RuntimeEvent::Press => {
                    if let Some(route) = crate::apps::route(crate::apps::launcher_id(cursor)) {
                        self.state = (self.state & !0x1ff) | i32::from(route);
                    } else {
                        self.native_app.open(crate::apps::launcher_id(cursor));
                    }
                }
                RuntimeEvent::Home | RuntimeEvent::Back | RuntimeEvent::Swipe(2) => {
                    self.state = (self.state & !0x1ff) | 1
                }
                _ => {}
            }
            return self.render();
        }
        if self.name_editor.active {
            match event {
                RuntimeEvent::Rotate(delta) => self.name_editor.rotate(delta),
                RuntimeEvent::Press => self.name_editor.press(),
                RuntimeEvent::Back | RuntimeEvent::Home => {
                    self.name_editor.active = false;
                    if event == RuntimeEvent::Home {
                        self.state = (self.state & !0x1ff) | 1;
                    }
                }
                RuntimeEvent::Swipe(2) => self.name_editor.active = false,
                _ => {}
            }
            return self.render();
        }
        if screen == 8 {
            match event {
                RuntimeEvent::Rotate(d) => {
                    self.state = (self.state & !0x1e0)
                        | ((i32::from(cursor) + d.rem_euclid(4)).rem_euclid(4) << 5)
                }
                RuntimeEvent::Back | RuntimeEvent::Swipe(2) => {
                    self.state = (self.state & !0x1ff) | 15 | (1 << 5)
                }
                RuntimeEvent::Home => self.state = (self.state & !0x1ff) | 1,
                _ => {}
            }
            return self.render();
        }
        if screen == 15 {
            match event {
                RuntimeEvent::Rotate(delta) => {
                    self.state = (self.state & !0x1e0)
                        | ((i32::from(cursor) + delta.rem_euclid(8)).rem_euclid(8) << 5)
                }
                RuntimeEvent::Press => match cursor {
                    0 => self.state = (self.state & !0x1ff) | 16,
                    1 => self.state = (self.state & !0x1ff) | 8,
                    2 => self.state = (self.state & !0x1ff) | 22,
                    3 => self.name_editor.begin(),
                    4 => self.native_app.open(crate::apps::DIAGNOSTICS),
                    5 => {
                        self.power_menu = true;
                        self.power_cursor = 0;
                    }
                    6 => self.native_app.open(crate::apps::MEMORY),
                    _ => self.state = (self.state & !0x1ff) | 5 | (7 << 5),
                },
                RuntimeEvent::Back => self.state = (self.state & !0x1ff) | 5 | (7 << 5),
                RuntimeEvent::Home => self.state = (self.state & !0x1ff) | 1,
                _ => {}
            }
            return self.render();
        }
        if screen == 14 {
            match event {
                RuntimeEvent::Rotate(d) => {
                    self.state = (self.state & !0x1e0)
                        | ((i32::from(cursor) + d.rem_euclid(10)).rem_euclid(10) << 5)
                }
                RuntimeEvent::Press => match cursor {
                    0 => self.state = (self.state & !0x1ff) | 28,
                    1 => {
                        let next = (((self.state >> 18) & 3) + 1) % 4;
                        self.state = (self.state & !(3 << 18)) | (next << 18);
                    }
                    2 => {
                        self.timeout_parent = 14;
                        self.security.begin(5);
                        self.state = (self.state & !0x1ff) | 20;
                    }
                    3 => self.watch_face = (self.watch_face + 1) % 5,
                    4 => self.wallpaper_enabled = !self.wallpaper_enabled,
                    5 => {
                        self.wallpaper_index =
                            (self.wallpaper_index + 1) % (crate::wallpaper::builtin_count() + 1);
                        self.wallpaper_enabled = true;
                    }
                    6 => self.aod_enabled = !self.aod_enabled,
                    7 => {
                        self.wallpaper_level = match self.wallpaper_level {
                            0..=59 => 60,
                            60..=74 => 75,
                            75..=84 => 85,
                            85..=99 => 100,
                            _ => 40,
                        }
                    }
                    8 => self.show_seconds = !self.show_seconds,
                    _ => self.state = (self.state & !0x1ff) | 5 | (2 << 5),
                },
                RuntimeEvent::Back => self.state = (self.state & !0x1ff) | 5 | (2 << 5),
                RuntimeEvent::Home => self.state = (self.state & !0x1ff) | 1,
                _ => {}
            }
            return self.render();
        }
        if screen == 20 {
            if self.security.view != 0 {
                let was_timeout = self.security.view == 5;
                self.security.event(event, now);
                if was_timeout && self.security.view == 0 && self.timeout_parent == 14 {
                    self.state = (self.state & !0x1ff) | 14 | (2 << 5);
                    self.timeout_parent = 20;
                }
                if event == RuntimeEvent::Home {
                    self.state = (self.state & !0x1ff) | 1;
                }
                return self.render();
            }
            match event {
                RuntimeEvent::Rotate(d) => {
                    self.state =
                        (self.state & !0x1e0) | ((i32::from(cursor) + d).rem_euclid(5) << 5)
                }
                RuntimeEvent::Press => match cursor {
                    0 => self
                        .security
                        .begin(if self.security.has_pin { 4 } else { 1 }),
                    1 => {
                        self.timeout_parent = 20;
                        self.security.begin(5);
                    }
                    2 => {
                        if self.security_saved[5] == 0
                            && self.security.encode() != self.security_saved
                        {
                            self.security.message = 7;
                        } else {
                            self.security.lock();
                        }
                    }
                    3 => {
                        if self.security.has_pin {
                            self.security.begin(6);
                        } else {
                            self.security.message = 6;
                        }
                    }
                    _ => self.state = (self.state & !0x1ff) | 5 | (6 << 5),
                },
                RuntimeEvent::Back => self.state = (self.state & !0x1ff) | 5 | (6 << 5),
                RuntimeEvent::Home => self.state = (self.state & !0x1ff) | 1,
                _ => {}
            }
            return self.render();
        }
        if screen == 18 || screen == 19 {
            let count = if screen == 18 { 3 } else { 2 };
            match event {
                RuntimeEvent::Rotate(d) => {
                    self.state =
                        (self.state & !0x1e0) | ((i32::from(cursor) + d).rem_euclid(count) << 5)
                }
                RuntimeEvent::Press => {
                    if screen == 18 && cursor == 0 {
                        self.crown_sensitivity = (self.crown_sensitivity + 1) % 3;
                    } else if screen == 18 && cursor == 1 {
                        self.crown_reverse = !self.crown_reverse;
                    } else if screen == 19 && cursor == 0 {
                        self.high_contrast = !self.high_contrast;
                    } else {
                        self.state =
                            (self.state & !0x1ff) | 5 | ((if screen == 18 { 4 } else { 5 }) << 5);
                    }
                }
                RuntimeEvent::Back => {
                    self.state =
                        (self.state & !0x1ff) | 5 | ((if screen == 18 { 4 } else { 5 }) << 5)
                }
                RuntimeEvent::Home => self.state = (self.state & !0x1ff) | 1,
                _ => {}
            }
            return self.render();
        }
        if clock::alert() != 0 && event == RuntimeEvent::Press {
            clock_action(6);
            return self.render();
        }
        if screen == 6 && matches!(event, RuntimeEvent::Rotate(_) | RuntimeEvent::Swipe(-1 | 1)) {
            let delta = match event {
                RuntimeEvent::Rotate(d) => d,
                RuntimeEvent::Swipe(d) => -d,
                _ => 0,
            };
            self.state = (self.state & !0x1e0)
                | ((i32::from(cursor) + delta.rem_euclid(5)).rem_euclid(5) << 5);
            return self.render();
        }
        if screen == 6 && event == RuntimeEvent::Press {
            if cursor == 0 {
                self.state = (self.state & !0x1ff) | 11;
                self.timer_editor = 1;
            } else if cursor == 1 {
                self.state = (self.state & !0x1ff) | 11;
                self.timer_editor = 3;
            } else if cursor == 2 {
                self.state = (self.state & !0x1ff) | 31;
            } else if cursor == 3 {
                self.native_app.open(crate::apps::WORLD);
            } else {
                self.state = (self.state & !0x1ff) | 2;
            }
            return self.render();
        }
        if screen == 11 && self.timer_editor == 3 {
            match event {
                RuntimeEvent::Rotate(d) => {
                    self.state =
                        (self.state & !0x1e0) | ((i32::from(cursor) + d).rem_euclid(3) << 5)
                }
                RuntimeEvent::Press => match cursor {
                    0 => {
                        clock_action(0);
                    }
                    1 => {
                        clock_action(1);
                    }
                    _ => {
                        self.state = (self.state & !0x1ff) | 6 | (1 << 5);
                    }
                },
                RuntimeEvent::Back => self.state = (self.state & !0x1ff) | 6 | (1 << 5),
                RuntimeEvent::Home => self.state = (self.state & !0x1ff) | 1,
                _ => {}
            }
            return self.render();
        }
        if screen == 11 && self.timer_editor != 0 {
            match event {
                RuntimeEvent::Rotate(delta) if self.timer_editor == 2 => {
                    clock::set_timer_duration(
                        i32::from(clock::timer_duration()).saturating_add(delta.saturating_mul(60)),
                    );
                }
                RuntimeEvent::Rotate(delta) => {
                    self.state =
                        (self.state & !0x1e0) | ((i32::from(cursor) + delta).rem_euclid(5) << 5);
                }
                RuntimeEvent::Press => {
                    if self.timer_editor == 2 {
                        clock::start_timer();
                        self.timer_editor = 1;
                    } else if cursor < 4 {
                        clock::set_timer_duration([300, 600, 900, 1800][cursor as usize]);
                        clock::start_timer();
                    } else {
                        self.timer_editor = 2;
                    }
                }
                RuntimeEvent::Back => {
                    self.timer_editor = if self.timer_editor == 2 { 1 } else { 0 };
                    self.state &= !0x1e0;
                    if self.timer_editor == 0 {
                        self.state = (self.state & !0x1ff) | 6;
                    }
                }
                RuntimeEvent::Home => {
                    self.state = (self.state & !0x1ff) | 1;
                }
                _ => {}
            }
            return self.render();
        }
        if screen == 11 && cursor == 0 && event == RuntimeEvent::Press {
            self.timer_editor = 1;
            return self.render();
        }
        if screen == 11 && cursor == 2 && event == RuntimeEvent::Press {
            self.state = (self.state & !0x1ff) | 31;
            return self.render();
        }
        if screen == 31 {
            match event {
                RuntimeEvent::Rotate(delta) => {
                    self.state =
                        (self.state & !0x1e0) | ((i32::from(cursor) + delta).rem_euclid(4) << 5);
                }
                RuntimeEvent::Press => {
                    if cursor < 3 {
                        clock_action([3, 4, 5][cursor as usize]);
                    } else {
                        self.state = (self.state & !0x1ff) | 6 | (2 << 5);
                    }
                }
                RuntimeEvent::Back => {
                    self.state = (self.state & !0x1ff) | 6 | (2 << 5);
                }
                RuntimeEvent::Home => {
                    self.state = (self.state & !0x1ff) | 1;
                }
                _ => {}
            }
            return self.render();
        }
        if screen == 30 {
            match event {
                RuntimeEvent::Rotate(delta) => {
                    let next = (i32::from(cursor) + delta).rem_euclid(3);
                    self.state = (self.state & !0x1e0) | (next << 5);
                }
                RuntimeEvent::Press => {
                    self.bluetooth_action = Some(if self.bluetooth_pin.is_some() {
                        BluetoothUiAction::Confirm(cursor == 0)
                    } else {
                        match cursor {
                            0 => BluetoothUiAction::Pair,
                            1 => BluetoothUiAction::Disconnect,
                            _ => BluetoothUiAction::Forget,
                        }
                    });
                    if cursor == 0 {
                        self.state = (self.state | (1 << 22)) & !(1 << 17);
                    }
                }
                RuntimeEvent::Back | RuntimeEvent::Home => {
                    if self.bluetooth_pin.is_some() {
                        self.bluetooth_action = Some(BluetoothUiAction::Confirm(false));
                    }
                    self.state =
                        (self.state & !0x1ff) | if event == RuntimeEvent::Home { 1 } else { 13 };
                }
                _ => {}
            }
            return self.render();
        }
        if screen == 27 && event == RuntimeEvent::Press {
            if self.bluetooth_status == 3 {
                self.bluetooth_action = Some(BluetoothUiAction::Media(0x00CF));
            }
            return self.render();
        }
        if screen == 10 && event == RuntimeEvent::Press {
            if let Some(usage) = [0x00B6, 0x00CD, 0x00B5, 0x00EA, 0x00E9].get(cursor as usize) {
                self.bluetooth_action = Some(BluetoothUiAction::Media(*usage));
                return self.render();
            }
        }
        if screen == 23 || screen == 24 || screen == 25 {
            if self.dispatch_wifi_ui(event) {
                return self.render();
            }
        }
        let (kind, value) = match event {
            RuntimeEvent::Sync => ("sync", 0),
            RuntimeEvent::Home => ("home", 0),
            RuntimeEvent::Back => ("back", 0),
            RuntimeEvent::Rotate(delta) => ("rotate", delta),
            RuntimeEvent::Swipe(direction) => ("swipe", direction),
            RuntimeEvent::Press => ("press", 0),
            RuntimeEvent::Power => unreachable!(),
        };
        let options = CallFnOptions::new().eval_ast(false).rewind_scope(true);
        let new_state = self
            .engine
            .call_fn_with_options::<i32>(
                options,
                &mut self.scope,
                &self.ast,
                "on_event",
                (self.state, kind.to_string(), value),
            )
            .map_err(|err| {
                crate::watch_println!("rhai event error: {:?}", err);
                RuntimeError::Script
            })?;
        if new_state < 0 {
            return Err(RuntimeError::NonZero);
        }
        let old_screen = screen;
        self.state = new_state;
        if ((new_state & 31) == 30 && old_screen != 30)
            || (event == RuntimeEvent::Press
                && !was_bt_enabled
                && new_state & (1 << 22) != 0
                && ((screen == 13 && cursor == 0) || (screen == 4 && cursor == 1)))
        {
            self.bluetooth_action = Some(BluetoothUiAction::Pair);
        }
        if old_screen == 9 && (new_state & 31) == 11 {
            self.timer_editor = if ((new_state >> 5) & 15) == 1 { 3 } else { 1 };
        }
        if (new_state & 0x1f) as u8 == 23 && old_screen != 23 {
            self.wifi_ui.pending_scan = true;
            self.wifi_ui.aps.clear();
            self.wifi_ui.selected = 0;
        }
        self.render()
    }

}
