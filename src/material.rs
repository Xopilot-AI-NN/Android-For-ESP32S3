extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;

use crate::font::glyph;

pub const WIDTH: usize = 240;
pub const HEIGHT: usize = 280;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MaterialFrame {
    pub screen: u8,
    pub cursor: u8,
    pub brightness: u8,
    pub dnd: bool,
    pub airplane: bool,
    pub theme: u8,
    pub locked: bool,
    pub wifi: bool,
    pub bt: bool,
    pub adb: bool,
    pub notes: u8,
    pub time_minutes: u16,
    pub swap_pages: u8,
}

impl MaterialFrame {
    pub fn parse(raw: &str) -> Option<Self> {
        let mut p = raw.split('|');
        if p.next()? != "M3" { return None; }
        let screen = p.next()?.parse::<u8>().ok()?.min(8);
        let cursor = p.next()?.parse::<u8>().ok()?.min(15);
        let brightness = p.next()?.parse::<u8>().ok()?.min(100);
        let dnd = p.next()? == "1";
        let airplane = p.next()? == "1";
        let theme = p.next()?.parse::<u8>().ok()? % 4;
        let locked = p.next()? == "1";
        let wifi = p.next()? == "1";
        let bt = p.next()? == "1";
        let adb = p.next()? == "1";
        let notes = p.next()?.parse::<u8>().ok()?.min(7);
        let time_minutes = p.next()?.parse::<u16>().ok()? % 1440;
        if p.next().is_some() { return None; }
        Some(Self { screen, cursor, brightness, dnd, airplane, theme, locked, wifi, bt, adb, notes, time_minutes, swap_pages: 0 })
    }

    pub fn set_swap_pages(&mut self, pages: u8) { self.swap_pages = pages; }

