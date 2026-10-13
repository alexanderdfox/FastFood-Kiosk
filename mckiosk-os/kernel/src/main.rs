#![no_std]
#![no_main]

use bootloader_api::{config::Mapping, entry_point, BootInfo, info::FrameBufferInfo};
use core::panic::PanicInfo;

pub static BOOTLOADER_CONFIG: bootloader_api::BootloaderConfig = {
    let mut config = bootloader_api::BootloaderConfig::new_default();
    config.mappings.physical_memory = Some(Mapping::Dynamic);
    config
};

entry_point!(kernel_main, config = &BOOTLOADER_CONFIG);

// ---------------------------------------------------------------------------
// Port I/O
// ---------------------------------------------------------------------------
#[inline(always)] unsafe fn outb(port: u16, val: u8) {
    core::arch::asm!("out dx, al", in("dx") port, in("al") val, options(nomem, nostack, preserves_flags));
}
#[inline(always)] unsafe fn inb(port: u16) -> u8 {
    let v: u8; core::arch::asm!("in al, dx", out("al") v, in("dx") port, options(nomem, nostack, preserves_flags)); v
}

fn kbd() -> Option<u8> {
    unsafe { if inb(0x64) & 1 != 0 { Some(inb(0x60)) } else { None } }
}

struct Mouse { x: i32, y: i32, left: bool, pkt: [u8;3], idx: usize }
impl Mouse {
    const fn new() -> Self { Self { x: 400, y: 300, left: false, pkt: [0;3], idx: 0 } }
}
fn mouse_init() {
    unsafe {
        outb(0x64, 0xA8); outb(0x64, 0x20);
        let mut s = inb(0x60); s |= 2;
        outb(0x64, 0x60); outb(0x60, s);
        outb(0x64, 0xD4); outb(0x60, 0xF6);
        outb(0x64, 0xD4); outb(0x60, 0xF4);
    }
}
fn mouse_poll(m: &mut Mouse) {
    unsafe {
        let st = inb(0x64);
        if st & 0x20 == 0 || st & 1 == 0 { return; }
        let d = inb(0x60);
        m.pkt[m.idx] = d; m.idx += 1;
        if m.idx == 3 {
            m.idx = 0;
            let f = m.pkt[0];
            if f & 0x08 == 0 { return; }
            m.left = f & 1 != 0;
            let mut dx = m.pkt[1] as i32; let mut dy = m.pkt[2] as i32;
            if f & 0x10 != 0 { dx -= 256; }
            if f & 0x20 != 0 { dy -= 256; }
            m.x = (m.x + dx).clamp(0, 1270);
            m.y = (m.y - dy).clamp(0, 710);
        }
    }
}

// ---------------------------------------------------------------------------
// Graphics
// ---------------------------------------------------------------------------
#[inline(always)]
fn put(buf: &mut [u8], info: &FrameBufferInfo, x: usize, y: usize, c: u32) {
    if x >= info.width || y >= info.height { return; }
    let o = (y * info.stride + x) * info.bytes_per_pixel;
    if o + 2 >= buf.len() { return; }
    buf[o] = (c & 0xFF) as u8;
    buf[o+1] = ((c>>8) & 0xFF) as u8;
    buf[o+2] = ((c>>16) & 0xFF) as u8;
}

fn fill(buf: &mut [u8], info: &FrameBufferInfo, x: usize, y: usize, w: usize, h: usize, c: u32) {
    let b = (c&0xFF) as u8; let g = ((c>>8)&0xFF) as u8; let r = ((c>>16)&0xFF) as u8;
    let bpp = info.bytes_per_pixel;
    for row in y..y.saturating_add(h).min(info.height) {
        let mut o = (row * info.stride + x) * bpp;
        for _ in 0..w {
            if o+2 >= buf.len() { break; }
            buf[o]=b; buf[o+1]=g; buf[o+2]=r; o += bpp;
        }
    }
}

fn clear(buf: &mut [u8], info: &FrameBufferInfo, bg: u32) {
    let b = (bg&0xFF) as u8; let g = ((bg>>8)&0xFF) as u8; let r = ((bg>>16)&0xFF) as u8;
    let bpp = info.bytes_per_pixel;
    let mut i = 0;
    while i+2 < buf.len() {
        buf[i]=b; buf[i+1]=g; buf[i+2]=r;
        i += bpp;
    }
}

