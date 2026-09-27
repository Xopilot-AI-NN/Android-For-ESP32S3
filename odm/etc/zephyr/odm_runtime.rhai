// /odm: concrete board implementation details.
fn odm_model() { "N4R2" }
fn odm_panel() {
    if device_width() >= 200 { "ST7789V3" } else { "SSD1306" }
}
