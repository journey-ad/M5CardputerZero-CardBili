use super::*;
#[cfg(not(feature = "desktop"))]
use memmap2::MmapOptions;
use slint::platform::software_renderer::{MinimalSoftwareWindow, RepaintBufferType};
use slint::platform::{Platform, WindowAdapter};
use slint::PhysicalSize;
#[cfg(not(feature = "desktop"))]
use std::fs::OpenOptions;
use std::rc::Rc;

const FB_BYTES: usize = (WIDTH as usize) * (HEIGHT as usize) * 2;

struct AppPlatform {
    window: Rc<MinimalSoftwareWindow>,
}
impl Platform for AppPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        Ok(self.window.clone())
    }

    fn click_interval(&self) -> Duration {
        // 界面使用单击和长按，每次按下独立处理
        Duration::ZERO
    }
}

/// 本机预览：把窗口按键映射成与真机 evdev 一致的按键字符串
#[cfg(feature = "desktop")]
fn desktop_key(k: minifb::Key) -> Option<&'static str> {
    use minifb::Key;
    Some(match k {
        Key::Left => "LEFT",
        Key::Right => "RIGHT",
        Key::Up => "UP",
        Key::Down => "DOWN",
        Key::PageUp => "PGUP",
        Key::PageDown => "PGDN",
        Key::Enter => "ENTER",
        Key::Space => "SPACE",
        Key::Escape => "ESC",
        Key::Backspace => "BACKSPACE",
        Key::Tab => "TAB",
        Key::Key1 => "1",
        Key::Key2 => "2",
        Key::Key3 => "3",
        Key::Key4 => "4",
        Key::Key5 => "5",
        Key::Key6 => "6",
        Key::Key7 => "7",
        Key::Z => "z",
        Key::C => "c",
        Key::F => "f",
        Key::W => "w",
        Key::X => "x",
        Key::S => "s",
        Key::Q => "q",
        Key::V => "v",
        Key::B => "b",
        Key::D => "d",
        Key::R => "r",
        Key::L => "l",
        Key::H => "h",
        Key::U => "u",
        Key::Key8 => "8",
        _ => return None,
    })
}

pub(super) struct Display {
    window: Rc<MinimalSoftwareWindow>,
    buffer: Vec<slint::platform::software_renderer::Rgb565Pixel>,
    #[cfg(not(feature = "desktop"))]
    fbmap: memmap2::MmapMut,
    #[cfg(feature = "desktop")]
    win: minifb::Window,
    #[cfg(feature = "desktop")]
    dbuf: Vec<u32>,
    #[cfg(feature = "desktop")]
    ktx: mpsc::Sender<String>,
    #[cfg(feature = "desktop")]
    scripted: Vec<String>,
    #[cfg(feature = "desktop")]
    script_idx: usize,
    #[cfg(feature = "desktop")]
    script_next: Instant,
    #[cfg(feature = "desktop")]
    prev_keys: HashSet<minifb::Key>,
    #[cfg(feature = "desktop")]
    pointer_down: bool,
    #[cfg(feature = "desktop")]
    pointer_position: Option<(f32, f32)>,
    #[cfg(feature = "desktop")]
    hold_key: Option<String>,
    #[cfg(feature = "desktop")]
    hold_ms: u64,
    #[cfg(feature = "desktop")]
    hold_started: bool,
    #[cfg(feature = "desktop")]
    hold_release: Option<Instant>,
}
impl Display {
    pub(super) fn new(ktx: mpsc::Sender<String>) -> Self {
        #[cfg(not(feature = "desktop"))]
        let fb_path = std::env::var("APPLAUNCH_LINUX_FBDEV_DEVICE")
            .or_else(|_| std::env::var("BILI_FB"))
            .unwrap_or_else(|_| "/dev/fb0".into());
        #[cfg(not(feature = "desktop"))]
        let kb_path = std::env::var("APPLAUNCH_LINUX_KEYBOARD_DEVICE")
            .or_else(|_| std::env::var("BILI_KBD"))
            .unwrap_or_else(|_| "/dev/input/by-path/platform-3f804000.i2c-event".into());

        let window = MinimalSoftwareWindow::new(RepaintBufferType::ReusedBuffer);
        window.set_size(PhysicalSize::new(WIDTH, HEIGHT));
        crate::fonts::register(window.as_ref());
        slint::platform::set_platform(Box::new(AppPlatform {
            window: window.clone(),
        }))
        .expect("set_platform");

        #[cfg(not(feature = "desktop"))]
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&fb_path)
            .expect("open fb");
        #[cfg(not(feature = "desktop"))]
        let fbmap = unsafe {
            MmapOptions::new()
                .len(FB_BYTES)
                .map_mut(&file)
                .expect("mmap fb")
        };

