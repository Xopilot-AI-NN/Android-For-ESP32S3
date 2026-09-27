// 0 lock, 1 watchface, 2 launcher, 3 notifications, 4 quick settings,
// 5 settings, 6 clock, 7 connectivity, 8 about.
fn open_package(state,index) {
    if index==0 {return with_screen(state,3,0);}
    if index==1 {return with_screen(state,5,0);}
    if index==2 {return with_screen(state,6,0);}
    if index==3 {return with_screen(state,7,0);}
    with_screen(state,8,0)
}
