fn render(state) {
    if !services_ready() {return "ANDROID|SYSTEM SERVER|SERVICE FAILURE|FASTBOOT ADVISED";}
    m3_scene(state_screen(state),state_cursor(state),state_brightness(state),state_dnd(state),state_airplane(state),state_theme(state),state_locked(state),state_wifi(state),state_bt(state),state_adb(state),state_notes(state))
}