        #[cfg(feature = "desktop")]
        let win = minifb::Window::new(
            "CardBili 320x170",
            WIDTH as usize,
            HEIGHT as usize,
            minifb::WindowOptions {
                scale: minifb::Scale::X2,
                resize: false,
                ..Default::default()
            },
        )
        .expect("创建预览窗口失败");
        #[cfg(feature = "desktop")]
        let dbuf: Vec<u32> = vec![0; (WIDTH * HEIGHT) as usize];

        #[cfg(not(feature = "desktop"))]
        util::spawn_keys(kb_path, ktx);
        // 本机预览可用的按键脚本：BILI_KEYS="TAB,RIGHT,ENTER" 按 700ms 间隔依次注入
        #[cfg(feature = "desktop")]
        let scripted: Vec<String> = std::env::var("BILI_KEYS")
            .ok()
            .map(|s| {
                s.split(',')
                    .map(|x| x.trim().to_string())
                    .filter(|x| !x.is_empty())
                    .collect()
            })
            .unwrap_or_default();
        #[cfg(feature = "desktop")]
        let script_idx = 0usize;
        #[cfg(feature = "desktop")]
        let script_next = Instant::now() + Duration::from_millis(900);
        #[cfg(feature = "desktop")]
        let prev_keys: std::collections::HashSet<minifb::Key> = std::collections::HashSet::new();
        // 长按测试入口：BILI_HOLD=RIGHT，按下后保持 BILI_HOLD_MS 毫秒再松开
        #[cfg(feature = "desktop")]
        let hold_key = std::env::var("BILI_HOLD").ok();
        #[cfg(feature = "desktop")]
        let hold_ms = std::env::var("BILI_HOLD_MS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(1500);
        #[cfg(feature = "desktop")]
        let hold_started = false;
        #[cfg(feature = "desktop")]
        let hold_release: Option<Instant> = None;

        let buffer = vec![slint::platform::software_renderer::Rgb565Pixel(0); FB_BYTES / 2];
        Self {
            window,
            buffer,
            #[cfg(not(feature = "desktop"))]
            fbmap,
            #[cfg(feature = "desktop")]
            win,
            #[cfg(feature = "desktop")]
            dbuf,
            #[cfg(feature = "desktop")]
            ktx,
            #[cfg(feature = "desktop")]
            scripted,
            #[cfg(feature = "desktop")]
            script_idx,
            #[cfg(feature = "desktop")]
            script_next,
            #[cfg(feature = "desktop")]
            prev_keys,
            #[cfg(feature = "desktop")]
            pointer_down: false,
            #[cfg(feature = "desktop")]
            pointer_position: None,
            #[cfg(feature = "desktop")]
            hold_key,
            #[cfg(feature = "desktop")]
            hold_ms,
            #[cfg(feature = "desktop")]
            hold_started,
            #[cfg(feature = "desktop")]
            hold_release,
        }
    }
    pub(super) fn poll_keys(&mut self) {
        #[cfg(feature = "desktop")]
        if let Some((x, y)) = self.win.get_mouse_pos(minifb::MouseMode::Clamp) {
            use slint::platform::{PointerEventButton, WindowEvent};
            let position = slint::LogicalPosition::new(x, y);
            if self.pointer_position != Some((x, y)) {
                self.pointer_position = Some((x, y));
                let _ = self.ktx.send("POINTER".into());
                self.window
                    .window()
                    .dispatch_event(WindowEvent::PointerMoved { position });
            }
            let down = self.win.get_mouse_down(minifb::MouseButton::Left);
            if down != self.pointer_down {
                let _ = self.ktx.send("POINTER".into());
                let event = if down {
                    WindowEvent::PointerPressed {
                        position,
                        button: PointerEventButton::Left,
                    }
                } else {
                    WindowEvent::PointerReleased {
                        position,
                        button: PointerEventButton::Left,
                    }
                };
                self.window.window().dispatch_event(event);
                self.pointer_down = down;
            }
        }

        #[cfg(feature = "desktop")]
        if self.script_idx < self.scripted.len() && Instant::now() >= self.script_next {
            let _ = self.ktx.send(self.scripted[self.script_idx].clone());
            self.script_idx += 1;
            self.script_next = Instant::now() + Duration::from_millis(700);
        }
        #[cfg(feature = "desktop")]
        if let Some(hk) = &self.hold_key {
            if !self.hold_started && self.scripted.is_empty() && Instant::now() >= self.script_next
            {
                let _ = self.ktx.send(hk.clone());
                self.hold_started = true;
                self.hold_release = Some(Instant::now() + Duration::from_millis(self.hold_ms));
            }
        }
        #[cfg(feature = "desktop")]
        if let Some(rt) = self.hold_release {
            if Instant::now() >= rt {
                if let Some(hk) = &self.hold_key {
                    let _ = self.ktx.send(format!("{}_UP", hk));
                }
                self.hold_release = None;
            }
        }
        #[cfg(feature = "desktop")]
        {
            let cur: std::collections::HashSet<minifb::Key> =
                self.win.get_keys().into_iter().collect();
            for key in cur.iter() {
                if !self.prev_keys.contains(key) {
                    if let Some(s) = desktop_key(*key) {
                        let _ = self.ktx.send(s.to_string());
                    }
                }
            }
            for key in self.prev_keys.iter() {
                if !cur.contains(key) {
                    if let Some(release) = desktop_key(*key).and_then(util::key_release) {
                        let _ = self.ktx.send(release.to_string());
                    }
                }
            }
            self.prev_keys = cur;
        }
    }
    pub(super) fn render(&mut self) -> bool {
        // --- 渲染 ---
        slint::platform::update_timers_and_animations();
        let redrawn = self.window.draw_if_needed(|r| {
            let _ = r.render(&mut self.buffer, WIDTH as usize);
        });
        if redrawn {
            #[cfg(not(feature = "desktop"))]
            {
                let out = unsafe {
                    std::slice::from_raw_parts(self.buffer.as_ptr() as *const u8, FB_BYTES)
                };
                self.fbmap[..FB_BYTES].copy_from_slice(out);
            }
            #[cfg(feature = "desktop")]
            {
                for (i, px) in self.buffer.iter().enumerate() {
                    let v = px.0;
                    let r = ((v >> 11) & 0x1f) as u32 * 255 / 31;
                    let g = ((v >> 5) & 0x3f) as u32 * 255 / 63;
                    let b = (v & 0x1f) as u32 * 255 / 31;
                    self.dbuf[i] = (r << 16) | (g << 8) | b;
                }
                if self
                    .win
                    .update_with_buffer(&self.dbuf, WIDTH as usize, HEIGHT as usize)
                    .is_err()
                {
                    return false;
                }
            }
        }

        #[cfg(feature = "desktop")]
        if !redrawn {
            self.win.update();
        }
        #[cfg(feature = "desktop")]
        {
            self.win.is_open()
        }
        #[cfg(not(feature = "desktop"))]
        {
            true
        }
    }
}
