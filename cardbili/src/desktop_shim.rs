// macOS 预览窗口的鼠标兼容层：把底栏槽位点击翻译成数字键，业务层只接收键盘事件

const BAR_HEIGHT: f32 = 20.0;
const SLOT_HALF_WIDTH: f32 = 27.0;

struct Slot {
    center: f32,
    key: &'static str,
}

const BOTTOM_SLOTS: [Slot; 5] = [
    Slot { center: 49.0, key: "4" },
    Slot { center: 106.0, key: "5" },
    Slot { center: 162.0, key: "6" },
    Slot { center: 217.0, key: "7" },
    Slot { center: 275.0, key: "8" },
];

/// 窗口逻辑坐标命中底栏槽位时返回对应数字键
pub fn bottom_bar_key(x: f32, y: f32, view_height: f32) -> Option<&'static str> {
    if y < view_height - BAR_HEIGHT {
        return None;
    }
    BOTTOM_SLOTS
        .iter()
        .find(|slot| (x - slot.center).abs() <= SLOT_HALF_WIDTH)
        .map(|slot| slot.key)
}
