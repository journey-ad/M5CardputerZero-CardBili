use std::fs::File;
use std::io::Read;
use std::sync::mpsc;
use std::thread;

use slint::Rgb8Pixel;

pub fn code_name(code: u16) -> Option<&'static str> {
    Some(match code {
        103 => "UP", 108 => "DOWN", 105 => "LEFT", 106 => "RIGHT",
        104 => "PGUP", 109 => "PGDN",
        28 => "ENTER", 57 => "SPACE", 1 => "ESC", 14 => "BACKSPACE", 15 => "TAB",
        16 => "q", 17 => "w", 18 => "e", 19 => "r", 20 => "t", 21 => "y", 22 => "u",
        23 => "i", 24 => "o", 25 => "p", 30 => "a", 31 => "s", 32 => "d", 33 => "f",
        34 => "g", 35 => "h", 36 => "j", 37 => "k", 38 => "l", 44 => "z", 45 => "x",
        46 => "c", 47 => "v", 48 => "b", 49 => "n", 50 => "m",
        2 => "1", 3 => "2", 4 => "3", 5 => "4", 6 => "5", 7 => "6", 8 => "7", 9 => "8", 10 => "9", 11 => "0",
        26 => "1", 27 => "2", 39 => "3", 40 => "4", 41 => "5", 43 => "6", 51 => "7", 52 => "8", 53 => "9", 94 => "0",
        _ => return None,
    })
}

pub fn spawn_keys(path: String, tx: mpsc::Sender<String>) {
    thread::spawn(move || {
        let mut f = match File::open(&path) { Ok(f) => f, Err(_) => return };
        let mut buf = [0u8; 24];
        let mut esc_down: Option<std::time::Instant> = None;
        loop {
            if f.read_exact(&mut buf).is_err() { break; }
            let ty = u16::from_le_bytes([buf[16], buf[17]]);
            let code = u16::from_le_bytes([buf[18], buf[19]]);
            let value = i32::from_le_bytes([buf[20], buf[21], buf[22], buf[23]]);
            if ty != 1 { continue; }
            // ESC 单独处理：短按返回，长按退出
            if code == 1 {
                if value == 1 {
                    esc_down = Some(std::time::Instant::now());
                    if tx.send("ESC_DOWN".to_string()).is_err() { break; }
                } else if value == 0 {
                    let held = esc_down.take().map(|t| t.elapsed().as_millis()).unwrap_or(0);
                    if tx.send("ESC_UP".to_string()).is_err() { break; }
                    let name = if held >= 2000 { "ESC_HOLD" } else { "ESC" };
                    if tx.send(name.to_string()).is_err() { break; }
                }
                continue;
            }
            if value == 1 {
                if let Some(n) = code_name(code) {
                    if tx.send(n.to_string()).is_err() { break; }
                }
            }
        }
    });
}

pub fn raw_to_image(w: u32, h: u32, raw: &[u8]) -> slint::Image {
    let mut pb = slint::SharedPixelBuffer::<Rgb8Pixel>::new(w, h);
    {
        let sl = pb.make_mut_slice();
        if sl.len() >= (w * h) as usize {
            for (dst, src) in sl.iter_mut().zip(raw.chunks_exact(3)) {
                *dst = Rgb8Pixel { r: src[0], g: src[1], b: src[2] };
            }
        }
    }
    slint::Image::from_rgb8(pb)
}

pub fn make_placeholder() -> slint::Image {
    let mut pb = slint::SharedPixelBuffer::<Rgb8Pixel>::new(2, 2);
    for p in pb.make_mut_slice() { *p = Rgb8Pixel { r: 26, g: 33, b: 46 }; }
    slint::Image::from_rgb8(pb)
}

pub fn decode_avatar(bytes: &[u8]) -> Option<slint::Image> {
    let img = image::load_from_memory(bytes).ok()?.thumbnail(64, 64).to_rgb8();
    let (w, h) = img.dimensions();
    Some(raw_to_image(w, h, img.as_raw()))
}

pub fn fetch_cover(pic: &str) -> Option<(u32, u32, Vec<u8>)> {
    crate::cache::cover(pic)
}

pub fn make_qr(url: &str) -> Option<slint::Image> {
    use qrcodegen::{QrCode, QrCodeEcc};
    let code = QrCode::encode_text(url, QrCodeEcc::Low).ok()?;
    let n = code.size();
    let quiet = 4usize;
    let total = n as usize + 2 * quiet;
    let scale = (118usize / total).max(1);
    let dim = (total * scale) as u32;
    let mut pb = slint::SharedPixelBuffer::<Rgb8Pixel>::new(dim, dim);
    {
        let sl = pb.make_mut_slice();
        for p in sl.iter_mut() { *p = Rgb8Pixel { r: 255, g: 255, b: 255 }; }
        for y in 0..n {
            for x in 0..n {
                if code.get_module(x, y) {
                    for dy in 0..scale {
                        for dx in 0..scale {
                            let px = (x as usize + quiet) * scale + dx;
                            let py = (y as usize + quiet) * scale + dy;
                            if px < dim as usize && py < dim as usize {
                                sl[py * dim as usize + px] = Rgb8Pixel { r: 0, g: 0, b: 0 };
                            }
                        }
                    }
                }
            }
        }
    }
    Some(slint::Image::from_rgb8(pb))
}

pub fn parse_mid(cookies: &str) -> String {
    for kv in cookies.split(';') {
        let kv = kv.trim();
        if let Some(v) = kv.strip_prefix("DedeUserID=") {
            if !v.is_empty() { return v.to_string(); }
        }
    }
    String::new()
}

pub fn cookie_path() -> String {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    format!("{}/.config/cardbili/cookie.txt", home)
}

/// 估算 zpix 字体下一行文字的像素宽度（CJK 约等于字号，ASCII 约 0.55 字号）。
pub fn est_w(s: &str, fs: f64) -> f64 {
    s.chars().map(|c| if (c as u32) < 128 { fs * 0.55 } else { fs }).sum()
}