fn glyph(c: char) -> [u8;8] {
    match c {
        ' '=>[0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00],
        '!'=>[0x18,0x3C,0x3C,0x18,0x18,0x00,0x18,0x00],
        '$'=>[0x0C,0x3E,0x03,0x1E,0x30,0x1F,0x0C,0x00],
        '%'=>[0x00,0x63,0x33,0x18,0x0C,0x66,0x63,0x00],
        '('=>[0x18,0x0C,0x06,0x06,0x06,0x0C,0x18,0x00],
        ')'=>[0x06,0x0C,0x18,0x18,0x18,0x0C,0x06,0x00],
        '+'=>[0x00,0x0C,0x0C,0x3F,0x0C,0x0C,0x00,0x00],
        '-'=>[0x00,0x00,0x00,0x3F,0x00,0x00,0x00,0x00],
        '.'=>[0x00,0x00,0x00,0x00,0x00,0x0C,0x0C,0x00],
        '/'=>[0x60,0x30,0x18,0x0C,0x06,0x03,0x01,0x00],
        '0'=>[0x3E,0x63,0x73,0x7B,0x6F,0x67,0x3E,0x00],
        '1'=>[0x0C,0x0E,0x0C,0x0C,0x0C,0x0C,0x3F,0x00],
        '2'=>[0x1E,0x33,0x30,0x1C,0x06,0x33,0x3F,0x00],
        '3'=>[0x1E,0x33,0x30,0x1C,0x30,0x33,0x1E,0x00],
        '4'=>[0x38,0x3C,0x36,0x33,0x7F,0x30,0x78,0x00],
        '5'=>[0x3F,0x03,0x1F,0x30,0x30,0x33,0x1E,0x00],
        '6'=>[0x1C,0x06,0x03,0x1F,0x33,0x33,0x1E,0x00],
        '7'=>[0x3F,0x33,0x30,0x18,0x0C,0x0C,0x0C,0x00],
        '8'=>[0x1E,0x33,0x33,0x1E,0x33,0x33,0x1E,0x00],
        '9'=>[0x1E,0x33,0x33,0x3E,0x30,0x18,0x0E,0x00],
        ':'=>[0x00,0x0C,0x0C,0x00,0x00,0x0C,0x0C,0x00],
        'A'=>[0x0C,0x1E,0x33,0x33,0x3F,0x33,0x33,0x00],
        'B'=>[0x3F,0x66,0x66,0x3E,0x66,0x66,0x3F,0x00],
        'C'=>[0x3C,0x66,0x03,0x03,0x03,0x66,0x3C,0x00],
        'D'=>[0x1F,0x36,0x66,0x66,0x66,0x36,0x1F,0x00],
        'E'=>[0x7F,0x46,0x16,0x1E,0x16,0x46,0x7F,0x00],
        'F'=>[0x7F,0x46,0x16,0x1E,0x16,0x06,0x0F,0x00],
        'G'=>[0x3C,0x66,0x03,0x03,0x73,0x66,0x7C,0x00],
        'H'=>[0x33,0x33,0x33,0x3F,0x33,0x33,0x33,0x00],
        'I'=>[0x1E,0x0C,0x0C,0x0C,0x0C,0x0C,0x1E,0x00],
        'J'=>[0x78,0x30,0x30,0x30,0x33,0x33,0x1E,0x00],
        'K'=>[0x67,0x66,0x36,0x1E,0x36,0x66,0x67,0x00],
        'L'=>[0x0F,0x06,0x06,0x06,0x46,0x66,0x7F,0x00],
        'M'=>[0x63,0x77,0x7F,0x7F,0x6B,0x63,0x63,0x00],
        'N'=>[0x63,0x67,0x6F,0x7B,0x73,0x63,0x63,0x00],
        'O'=>[0x1C,0x36,0x63,0x63,0x63,0x36,0x1C,0x00],
        'P'=>[0x3F,0x66,0x66,0x3E,0x06,0x06,0x0F,0x00],
        'Q'=>[0x1E,0x33,0x33,0x33,0x3B,0x1E,0x38,0x00],
        'R'=>[0x3F,0x66,0x66,0x3E,0x36,0x66,0x67,0x00],
        'S'=>[0x1E,0x33,0x07,0x0E,0x38,0x33,0x1E,0x00],
        'T'=>[0x3F,0x2D,0x0C,0x0C,0x0C,0x0C,0x1E,0x00],
        'U'=>[0x33,0x33,0x33,0x33,0x33,0x33,0x3F,0x00],
        'V'=>[0x33,0x33,0x33,0x33,0x33,0x1E,0x0C,0x00],
        'W'=>[0x63,0x63,0x63,0x6B,0x7F,0x77,0x63,0x00],
        'X'=>[0x63,0x63,0x36,0x1C,0x1C,0x36,0x63,0x00],
        'Y'=>[0x33,0x33,0x33,0x1E,0x0C,0x0C,0x1E,0x00],
        'Z'=>[0x7F,0x63,0x31,0x18,0x4C,0x66,0x7F,0x00],
        'a'=>[0x00,0x00,0x1E,0x30,0x3E,0x33,0x6E,0x00],
        'b'=>[0x07,0x06,0x06,0x3E,0x66,0x66,0x3B,0x00],
        'c'=>[0x00,0x00,0x1E,0x33,0x03,0x33,0x1E,0x00],
        'd'=>[0x38,0x30,0x30,0x3E,0x33,0x33,0x6E,0x00],
        'e'=>[0x00,0x00,0x1E,0x33,0x3F,0x03,0x1E,0x00],
        'f'=>[0x1C,0x36,0x06,0x0F,0x06,0x06,0x0F,0x00],
        'g'=>[0x00,0x00,0x6E,0x33,0x33,0x3E,0x30,0x1F],
        'h'=>[0x07,0x06,0x36,0x6E,0x66,0x66,0x67,0x00],
        'i'=>[0x0C,0x00,0x0E,0x0C,0x0C,0x0C,0x1E,0x00],
        'k'=>[0x07,0x06,0x66,0x36,0x1E,0x36,0x67,0x00],
        'l'=>[0x0E,0x0C,0x0C,0x0C,0x0C,0x0C,0x1E,0x00],
        'm'=>[0x00,0x00,0x33,0x7F,0x7F,0x6B,0x63,0x00],
        'n'=>[0x00,0x00,0x1F,0x33,0x33,0x33,0x33,0x00],
        'o'=>[0x00,0x00,0x1E,0x33,0x33,0x33,0x1E,0x00],
        'p'=>[0x00,0x00,0x3B,0x66,0x66,0x3E,0x06,0x0F],
        'r'=>[0x00,0x00,0x3B,0x6E,0x66,0x06,0x0F,0x00],
        's'=>[0x00,0x00,0x3E,0x03,0x1E,0x30,0x1F,0x00],
        't'=>[0x08,0x0C,0x3E,0x0C,0x0C,0x2C,0x18,0x00],
        'u'=>[0x00,0x00,0x33,0x33,0x33,0x33,0x6E,0x00],
        'v'=>[0x00,0x00,0x33,0x33,0x33,0x1E,0x0C,0x00],
        'w'=>[0x00,0x00,0x63,0x6B,0x7F,0x7F,0x36,0x00],
        'x'=>[0x00,0x00,0x63,0x36,0x1C,0x36,0x63,0x00],
        'y'=>[0x00,0x00,0x33,0x33,0x33,0x3E,0x30,0x1F],
        '\''=>[0x0C,0x0C,0x06,0x00,0x00,0x00,0x00,0x00],
        ','=>[0x00,0x00,0x00,0x00,0x00,0x0C,0x0C,0x06],
        _  =>[0xFF,0x81,0x81,0x81,0x81,0x81,0xFF,0x00],
    }
}

