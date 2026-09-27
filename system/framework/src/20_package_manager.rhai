fn package_label(index) {
    if index==0 {return "MESSAGES";}
    if index==1 {return "SETTINGS";}
    if index==2 {return "CLOCK";}
    if index==3 {return "CONNECT";}
    "ABOUT"
}
fn package_name(index) {
    if index==0 {return "com.android.systemui.notifications";}
    if index==1 {return "by.xopilot.settings";}
    if index==2 {return "by.xopilot.clock";}
    if index==3 {return "com.android.settings.connectivity";}
    "android.system.about"
}
