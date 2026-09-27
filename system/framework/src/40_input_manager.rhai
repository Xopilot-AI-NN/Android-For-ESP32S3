fn on_event(state,kind,value) {
    let screen=state_screen(state); let cursor=state_cursor(state);
    let b=state_brightness(state); let dnd=state_dnd(state); let air=state_airplane(state);
    let th=state_theme(state); let locked=state_locked(state);
    let wifi=state_wifi(state); let bt=state_bt(state); let adb=state_adb(state); let notes=state_notes(state);
    if kind=="sync" {return state;}
    if kind=="home" {return pack_state(0,0,b,dnd,air,th,1,wifi,bt,adb,notes);}
    if kind=="rotate" {
        if screen==1 { if value>0 {return with_screen(state,4,0);} if value<0 {return with_screen(state,3,0);} }
        if screen==2 {return pack_state(2,wrap(cursor+value,5),b,dnd,air,th,locked,wifi,bt,adb,notes);}
        if screen==4 {return pack_state(4,wrap(cursor+value,6),b,dnd,air,th,locked,wifi,bt,adb,notes);}
        if screen==5 {return pack_state(5,wrap(cursor+value,5),b,dnd,air,th,locked,wifi,bt,adb,notes);}
        if screen==7 {return pack_state(7,wrap(cursor+value,5),b,dnd,air,th,locked,wifi,bt,adb,notes);}
        return state;
    }
    if kind=="press" {
        if screen==0 {return pack_state(1,0,b,dnd,air,th,0,wifi,bt,adb,notes);}
        if screen==1 {return with_screen(state,2,0);}
        if screen==2 {return open_package(state,cursor);}
        if screen==3 {return with_screen(state,1,0);}
        if screen==6 || screen==8 {return with_screen(state,2,0);}
        if screen==4 {
            if cursor==0 {return pack_state(4,cursor,b,dnd,0,th,locked,1-wifi,bt,adb,notes);}
            if cursor==1 {return pack_state(4,cursor,b,dnd,0,th,locked,wifi,1-bt,adb,notes);}
            if cursor==2 {return pack_state(4,cursor,b,dnd,air,th,locked,wifi,bt,1-adb,notes);}
            if cursor==3 {return pack_state(4,cursor,b,1-dnd,air,th,locked,wifi,bt,adb,notes);}
            if cursor==4 {let na=1-air; if na==1 {return pack_state(4,cursor,b,dnd,na,th,locked,0,0,adb,notes);} return pack_state(4,cursor,b,dnd,na,th,locked,wifi,bt,adb,notes);}
            let nb=b+10; if nb>100 {nb=20;}
            return pack_state(4,cursor,nb,dnd,air,th,locked,wifi,bt,adb,notes);
        }
        if screen==5 {
            if cursor==0 {let nb=b+10; if nb>100 {nb=20;} return pack_state(5,cursor,nb,dnd,air,th,locked,wifi,bt,adb,notes);}
            if cursor==1 {let nt=th+1; if nt>=4 {nt=0;} return pack_state(5,cursor,b,dnd,air,nt,locked,wifi,bt,adb,notes);}
            if cursor==2 {return with_screen(state,7,0);}
            if cursor==3 {return with_screen(state,8,0);}
            return with_screen(state,1,0);
        }
        if screen==7 {
            if cursor==0 {return pack_state(7,cursor,b,dnd,0,th,locked,1-wifi,bt,adb,notes);}
            if cursor==1 {return pack_state(7,cursor,b,dnd,0,th,locked,wifi,1-bt,adb,notes);}
            if cursor==2 {return pack_state(7,cursor,b,dnd,air,th,locked,wifi,bt,1-adb,notes);}
            if cursor==3 {let na=1-air; if na==1 {return pack_state(7,cursor,b,dnd,na,th,locked,0,0,adb,notes);} return pack_state(7,cursor,b,dnd,na,th,locked,wifi,bt,adb,notes);}
            return with_screen(state,5,0);
        }
    }
    state
}
