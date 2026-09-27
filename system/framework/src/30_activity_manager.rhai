// Activity ids:
// 0 lock, 1 watch face, 2 grid launcher, 3 notifications, 4 quick settings,
// 5 settings, 6 clock, 7 connectivity, 8 about, 9 tiles, 10 media,
// 11 clock tools, 12 Wi-Fi, 13 Bluetooth, 14 display, 15 system,
// 16 date/time, 17 sound, 18 gestures, 19 accessibility, 20 security,
// 21 apps/notifications, 22 developer options, 23 Wi-Fi networks,
// 24 keyboard, 25 Wi-Fi connection, 26 list launcher, 27 assistant.
fn open_package(state,index) {
    if index==0 {return with_screen(state,6,0);}
    if index==1 {return with_screen(state,10,0);}
    if index==2 {return with_screen(state,5,0);}
    if index==3 {return with_screen(state,7,0);}
    if index==4 {return with_screen(state,8,0);}
    if index==5 {return with_screen(state,27,0);}
    state
}