fn dchar(buf: &mut [u8], info: &FrameBufferInfo, x: usize, y: usize, c: char, col: u32) {
    let g = glyph(c);
    for row in 0..8 {
        for colb in 0..8 {
            if g[row] & (1 << colb) != 0 {
                put(buf, info, x+colb, y+row, col);
            }
        }
    }
}

fn dtext(buf: &mut [u8], info: &FrameBufferInfo, mut x: usize, y: usize, t: &str, col: u32) {
    for c in t.chars() { dchar(buf, info, x, y, c, col); x += 9; }
}

fn dtext2(buf: &mut [u8], info: &FrameBufferInfo, x: usize, y: usize, t: &str, col: u32, sc: usize) {
    let mut cx = x;
    for c in t.chars() {
        let g = glyph(c);
        for row in 0..8 {
            for colb in 0..8 {
                if g[row] & (1 << colb) != 0 {
                    for sy in 0..sc { for sx in 0..sc {
                        put(buf, info, cx+colb*sc+sx, y+row*sc+sy, col);
                    }}
                }
            }
        }
        cx += 9*sc;
    }
}

fn cursor(buf: &mut [u8], info: &FrameBufferInfo, x: usize, y: usize) {
    for i in 0..12 { put(buf, info, x, y+i, 0xFFFFFF); }
    for i in 0..8 { put(buf, info, x+i, y+i, 0xFFFFFF); }
}

