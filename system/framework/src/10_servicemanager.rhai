fn service_state(name) {
    if name=="servicemanager" {return 1;}
    if name=="surfaceflinger" {return 1;}
    if name=="package" {return 1;}
    if name=="activity" {return 1;}
    if name=="power" {return 1;}
    if name=="input" {return 1;}
    if name=="connectivity" {return 1;}
    if name=="wifi" {return 1;}
    if name=="bluetooth" {return 1;}
    if name=="adb" {return 1;}
    if name=="zpager" {return 1;}
    0
}
