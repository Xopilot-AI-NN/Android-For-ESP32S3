// AOSP Wear OS Android 17 QPR1 Runtime State ABI v6.
// [0..3] screen, [4..7] cursor, [8..14] brightness,
// [15] DND, [16] airplane, [17..18] theme, [19] locked,
// [20] wifi, [21] bluetooth, [22] ADB/WADB, [23..25] notification count.
fn wrap(value, count) {
    let x = value;
    if x < 0 { x = count - 1; }
    if x >= count { x = 0; }
    x
}
fn pack_state(screen,cursor,brightness,dnd,airplane,theme,locked,wifi,bt,adb,notes) {
    screen + cursor*16 + brightness*256 + dnd*32768 + airplane*65536
        + theme*131072 + locked*524288 + wifi*1048576 + bt*2097152
        + adb*4194304 + notes*8388608
}
fn state_screen(s){s%16}
fn state_cursor(s){(s/16)%16}
fn state_brightness(s){(s/256)%128}
fn state_dnd(s){(s/32768)%2}
fn state_airplane(s){(s/65536)%2}
fn state_theme(s){(s/131072)%4}
fn state_locked(s){(s/524288)%2}
fn state_wifi(s){(s/1048576)%2}
fn state_bt(s){(s/2097152)%2}
fn state_adb(s){(s/4194304)%2}
fn state_notes(s){(s/8388608)%8}
fn with_screen(s,screen,cursor){
    pack_state(screen,cursor,state_brightness(s),state_dnd(s),state_airplane(s),state_theme(s),state_locked(s),state_wifi(s),state_bt(s),state_adb(s),state_notes(s))
}
fn services_ready(){
    service_state("servicemanager")==1 && service_state("surfaceflinger")==1
      && service_state("package")==1 && service_state("activity")==1
      && service_state("power")==1 && service_state("input")==1
      && service_state("connectivity")==1 && service_state("zpager")==1
}