// ---------------------------------------------------------------------------
// On-screen keyboard
// ---------------------------------------------------------------------------
const OSK: &[&str] = &["1234567890", "qwertyuiop", "asdfghjkl", "zxcvbnm "];

fn draw_osk(buf: &mut [u8], info: &FrameBufferInfo, yb: usize) {
    fill(buf, info, 0, yb, info.width, 155, 0x111111);
    fill(buf, info, 0, yb, info.width, 2, 0x00D7FF);
    let kw = 46usize; let kh = 30usize;
    let mut y = yb + 8;
    for row in OSK {
        let mut x = 30usize;
        for c in row.chars() {
            fill(buf, info, x, y, kw-3, kh-3, 0x2A2A2A);
            dchar(buf, info, x+14, y+9, c, 0xFFFFFF);
            x += kw;
        }
        y += kh;
    }
    // special
    fill(buf, info, 30, y, 70, kh-3, 0xFF7000);
    dtext(buf, info, 42, y+9, "SPC", 0xFFFFFF);
    fill(buf, info, 110, y, 70, kh-3, 0x0000E0);
    dtext(buf, info, 125, y+9, "DEL", 0xFFFFFF);
    fill(buf, info, 190, y, 70, kh-3, 0x00CC44);
    dtext(buf, info, 208, y+9, "OK", 0xFFFFFF);
    fill(buf, info, 270, y, 80, kh-3, 0x666666);
    dtext(buf, info, 285, y+9, "HIDE", 0xFFFFFF);
}

fn osk_hit(mx: usize, my: usize, yb: usize) -> Option<char> {
    if my < yb || my > yb+155 { return None; }
    let kw = 46usize; let kh = 30usize;
    let ry = my - (yb+8);
    let row = ry / kh;
    if row > 4 { return None; }
    if row == 4 {
        if mx>=30 && mx<100 { return Some(' '); }
        if mx>=110 && mx<180 { return Some('\x08'); }
        if mx>=190 && mx<260 { return Some('\n'); }
        if mx>=270 && mx<350 { return Some('\x1b'); }
        return None;
    }
    let rs = OSK.get(row)?;
    let cx = mx.saturating_sub(30) / kw;
    rs.chars().nth(cx)
}

// ---------------------------------------------------------------------------
// Data
// ---------------------------------------------------------------------------
const MAX_ITEMS: usize = 16;
const MAX_NAME: usize = 22;
const MAX_RNAME: usize = 20;

#[derive(Copy, Clone)]
struct Item {
    name: [u8; MAX_NAME],
    nlen: usize,
    price: u16,
    active: bool,
}
impl Item {
    const fn empty() -> Self { Self { name: [0;MAX_NAME], nlen: 0, price: 0, active: false } }
    fn set_name(&mut self, s: &str) {
        self.nlen = 0;
        for b in s.bytes().take(MAX_NAME) { self.name[self.nlen]=b; self.nlen+=1; }
    }
    fn name(&self) -> &str { core::str::from_utf8(&self.name[..self.nlen]).unwrap_or("?") }
}

struct Brand {
    name: [u8; MAX_RNAME],
    nlen: usize,
    primary: u32,   // header / buttons
    accent: u32,    // gold line / highlights
    bg: u32,        // background
}
impl Brand {
    fn set_name(&mut self, s: &str) {
        self.nlen = 0;
        for b in s.bytes().take(MAX_RNAME) { self.name[self.nlen]=b; self.nlen+=1; }
    }
    fn name(&self) -> &str { core::str::from_utf8(&self.name[..self.nlen]).unwrap_or("Kiosk") }
}

