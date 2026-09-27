// Compact installed-app registry exposed by the encoder-first Wear launcher.
// Notifications / Quick Settings / Tiles remain system surfaces, not apps.
fn package_label(index) {
    if index==0 {return "CLOCK";}
    if index==1 {return "MEDIA";}
    if index==2 {return "SETTINGS";}
    if index==3 {return "CONNECTIVITY";}
    if index==4 {return "SYSTEM INFO";}
    if index==5 {return "ASSISTANT";}
    "" 
}
fn package_name(index) {
    if index==0 {return "com.android.deskclock";}
    if index==1 {return "com.android.wear.media";}
    if index==2 {return "com.android.settings";}
    if index==3 {return "com.android.settings.connectivity";}
    if index==4 {return "com.android.settings.deviceinfo";}
    if index==5 {return "com.android.wear.assistant";}
    ""
}
