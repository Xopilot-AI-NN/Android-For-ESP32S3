// AOSP Wear-compatible Android 17 QPR1 first-stage init.
// With no screen lock configured, Wear boots to the watch face. Absolute wall
// time remains invalid until supplied by the paired companion or set manually.
fn init() {
    if boot_version() < 2 { return -20; }
    if android_api_level() < 1 { return -21; }
    if vendor_hal_level() < 1 { return -22; }
    // screen=watch face, brightness=70, unlocked, radios/ADB opt-in, no fake notification.
    pack_state(1,0,70,0,0,0,0,0,0,0,0)
}