struct Pass {
    data: [u8;16],
    len: usize,
}
impl Pass {
    fn set(&mut self, s: &str) {
        self.len = 0;
        for b in s.bytes().take(16) { self.data[self.len] = b ^ 0x5A; self.len += 1; }
    }
    fn check(&self, a: &[u8]) -> bool {
        if a.len() != self.len { return false; }
        for i in 0..self.len { if self.data[i]^0x5A != a[i] { return false; } }
        true
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Scr { Splash, Menu, Cart, Confirm, Login, Admin, Edit, ChPass, Colors, BrandName }

// Preset color themes (BGR)
const THEMES: &[(u32,u32,u32,&str)] = &[
    (0x0000E0, 0x00D7FF, 0x000000, "Red/Gold"),      // McD style
    (0x00AA00, 0xFFFFFF, 0x002200, "Green"),         // healthy
    (0xCC4400, 0x00AAFF, 0x110800, "Blue/Orange"),   // modern
    (0x8800AA, 0x00FFCC, 0x100010, "Purple"),
    (0x333333, 0x00D7FF, 0x111111, "Dark Gold"),
    (0x000080, 0xFFFFFF, 0x000020, "Navy"),
];

fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    let fb = match boot_info.framebuffer.as_mut() {
        Some(f) => f, None => loop {},
    };
    let info = fb.info().clone();
    let buffer = fb.buffer_mut();

    mouse_init();
    let mut mouse = Mouse::new();
    mouse.x = (info.width/2) as i32;
    mouse.y = (info.height/2) as i32;

    let mut brand = Brand {
        name: [0;MAX_RNAME], nlen: 0,
        primary: 0x0000E0,
        accent: 0x00D7FF,
        bg: 0x000000,
    };
    brand.set_name("McDonald's");

    let mut items = [Item::empty(); MAX_ITEMS];
    let defs = [
        ("Big Mac",599),("Quarter Pounder",649),("McChicken",399),
        ("Filet-O-Fish",499),("Cheeseburger",249),("World Famous Fries",299),
        ("10pc McNuggets",549),("Coca-Cola",199),("Sprite",199),
        ("McFlurry Oreo",349),("Apple Pie",149),
    ];
    for (i,(n,p)) in defs.iter().enumerate() {
        items[i].set_name(n); items[i].price=*p; items[i].active=true;
    }

    let mut pass = Pass { data:[0;16], len:0 };
    pass.set("admin");

    let mut qty = [0u8; MAX_ITEMS];
    let mut scr = Scr::Splash;
    let mut sel: usize = 0;
    let mut order: u32 = 1001;
    let mut last_left = false;
    let mut show_osk = false;
    let mut ibuf = [0u8; 32];
    let mut ilen: usize = 0;
    let mut edit_i: usize = 0;
    let mut theme_i: usize = 0;
    let mut dirty = true;          // only redraw when needed
    let mut login_err = false;
    let mut prev_mx = 0i32;
    let mut prev_my = 0i32;

    loop {
        mouse_poll(&mut mouse);
        let mut input_happened = false;

        if let Some(sc) = kbd() {
            input_happened = true;
            match sc {
                0x48 => if sel > 0 { sel -= 1; },
                0x50 => {
                    let c = items.iter().filter(|i| i.active).count();
                    if sel+1 < c { sel += 1; }
                },
                0x1C => match scr {
                    Scr::Splash => scr = Scr::Menu,
                    Scr::Menu => {
                        let mut idx=0;
                        for (i,it) in items.iter().enumerate() {
                            if it.active {
                                if idx==sel { qty[i]=qty[i].saturating_add(1); break; }
                                idx+=1;
                            }
                        }
                    }
                    Scr::Cart => { scr=Scr::Confirm; order+=1; qty=[0;MAX_ITEMS]; }
                    Scr::Confirm => scr=Scr::Menu,
                    Scr::Login => {
                        if pass.check(&ibuf[..ilen]) { scr=Scr::Admin; login_err=false; }
                        else { login_err=true; }
                        ilen=0;
                    }
                    _ => {}
                },
                0x01 => match scr {
                    Scr::Cart|Scr::Confirm|Scr::Admin => scr=Scr::Menu,
                    Scr::Login|Scr::Edit|Scr::ChPass|Scr::Colors|Scr::BrandName => {
                        scr=Scr::Menu; ilen=0; show_osk=false;
                    }
                    _ => {}
                },
                0x0F => if scr==Scr::Menu { scr=Scr::Cart; },
                0x3B => { scr=Scr::Login; ilen=0; show_osk=true; login_err=false; },
                _ => {}
            }
        }

        let clicked = mouse.left && !last_left;
        last_left = mouse.left;
        if mouse.x != prev_mx || mouse.y != prev_my { input_happened = true; }
        prev_mx = mouse.x; prev_my = mouse.y;

        if clicked {
            input_happened = true;
            let mx = mouse.x as usize;
            let my = mouse.y as usize;

            if show_osk {
                let yb = info.height - 165;
                if let Some(ch) = osk_hit(mx, my, yb) {
                    match ch {
                        '\x1b' => show_osk = false,
                        '\n' => match scr {
                            Scr::Login => {
                                if pass.check(&ibuf[..ilen]) { scr=Scr::Admin; login_err=false; }
                                else { login_err=true; }
                                ilen=0;
                            }
                            Scr::Edit => {
                                items[edit_i].set_name(core::str::from_utf8(&ibuf[..ilen]).unwrap_or("Item"));
                                scr=Scr::Admin; ilen=0; show_osk=false;
                            }
                            Scr::BrandName => {
                                brand.set_name(core::str::from_utf8(&ibuf[..ilen]).unwrap_or("Kiosk"));
                                scr=Scr::Admin; ilen=0; show_osk=false;
                            }
                            Scr::ChPass => {
                                if ilen >= 4 {
                                    pass.set(core::str::from_utf8(&ibuf[..ilen]).unwrap_or("admin"));
                                    scr=Scr::Admin; ilen=0; show_osk=false;
                                }
                            }
                            _ => {}
                        },
                        '\x08' => if ilen>0 { ilen-=1; },
                        c if ilen<30 => { ibuf[ilen]=c as u8; ilen+=1; },
                        _ => {}
                    }
                }
            } else {
                match scr {
                    Scr::Splash => if my>330 && my<410 { scr=Scr::Menu; },
                    Scr::Menu => {
                        if my>90 && my < info.height-75 {
                            let mut idx=0; let mut y=100;
                            for (i,it) in items.iter().enumerate() {
                                if it.active {
                                    if my>=y && my<y+30 { sel=idx; qty[i]=qty[i].saturating_add(1); break; }
                                    idx+=1; y+=30;
                                }
                            }
                        }
                        if my > info.height-70 { scr=Scr::Cart; }
                        if mx > info.width-90 && my < 50 {
                            scr=Scr::Login; ilen=0; show_osk=true; login_err=false;
                        }
                    }
                    Scr::Cart => {
                        if mx < info.width/2 { scr=Scr::Menu; }
                        else { scr=Scr::Confirm; order+=1; qty=[0;MAX_ITEMS]; }
                    }
                    Scr::Confirm => scr=Scr::Menu,
                    Scr::Admin => {
                        if my>110 && my<150 { // edit
                            let mut idx=0;
                            for (i,it) in items.iter().enumerate() {
                                if it.active {
                                    if idx==sel {
                                        edit_i=i; ilen=0;
                                        for b in it.name().bytes() { if ilen<30 { ibuf[ilen]=b; ilen+=1; } }
                                        scr=Scr::Edit; show_osk=true; break;
                                    }
                                    idx+=1;
                                }
                            }
                        }
                        if my>160 && my<200 { // add
                            for it in items.iter_mut() {
                                if !it.active { it.set_name("New Item"); it.price=199; it.active=true; break; }
                            }
                        }
                        if my>210 && my<250 { // del
                            let mut idx=0;
                            for it in items.iter_mut() {
                                if it.active {
                                    if idx==sel { it.active=false; break; }
                                    idx+=1;
                                }
                            }
                        }
                        if my>260 && my<300 { scr=Scr::BrandName; ilen=0; show_osk=true; }
                        if my>310 && my<350 { scr=Scr::Colors; }
                        if my>360 && my<400 { scr=Scr::ChPass; ilen=0; show_osk=true; }
                        if my>410 && my<450 { scr=Scr::Menu; }
                    }
                    Scr::Colors => {
                        // click theme buttons
                        let mut ty = 120;
                        for (i,_) in THEMES.iter().enumerate() {
                            if my>=ty && my<ty+36 {
                                theme_i = i;
                                brand.primary = THEMES[i].0;
                                brand.accent  = THEMES[i].1;
                                brand.bg      = THEMES[i].2;
                                break;
                            }
                            ty += 40;
                        }
                        if my > info.height-80 { scr=Scr::Admin; }
                    }
                    _ => {}
                }
            }
        }

        if input_happened { dirty = true; }

        // Only redraw when something changed — this stops the blinking
        if !dirty {
            for _ in 0..80_000 { core::hint::spin_loop(); }
            continue;
        }
        dirty = false;

        // ---- DRAW ----
        clear(buffer, &info, brand.bg);

        // Header
        fill(buffer, &info, 0, 0, info.width, 46, brand.primary);
        fill(buffer, &info, 0, 46, info.width, 3, brand.accent);
        dtext2(buffer, &info, 16, 10, brand.name(), 0xFFFFFF, 2);
        dtext(buffer, &info, info.width-130, 16, "KIOSK", brand.accent);

        match scr {
            Scr::Splash => {
                dtext2(buffer, &info, 50, 140, "Welcome", brand.accent, 3);
                dtext(buffer, &info, 50, 210, "Click START or press Enter", 0xAAAAAA);
                dtext(buffer, &info, 50, 240, "F1 or top-right for Admin", 0x888888);
                fill(buffer, &info, info.width/2-100, 350, 200, 48, brand.primary);
                dtext(buffer, &info, info.width/2-55, 366, "START ORDER", 0xFFFFFF);
            }
            Scr::Menu => {
                dtext(buffer, &info, 20, 65, "MENU", brand.accent);
                let mut y = 95; let mut idx = 0;
                for (i,it) in items.iter().enumerate() {
                    if !it.active { continue; }
                    let bg = if idx==sel { 0xFF7000 } else { 0x1A1A1A };
                    fill(buffer, &info, 16, y, info.width-32, 28, bg);
                    dtext(buffer, &info, 28, y+8, it.name(), if idx==sel {brand.accent} else {0xFFFFFF});
                    let d=it.price/100; let c=it.price%100;
                    let mut px=info.width-80;
                    dchar(buffer,&info,px,y+8,'$',brand.accent); px+=9;
                    if d>=10 { dchar(buffer,&info,px,y+8,(b'0'+(d/10)as u8)as char,brand.accent); px+=9; }
                    dchar(buffer,&info,px,y+8,(b'0'+(d%10)as u8)as char,brand.accent); px+=9;
                    dchar(buffer,&info,px,y+8,'.',brand.accent); px+=9;
                    dchar(buffer,&info,px,y+8,(b'0'+(c/10)as u8)as char,brand.accent); px+=9;
                    dchar(buffer,&info,px,y+8,(b'0'+(c%10)as u8)as char,brand.accent);
                    y+=30; idx+=1;
                }
                fill(buffer, &info, 0, info.height-58, info.width, 58, brand.primary);
                fill(buffer, &info, 0, info.height-58, info.width, 3, brand.accent);
                let tot: u32 = qty.iter().enumerate().map(|(i,&q)| if items[i].active {q as u32} else {0}).sum();
                if tot==0 { dtext(buffer,&info,24,info.height-36,"Cart empty - click items",0xFFFFFF); }
                else { dtext(buffer,&info,24,info.height-36,"View Cart (click bar / Tab)",0xFFFFFF); }
            }
            Scr::Cart => {
                dtext2(buffer,&info,24,80,"Your Order",brand.accent,2);
                let mut y=140;
                for (i,it) in items.iter().enumerate() {
                    if qty[i]>0 && it.active {
                        dchar(buffer,&info,40,y,(b'0'+qty[i])as char,0xFFFFFF);
                        dtext(buffer,&info,60,y,"x",0xFFFFFF);
                        dtext(buffer,&info,80,y,it.name(),0xFFFFFF);
                        y+=28;
                    }
                }
                if y==140 { dtext(buffer,&info,40,160,"Cart is empty",0x888888); }
                fill(buffer,&info,30,info.height-90,150,44,0x222222);
                dtext(buffer,&info,70,info.height-75,"BACK",0xFFFFFF);
                fill(buffer,&info,info.width-230,info.height-90,200,44,0x00CC44);
                dtext(buffer,&info,info.width-190,info.height-75,"PLACE ORDER",0xFFFFFF);
            }
            Scr::Confirm => {
                dtext2(buffer,&info,50,140,"ORDER CONFIRMED",0x00CC44,2);
                dtext(buffer,&info,50,200,"Please pay at the counter",0xFFFFFF);
                dtext(buffer,&info,50,230,"Your number is",0xAAAAAA);
                let ds=[(b'0'+((order/1000)%10)as u8)as char,(b'0'+((order/100)%10)as u8)as char,
                        (b'0'+((order/10)%10)as u8)as char,(b'0'+(order%10)as u8)as char];
                let mut nx=50;
                for d in ds {
                    dtext2(buffer,&info,nx,280,core::str::from_utf8(&[d as u8]).unwrap_or("0"),brand.accent,4);
                    nx+=40;
                }
            }
            Scr::Login => {
                dtext2(buffer,&info,50,100,"ADMIN LOGIN",brand.accent,2);
                dtext(buffer,&info,50,160,"Password:",0xFFFFFF);
                fill(buffer,&info,50,190,280,30,0x1A1A1A);
                for i in 0..ilen { dchar(buffer,&info,60+i*12,198,'*',brand.accent); }
                if login_err { dtext(buffer,&info,50,240,"Wrong password",0x0000FF); }
                dtext(buffer,&info,50,280,"Default: admin",0x888888);
            }
            Scr::Admin => {
                dtext2(buffer,&info,30,70,"ADMIN",brand.accent,2);
                dtext(buffer,&info,30,120,"Edit Selected Item",0xFFFFFF);
                dtext(buffer,&info,30,170,"Add New Item",0xFFFFFF);
                dtext(buffer,&info,30,220,"Delete Selected Item",0xFFFFFF);
                dtext(buffer,&info,30,270,"Change Restaurant Name",0xFFFFFF);
                dtext(buffer,&info,30,320,"Change Colors / Theme",0xFFFFFF);
                dtext(buffer,&info,30,370,"Change Password",0xFFFFFF);
                dtext(buffer,&info,30,420,"Logout",0xAAAAAA);
            }
            Scr::Edit => {
                dtext2(buffer,&info,30,70,"EDIT ITEM NAME",brand.accent,2);
                fill(buffer,&info,30,140,400,30,0x1A1A1A);
                for i in 0..ilen { dchar(buffer,&info,40+i*9,148,ibuf[i]as char,brand.accent); }
                dtext(buffer,&info,30,200,"Type name then press OK",0x888888);
            }
            Scr::BrandName => {
                dtext2(buffer,&info,30,70,"RESTAURANT NAME",brand.accent,2);
                fill(buffer,&info,30,140,400,30,0x1A1A1A);
                for i in 0..ilen { dchar(buffer,&info,40+i*9,148,ibuf[i]as char,brand.accent); }
                dtext(buffer,&info,30,200,"Type name then press OK",0x888888);
            }
            Scr::Colors => {
                dtext2(buffer,&info,30,70,"COLOR THEMES",brand.accent,2);
                let mut ty = 120;
                for (i, t) in THEMES.iter().enumerate() {
                    let bg = if i==theme_i { 0xFF7000 } else { 0x1A1A1A };
                    fill(buffer,&info,30,ty,300,34,bg);
                    // color preview boxes
                    fill(buffer,&info,40,ty+6,20,20,t.0);
                    fill(buffer,&info,70,ty+6,20,20,t.1);
                    dtext(buffer,&info,110,ty+10,t.3,0xFFFFFF);
                    ty += 40;
                }
                dtext(buffer,&info,30,info.height-50,"Click a theme, then click bottom to go back",0x888888);
            }
            Scr::ChPass => {
                dtext2(buffer,&info,30,70,"NEW PASSWORD",brand.accent,2);
                fill(buffer,&info,30,150,280,30,0x1A1A1A);
                for i in 0..ilen { dchar(buffer,&info,40+i*12,158,'*',brand.accent); }
                dtext(buffer,&info,30,210,"Min 4 chars, then OK",0x888888);
            }
        }

        if show_osk {
            draw_osk(buffer, &info, info.height-165);
        }

        cursor(buffer, &info, mouse.x as usize, mouse.y as usize);

        // Long delay after draw so QEMU is stable
        for _ in 0..400_000 { core::hint::spin_loop(); }
    }
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! { loop {} }
