// Minimal Android/Wear service registry exported by the Rhai system_server.
fn service_state(name) {
    if name=="servicemanager" {return 1;}
    if name=="surfaceflinger" {return 1;}
    if name=="package" {return 1;}
    if name=="activity" {return 1;}
    if name=="power" {return 1;}
    if name=="input" {return 1;}
    if name=="display" {return 1;}
    if name=="connectivity" {return 1;}
    if name=="wifi" {return 1;}
    if name=="bluetooth" {return 1;}
    if name=="adb" {return 1;}
    if name=="notification" {return 1;}
    if name=="media_session" {return 1;}
    if name=="alarm" {return 1;}
    if name=="battery" {return 1;}
    if name=="launcher" {return 1;}
    if name=="wearable" {return 1;}
    if name=="zpager" {return 1;}
    0
}
