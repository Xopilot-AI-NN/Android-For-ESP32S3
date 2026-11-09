use super::*;
impl Surface {
    /// Заполняет экран белым для режима фонарика.
    pub(super) fn flashlight_page(&mut self) { self.fill_rect(0, 0, WIDTH as i32, HEIGHT as i32, rgb565(255, 255, 255)); }
}
