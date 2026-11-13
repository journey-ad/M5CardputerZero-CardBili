/// 计算跑马灯在起点与终点之间的往返位移
pub(in crate::app) fn yoyo_shift(est: f64, clip: f64, t: f64) -> f32 {
    let d = est - clip;
    if d <= 1.0 {
        return 0.0;
    }
    let mt = (d / 40.0).max(0.8);
    let pause = 2.5;
    let per = (mt + pause) * 2.0;
    let ph = t % per;
    if ph < mt {
        (d * ph / mt) as f32
    } else if ph < mt + pause {
        d as f32
    } else if ph < mt * 2.0 + pause {
        (d * (1.0 - (ph - mt - pause) / mt)) as f32
    } else {
        0.0
    }
}