    pub fn fallback(&self) -> (&'static str, &'static str, &'static str, &'static str) {
        match self.screen {
            0 => ("LOCKED", "ZEPHYR WATCH", "PRESS CROWN", ""),
            1 => ("WATCHFACE", "CLOCK", "MESSAGES", "QUICK SETTINGS"),
            2 => ("APPS", "MESSAGES", "SETTINGS", "CLOCK / CONNECT"),
            3 => ("MESSAGES", "NOTIFICATIONS", "PHONE LINK", "BACK"),
            4 => ("QUICK SETTINGS", "WIFI / BT", "ADB / DND", "BRIGHTNESS"),
            5 => ("SETTINGS", "DISPLAY", "THEME", "CONNECTIVITY"),
            6 => ("CLOCK", "UPTIME CLOCK", "RTC/NTP NEXT", "BACK"),
            7 => ("CONNECTIVITY", "WIFI", "BLUETOOTH", "WADB"),
            _ => ("ABOUT", "ZEPHYR ANDROID", "ESP32-S3", "AOSP/RHAI"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Rect { pub x:i32, pub y:i32, pub w:i32, pub h:i32 }
impl Rect {
    pub const FULL:Self=Self{x:0,y:0,w:WIDTH as i32,h:HEIGHT as i32};
    pub fn x1(self)->i32{self.x+self.w-1}
    pub fn y1(self)->i32{self.y+self.h-1}
}

#[derive(Clone, Copy)]
struct Theme {
    bg:u16,surface:u16,surface_high:u16,primary:u16,on_primary:u16,
    primary_container:u16,on_primary_container:u16,secondary_container:u16,
    on_surface:u16,on_surface_variant:u16,outline:u16,error:u16,
}
const fn rgb565(r:u8,g:u8,b:u8)->u16{(((r as u16)&0xF8)<<8)|(((g as u16)&0xFC)<<3)|((b as u16)>>3)}
fn theme(index:u8)->Theme{
    match index%4{
        1=>Theme{bg:rgb565(12,16,24),surface:rgb565(22,27,38),surface_high:rgb565(32,39,53),primary:rgb565(167,199,255),on_primary:rgb565(0,46,92),primary_container:rgb565(18,68,116),on_primary_container:rgb565(213,227,255),secondary_container:rgb565(55,66,84),on_surface:rgb565(228,231,238),on_surface_variant:rgb565(194,199,209),outline:rgb565(139,145,156),error:rgb565(255,180,171)},
        2=>Theme{bg:rgb565(20,14,24),surface:rgb565(31,24,35),surface_high:rgb565(44,34,49),primary:rgb565(224,184,255),on_primary:rgb565(70,24,101),primary_container:rgb565(96,48,127),on_primary_container:rgb565(243,218,255),secondary_container:rgb565(77,57,82),on_surface:rgb565(237,227,238),on_surface_variant:rgb565(209,194,211),outline:rgb565(153,139,155),error:rgb565(255,180,171)},
        3=>Theme{bg:rgb565(24,15,13),surface:rgb565(37,25,22),surface_high:rgb565(51,35,31),primary:rgb565(255,181,157),on_primary:rgb565(92,29,11),primary_container:rgb565(124,52,31),on_primary_container:rgb565(255,219,208),secondary_container:rgb565(82,58,50),on_surface:rgb565(245,226,220),on_surface_variant:rgb565(216,194,187),outline:rgb565(160,140,134),error:rgb565(255,180,171)},
        _=>Theme{bg:rgb565(10,18,13),surface:rgb565(20,30,24),surface_high:rgb565(30,43,35),primary:rgb565(126,223,164),on_primary:rgb565(0,57,29),primary_container:rgb565(20,81,48),on_primary_container:rgb565(158,251,193),secondary_container:rgb565(52,70,59),on_surface:rgb565(224,233,225),on_surface_variant:rgb565(190,202,193),outline:rgb565(137,149,140),error:rgb565(255,180,171)},
    }
}

pub struct Surface{pixels:Vec<u16>,previous:Option<MaterialFrame>}
impl Surface{
    pub fn new()->Self{Self{pixels:vec![0;WIDTH*HEIGHT],previous:None}}
    pub fn pixels(&self)->&[u16]{&self.pixels}
    pub fn render(&mut self,frame:MaterialFrame)->Rect{
        let t=theme(frame.theme);
        let full=self.previous.map_or(true,|p|p.screen!=frame.screen||p.theme!=frame.theme);
        let dirty=if full{Rect::FULL}else{content_rect(frame.screen)};
        self.fill_rect(dirty.x,dirty.y,dirty.w,dirty.h,t.bg);
        match frame.screen{
            0=>self.lock_screen(frame,t),1=>self.watchface(frame,t),2=>self.launcher(frame,t),
            3=>self.notifications(frame,t),4=>self.quick_settings(frame,t),5=>self.settings(frame,t),
            6=>self.clock(frame,t),7=>self.connectivity(frame,t),_=>self.about(frame,t),
        }
        self.previous=Some(frame); dirty
    }
    fn pixel(&mut self,x:i32,y:i32,c:u16){if x>=0&&y>=0&&x<WIDTH as i32&&y<HEIGHT as i32{self.pixels[y as usize*WIDTH+x as usize]=c;}}
    fn fill_rect(&mut self,x:i32,y:i32,w:i32,h:i32,c:u16){if w<=0||h<=0{return;}let x0=x.max(0)as usize;let y0=y.max(0)as usize;let x1=(x+w).min(WIDTH as i32).max(0)as usize;let y1=(y+h).min(HEIGHT as i32).max(0)as usize;for yy in y0..y1{self.pixels[yy*WIDTH+x0..yy*WIDTH+x1].fill(c);}}
    fn round_rect(&mut self,x:i32,y:i32,w:i32,h:i32,r:i32,c:u16){if w<=0||h<=0{return;}let r=r.max(0).min(w/2).min(h/2);self.fill_rect(x+r,y,w-2*r,h,c);self.fill_rect(x,y+r,w,h-2*r,c);for dy in 0..r{for dx in 0..r{let ox=r-dx;let oy=r-dy;if ox*ox+oy*oy<=r*r{self.pixel(x+dx,y+dy,c);self.pixel(x+w-1-dx,y+dy,c);self.pixel(x+dx,y+h-1-dy,c);self.pixel(x+w-1-dx,y+h-1-dy,c);}}}}
    fn circle(&mut self,cx:i32,cy:i32,r:i32,c:u16){for y in -r..=r{for x in -r..=r{if x*x+y*y<=r*r{self.pixel(cx+x,cy+y,c);}}}}
    fn glyph(&mut self,x:i32,y:i32,ch:u8,s:i32,c:u16){let g=glyph(ch.to_ascii_uppercase());let s=s.max(1);for(col,bits)in g.iter().enumerate(){for row in 0..7{if(bits>>row)&1!=0{self.fill_rect(x+col as i32*s,y+row*s,s,s,c);}}}}
    fn text(&mut self,x:i32,y:i32,text:&str,s:i32,c:u16){let mut cx=x;for ch in text.bytes(){self.glyph(cx,y,ch,s,c);cx+=6*s;}}
    fn text_width(text:&str,s:i32)->i32{if text.is_empty(){0}else{(text.len()as i32*6-1)*s}}
    fn text_center(&mut self,y:i32,text:&str,s:i32,c:u16){self.text((WIDTH as i32-Self::text_width(text,s))/2,y,text,s,c);}
    fn time_text(mins:u16)->[u8;5]{let h=(mins/60)%24;let m=mins%60;[b'0'+(h/10)as u8,b'0'+(h%10)as u8,b':',b'0'+(m/10)as u8,b'0'+(m%10)as u8]}
    fn status_icons(&mut self,f:MaterialFrame,t:Theme){
        self.circle(18,18,5,if f.wifi{t.primary}else{t.surface_high}); self.text(29,14,"W",1,if f.wifi{t.primary}else{t.outline});
        self.circle(59,18,5,if f.bt{t.primary}else{t.surface_high}); self.text(70,14,"B",1,if f.bt{t.primary}else{t.outline});
        if f.adb{self.round_rect(91,10,31,16,8,t.primary_container);self.text(99,15,"ADB",1,t.on_primary_container);}
        self.text(181,14,"USB",1,t.on_surface_variant);
    }
    fn appbar(&mut self,title:&str,t:Theme){self.round_rect(10,8,220,34,17,t.surface);self.circle(28,25,7,t.primary);self.text(43,18,title,2,t.on_surface);}
    fn card(&mut self,y:i32,label:&str,selected:bool,t:Theme){let(x,w,h,r,bg,fg)=if selected{(8,224,46,22,t.primary_container,t.on_primary_container)}else{(13,214,40,17,t.surface,t.on_surface)};self.round_rect(x,y,w,h,r,bg);self.circle(x+21,y+h/2,if selected{9}else{7},if selected{t.primary}else{t.surface_high});self.text(x+39,y+(h-14)/2,label,2,fg);self.text(x+w-18,y+(h-7)/2,">",1,fg);}
    fn toggle(&mut self,x:i32,y:i32,on:bool,t:Theme){let bg=if on{t.primary}else{t.surface_high};self.round_rect(x,y,42,24,12,bg);self.circle(if on{x+30}else{x+12},y+12,8,if on{t.on_primary}else{t.on_surface_variant});}
    fn slider(&mut self,x:i32,y:i32,w:i32,v:u8,t:Theme){self.round_rect(x,y,w,8,4,t.surface_high);let a=((w-8)*v as i32/100).max(0);self.round_rect(x,y,a+8,8,4,t.primary);self.circle(x+4+a,y+4,7,t.primary);}

    fn lock_screen(&mut self,f:MaterialFrame,t:Theme){
        self.status_icons(f,t); let tm=Self::time_text(f.time_minutes); let ts=core::str::from_utf8(&tm).unwrap_or("00:00");
        self.text_center(72,ts,5,t.on_surface); self.text_center(120,"ZEPHYR WATCH",1,t.on_surface_variant);
        self.circle(120,158,22,t.primary_container); self.round_rect(112,146,16,22,7,t.primary); self.fill_rect(115,141,10,12,t.primary);
        if f.notes>0{self.round_rect(34,194,172,42,21,t.surface);self.text(52,207,"1 NOTIFICATION",1,t.on_surface);self.circle(188,215,7,t.primary);}
        self.text_center(256,"PRESS CROWN TO UNLOCK",1,t.outline);
    }
    fn watchface(&mut self,f:MaterialFrame,t:Theme){
        self.status_icons(f,t); let tm=Self::time_text(f.time_minutes); let ts=core::str::from_utf8(&tm).unwrap_or("00:00");
        self.text_center(64,ts,5,t.primary); self.text_center(112,"ZEPHYR",1,t.on_surface_variant);
        self.round_rect(18,143,96,58,25,if f.notes>0{t.primary_container}else{t.surface});self.text(33,157,"MESSAGES",1,if f.notes>0{t.on_primary_container}else{t.on_surface});self.text(33,177,if f.notes>0{"1 NEW"}else{"CLEAR"},2,if f.notes>0{t.primary}else{t.on_surface_variant});
        self.round_rect(126,143,96,58,25,t.surface);self.text(143,157,"QUICK",1,t.on_surface);self.text(143,177,if f.dnd{"DND"}else{"READY"},2,if f.dnd{t.primary}else{t.on_surface_variant});
        self.round_rect(46,215,148,30,15,t.secondary_container);self.text_center(225,"CROWN: APPS",1,t.on_surface);
        self.text_center(258,"LEFT MESSAGES  RIGHT QUICK",1,t.outline);
    }
    fn launcher(&mut self,f:MaterialFrame,t:Theme){self.appbar("APPS",t);let labels=["MESSAGES","SETTINGS","CLOCK","CONNECT","ABOUT"];let cur=f.cursor.min(4)as usize;let start=if cur>=4{1}else{0};for row in 0..4{let i=start+row;self.card(52+row as i32*52,labels[i],cur==i,t);}self.text_center(264,"CROWN: OPEN",1,t.outline);}
    fn notifications(&mut self,_f:MaterialFrame,t:Theme){self.appbar("MESSAGES",t);self.round_rect(12,56,216,88,28,t.primary_container);self.circle(38,83,12,t.primary);self.text(58,69,"SYSTEM",2,t.on_primary_container);self.text(58,95,"ZEPHYR IS READY",1,t.on_primary_container);self.text(58,113,"PHONE LINK WAITING",1,t.on_surface_variant);self.round_rect(12,156,216,66,25,t.surface);self.text(31,170,"CHATS",2,t.on_surface);self.text(31,198,"NO PHONE SYNC YET",1,t.on_surface_variant);self.text_center(253,"CROWN: BACK",1,t.outline);}
    fn quick_settings(&mut self,f:MaterialFrame,t:Theme){
        self.appbar("QUICK",t);let tiles=[(12,52,"WIFI",f.wifi),(124,52,"BT",f.bt),(12,112,"ADB",f.adb),(124,112,"DND",f.dnd)];
        for(i,&(x,y,label,on))in tiles.iter().enumerate(){let sel=f.cursor as usize==i;self.round_rect(x,y,104,52,if sel{25}else{20},if sel||on{t.primary_container}else{t.surface});self.circle(x+22,y+20,9,if on{t.primary}else{t.surface_high});self.text(x+39,y+13,label,1,if sel||on{t.on_primary_container}else{t.on_surface});self.text(x+39,y+29,if on{"ON"}else{"OFF"},1,t.on_surface_variant);}
        let air_sel=f.cursor==4;self.round_rect(if air_sel{8}else{12},174,if air_sel{224}else{216},34,17,if air_sel||f.airplane{t.primary_container}else{t.surface});self.text(29,186,"AIRPLANE",1,if air_sel||f.airplane{t.on_primary_container}else{t.on_surface});self.text(180,186,if f.airplane{"ON"}else{"OFF"},1,t.on_surface_variant);
        let bsel=f.cursor==5;self.round_rect(if bsel{8}else{12},218,if bsel{224}else{216},44,20,if bsel{t.primary_container}else{t.surface});self.text(26,229,"BRIGHT",1,if bsel{t.on_primary_container}else{t.on_surface});self.slider(92,235,116,f.brightness,t);
    }
    fn settings(&mut self,f:MaterialFrame,t:Theme){self.appbar("SETTINGS",t);let labels=["BRIGHTNESS","THEME","CONNECT","ABOUT","BACK"];let cur=f.cursor.min(4)as usize;let start=if cur>=4{1}else{0};for row in 0..4{let i=start+row;let y=52+row as i32*52;let sel=cur==i;self.card(y,labels[i],sel,t);if i==0{self.slider(132,y+19,72,f.brightness,t);}if i==1{let n=match f.theme{1=>"BLUE",2=>"PURPLE",3=>"CORAL",_=>"GREEN"};self.text(158,y+18,n,1,t.on_surface_variant);}}}
    fn clock(&mut self,f:MaterialFrame,t:Theme){self.appbar("CLOCK",t);let tm=Self::time_text(f.time_minutes);let ts=core::str::from_utf8(&tm).unwrap_or("00:00");self.round_rect(12,58,216,158,34,t.surface);self.text_center(90,ts,5,t.primary);self.text_center(148,"UPTIME CLOCK",1,t.on_surface_variant);self.round_rect(48,178,144,26,13,t.secondary_container);self.text_center(187,"RTC / NTP READY",1,t.on_surface);self.text_center(249,"CROWN: BACK",1,t.outline);}
    fn connectivity(&mut self,f:MaterialFrame,t:Theme){self.appbar("CONNECT",t);let labels=["WIFI","BLUETOOTH","WADB","AIRPLANE","BACK"];let vals=[f.wifi,f.bt,f.adb,f.airplane,false];let cur=f.cursor.min(4)as usize;let start=if cur>=4{1}else{0};for row in 0..4{let i=start+row;let y=52+row as i32*52;let sel=cur==i;self.card(y,labels[i],sel,t);if i<4{self.toggle(171,y+9,vals[i],t);}}}
    fn about(&mut self,f:MaterialFrame,t:Theme){self.appbar("ABOUT",t);self.round_rect(12,56,216,194,32,t.surface);self.circle(120,91,25,t.primary_container);self.text_center(79,"Z",4,t.primary);self.text_center(127,"ZEPHYR ANDROID",2,t.on_surface);self.text_center(154,"VERSION 0.4.0",1,t.on_surface_variant);self.round_rect(31,178,178,27,13,t.secondary_container);self.text_center(186,"ESP32-S3 / RHAI",1,t.on_surface);self.text_center(214,"PSRAM 2M / SWAP 32M",1,t.on_surface_variant);if f.swap_pages>0{self.text_center(232,"PAGER ACTIVE",1,t.primary);}else{self.text_center(232,"PAGER READY",1,t.outline);}self.text_center(261,"AVB ORANGE",1,t.error);}
}
fn content_rect(screen:u8)->Rect{match screen{0|1=>Rect::FULL,_=>Rect{x:0,y:44,w:WIDTH as i32,h:(HEIGHT-44)as i32}}}
