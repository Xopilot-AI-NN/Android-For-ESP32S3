// AOSP Wear OS / Android 17 QPR1 first-stage init. Boot enters a real lock screen.
fn init() {
    if boot_version() < 2 { return -20; }
    if android_api_level() < 1 { return -21; }
    if vendor_hal_level() < 1 { return -22; }
    // screen=lock, brightness=70, locked=1; radios/ADB are opt-in.
    pack_state(0,0,70,0,0,0,1,0,0,0,1)
}
