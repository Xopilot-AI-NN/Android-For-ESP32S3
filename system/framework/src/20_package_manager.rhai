// Compact installed-app registry. System surfaces such as notifications,
// Quick Settings and Tiles are intentionally not exposed as launcher apps.
fn package_label(index) {
    if index==0 {return "CLOCK";}
    if index==1 {return "MEDIA CONTROLS";}
    if index==2 {return "SETTINGS";}
    "SYSTEM INFO"
}
fn package_name(index) {
    if index==0 {return "com.android.deskclock";}
    if index==1 {return "com.android.wear.media";}
    if index==2 {return "com.android.settings";}
    "com.android.settings.deviceinfo"
}
