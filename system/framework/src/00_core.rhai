// AOSP Wear-compatible Android 17 QPR1 Runtime State ABI v8.
// Compact i32 state: [0..4] screen, [5..8] cursor, [9..15] brightness,
// [16] DND, [17] airplane, [18..19] theme, [20] locked,
// [21] wifi, [22] bluetooth, [23] ADB/WADB, [24..26] notification count.
fn wrap(value, count) {
    let x = value;
    if x < 0 { x = count - 1; }
    if x >= count { x = 0; }
    x
}
fn pack_state(screen,cursor,brightness,dnd,airplane,theme,locked,wifi,bt,adb,notes) {
    screen + cursor*32 + brightness*512 + dnd*65536 + airplane*131072
        + theme*262144 + locked*1048576 + wifi*2097152 + bt*4194304
        + adb*8388608 + notes*16777216
}
fn state_screen(s){s%32}
fn state_cursor(s){(s/32)%16}
fn state_brightness(s){(s/512)%128}
fn state_dnd(s){(s/65536)%2}
fn state_airplane(s){(s/131072)%2}
fn state_theme(s){(s/262144)%4}
fn state_locked(s){(s/1048576)%2}
fn state_wifi(s){(s/2097152)%2}
fn state_bt(s){(s/4194304)%2}
fn state_adb(s){(s/8388608)%2}
fn state_notes(s){(s/16777216)%8}
fn with_screen(s,screen,cursor){
    pack_state(screen,cursor,state_brightness(s),state_dnd(s),state_airplane(s),state_theme(s),state_locked(s),state_wifi(s),state_bt(s),state_adb(s),state_notes(s))
}
fn services_ready(){
    service_state("servicemanager")==1 && service_state("surfaceflinger")==1
      && service_state("package")==1 && service_state("activity")==1
      && service_state("power")==1 && service_state("input")==1
      && service_state("display")==1 && service_state("notification")==1
      && service_state("media_session")==1 && service_state("launcher")==1
      && service_state("connectivity")==1 && service_state("zpager")==1
}
