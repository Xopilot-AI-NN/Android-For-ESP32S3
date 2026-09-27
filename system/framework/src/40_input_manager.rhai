// Encoder-first Wear navigation for ESP32-S3 hardware.
// Hardware contract:
//   rotate        -> move focus / scroll
//   short press   -> activate
//   long press    -> Back
//   very long     -> Home (watch face)
// Touch/swipe events remain available only to the desktop development viewer.

fn go_back(state) {
    let screen=state_screen(state);
    if screen==0 || screen==1 {return with_screen(state,1,0);}
    if screen==2 || screen==26 {return with_screen(state,1,0);}
    if screen==3 || screen==4 || screen==9 {return with_screen(state,1,0);}
    if screen==6 || screen==10 || screen==27 {return with_screen(state,2,0);}
    if screen==5 {return with_screen(state,2,0);}
    if screen==7 || screen==14 || screen==17 || screen==18 || screen==19 || screen==20 || screen==21 {return with_screen(state,5,0);}
    if screen==12 || screen==13 {return with_screen(state,7,0);}
    if screen==8 || screen==16 || screen==22 {return with_screen(state,15,0);}
    if screen==11 {return with_screen(state,6,0);}
    state
}

fn on_event(state,kind,value) {
    let screen=state_screen(state); let cursor=state_cursor(state);
    let b=state_brightness(state); let dnd=state_dnd(state); let air=state_airplane(state);
    let th=state_theme(state); let locked=state_locked(state);
    let wifi=state_wifi(state); let bt=state_bt(state); let adb=state_adb(state); let notes=state_notes(state);

    if kind=="sync" {return state;}
    if kind=="home" {return pack_state(1,0,b,dnd,air,th,locked,wifi,bt,adb,notes);}
    if kind=="back" {return go_back(state);}

    if kind=="swipe" {
        // Desktop viewer only: value -1 up, +1 down, -2 left, +2 right.
        if screen==1 {
            if value==-1 {return with_screen(state,3,0);}
            if value==1 {return with_screen(state,4,0);}
            if value==-2 || value==2 {return with_screen(state,9,0);}
            return state;
        }
        if screen==9 && (value==-2 || value==2) {
            let delta=1; if value==-2 {delta=-1;}
            return pack_state(9,wrap(cursor+delta,3),b,dnd,air,th,locked,wifi,bt,adb,notes);
        }
        if screen==3 && value==1 {return with_screen(state,1,0);}
        if screen==4 && value==-1 {return with_screen(state,1,0);}
        if value==2 {return go_back(state);}
        return state;
    }

    if kind=="rotate" {
        if screen==0 {return state;}
        if screen==1 {
            // With no touchscreen, rotation on the watch face gives quick access
            // to the two vertical system surfaces.
            if value>0 {return with_screen(state,4,0);}
            if value<0 {return with_screen(state,3,0);}
            return state;
        }
        if screen==2 {return pack_state(2,wrap(cursor+value,7),b,dnd,air,th,locked,wifi,bt,adb,notes);}
        if screen==26 {return pack_state(26,wrap(cursor+value,7),b,dnd,air,th,locked,wifi,bt,adb,notes);}
        if screen==3 {return pack_state(3,wrap(cursor+value,2),b,dnd,air,th,locked,wifi,bt,adb,notes);}
        if screen==4 {return pack_state(4,wrap(cursor+value,6),b,dnd,air,th,locked,wifi,bt,adb,notes);}
        if screen==5 {return pack_state(5,wrap(cursor+value,9),b,dnd,air,th,locked,wifi,bt,adb,notes);}
        if screen==6 {return pack_state(6,wrap(cursor+value,4),b,dnd,air,th,locked,wifi,bt,adb,notes);}
        if screen==7 {return pack_state(7,wrap(cursor+value,4),b,dnd,air,th,locked,wifi,bt,adb,notes);}
        if screen==9 {return pack_state(9,wrap(cursor+value,3),b,dnd,air,th,locked,wifi,bt,adb,notes);}
        if screen==10 {return pack_state(10,wrap(cursor+value,3),b,dnd,air,th,locked,wifi,bt,adb,notes);}
        if screen==11 {return pack_state(11,wrap(cursor+value,4),b,dnd,air,th,locked,wifi,bt,adb,notes);}
        if screen==12 {return pack_state(12,wrap(cursor+value,3),b,dnd,air,th,locked,wifi,bt,adb,notes);}
        if screen==13 {return pack_state(13,wrap(cursor+value,3),b,dnd,air,th,locked,wifi,bt,adb,notes);}
        if screen==14 {return pack_state(14,wrap(cursor+value,3),b,dnd,air,th,locked,wifi,bt,adb,notes);}
        if screen==15 {return pack_state(15,wrap(cursor+value,4),b,dnd,air,th,locked,wifi,bt,adb,notes);}
        if screen==16 {return pack_state(16,wrap(cursor+value,5),b,dnd,air,th,locked,wifi,bt,adb,notes);}
        if screen==17 {return pack_state(17,wrap(cursor+value,2),b,dnd,air,th,locked,wifi,bt,adb,notes);}
        if screen==18 {return pack_state(18,wrap(cursor+value,2),b,dnd,air,th,locked,wifi,bt,adb,notes);}
        if screen==19 {return pack_state(19,wrap(cursor+value,2),b,dnd,air,th,locked,wifi,bt,adb,notes);}
        if screen==20 {return pack_state(20,wrap(cursor+value,2),b,dnd,air,th,locked,wifi,bt,adb,notes);}
        if screen==21 {return pack_state(21,wrap(cursor+value,3),b,dnd,air,th,locked,wifi,bt,adb,notes);}
        if screen==22 {return pack_state(22,wrap(cursor+value,4),b,dnd,air,th,locked,wifi,bt,adb,notes);}
        return state;
    }

    if kind=="press" {
        if screen==0 {return pack_state(1,0,b,dnd,air,th,0,wifi,bt,adb,notes);}
        if screen==1 {return with_screen(state,2,0);}
        if screen==2 {
            if cursor==6 {return with_screen(state,26,0);} // list view
            return open_package(state,cursor);
        }
        if screen==26 {
            if cursor==6 {return with_screen(state,2,0);} // grid view
            return open_package(state,cursor);
        }

        if screen==3 {
            if notes>0 {return pack_state(3,cursor,b,dnd,air,th,locked,wifi,bt,adb,notes-1);}
            return with_screen(state,1,0);
        }

        if screen==4 {
            if cursor==0 {return pack_state(4,cursor,b,dnd,0,th,locked,1-wifi,bt,adb,notes);}
            if cursor==1 {return pack_state(4,cursor,b,dnd,0,th,locked,wifi,1-bt,adb,notes);}
            if cursor==2 {return pack_state(4,cursor,b,1-dnd,air,th,locked,wifi,bt,adb,notes);}
            if cursor==3 {
                let na=1-air;
                if na==1 {return pack_state(4,cursor,b,dnd,na,th,locked,0,0,adb,notes);}
                return pack_state(4,cursor,b,dnd,na,th,locked,wifi,bt,adb,notes);
            }
            if cursor==4 {let nb=b+10; if nb>100 {nb=20;} return pack_state(4,cursor,nb,dnd,air,th,locked,wifi,bt,adb,notes);}
            return with_screen(state,5,0);
        }

        if screen==5 {
            if cursor==0 {return with_screen(state,7,0);}
            if cursor==1 {return with_screen(state,21,0);}
            if cursor==2 {return with_screen(state,14,0);}
            if cursor==3 {return with_screen(state,17,0);}
            if cursor==4 {return with_screen(state,18,0);}
            if cursor==5 {return with_screen(state,19,0);}
            if cursor==6 {return with_screen(state,20,0);}
            if cursor==7 {return with_screen(state,15,0);}
            return with_screen(state,1,0);
        }

        if screen==6 {
            if cursor<3 {return with_screen(state,11,cursor);}
            return with_screen(state,2,0);
        }

        if screen==7 {
            if cursor==0 {return with_screen(state,12,0);}
            if cursor==1 {return with_screen(state,13,0);}
            if cursor==2 {
                let na=1-air;
                if na==1 {return pack_state(7,cursor,b,dnd,na,th,locked,0,0,adb,notes);}
                return pack_state(7,cursor,b,dnd,na,th,locked,wifi,bt,adb,notes);
            }
            return with_screen(state,5,0);
        }

        if screen==8 {return with_screen(state,15,1);}
        if screen==9 {return with_screen(state,1,0);}
        if screen==10 {if cursor==2 {return with_screen(state,2,0);} return state;}
        if screen==27 {return with_screen(state,2,5);}

        if screen==11 {
            if cursor==0 {clock_action(2); return state;}
            if cursor==1 {clock_action(0); return state;}
            if cursor==2 {clock_action(3); return state;}
            return with_screen(state,6,0);
        }

        if screen==12 {
            if cursor==0 {return pack_state(12,cursor,b,dnd,0,th,locked,1-wifi,bt,adb,notes);}
            if cursor==1 {
                if wifi==0 {return pack_state(12,0,b,dnd,0,th,locked,1,bt,adb,notes);}
                return with_screen(state,23,0);
            }
            return with_screen(state,7,0);
        }
        if screen==13 {
            if cursor==0 {return pack_state(13,cursor,b,dnd,0,th,locked,wifi,1-bt,adb,notes);}
            return with_screen(state,7,1);
        }
        if screen==14 {
            if cursor==0 {let nb=b+10; if nb>100 {nb=20;} return pack_state(14,cursor,nb,dnd,air,th,locked,wifi,bt,adb,notes);}
            if cursor==1 {let nt=th+1; if nt>=4 {nt=0;} return pack_state(14,cursor,b,dnd,air,nt,locked,wifi,bt,adb,notes);}
            return with_screen(state,5,2);
        }
        if screen==15 {
            if cursor==0 {return with_screen(state,16,0);}
            if cursor==1 {return with_screen(state,8,0);}
            if cursor==2 {return with_screen(state,22,0);}
            return with_screen(state,5,7);
        }
        if screen==16 {
            if cursor==0 {time_action(0); return state;}
            if cursor==1 {time_action(1); return state;}
            if cursor==2 {time_action(2); return state;}
            if cursor==3 {time_action(3); return state;}
            return with_screen(state,15,0);
        }
        if screen==17 {return with_screen(state,5,3);}
        if screen==18 {return with_screen(state,5,4);}
        if screen==19 {return with_screen(state,5,5);}
        if screen==20 {
            if cursor==0 {return pack_state(20,cursor,b,dnd,air,th,1-locked,wifi,bt,adb,notes);}
            return with_screen(state,5,6);
        }
        if screen==21 {
            if cursor==0 {return with_screen(state,3,0);}
            if cursor==1 {return with_screen(state,2,0);}
            return with_screen(state,5,1);
        }
        if screen==22 {
            if cursor==0 {return pack_state(22,cursor,b,dnd,air,th,locked,wifi,bt,1-adb,notes);}
            if cursor==1 {return with_screen(state,8,0);}
            if cursor==2 {return with_screen(state,15,0);}
            return with_screen(state,15,2);
        }
    }
    state
}
