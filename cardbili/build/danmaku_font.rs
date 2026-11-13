use std::fmt::Write;
use std::path::PathBuf;

pub fn generate() {
    let source = "fonts/fusion12.ttf";
    println!("cargo:rerun-if-changed={source}");
    let font = fontdue::Font::from_bytes(
        std::fs::read(source).expect("读取 Fusion 12 字体失败"),
        fontdue::FontSettings::default(),
    )
    .expect("解析 Fusion 12 字体失败");
    let mut characters: Vec<char> = font.chars().keys().copied().collect();
    characters.sort_unstable();

    let mut pixels = Vec::new();
    let mut code = String::from("static GLYPHS: &[Glyph] = &[\n");
    for character in characters {
        let (metrics, bitmap) = font.rasterize(character, 12.0);
        let glyph = encode(character as u32, metrics, &bitmap, &mut pixels);
        writeln!(code, "{glyph},").unwrap();
    }
    code.push_str("];\n");
    let (metrics, bitmap) = font.rasterize_indexed(0, 12.0);
    let missing = encode(0, metrics, &bitmap, &mut pixels);
    writeln!(code, "static MISSING_GLYPH: Glyph = {missing};").unwrap();

    let output = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    std::fs::write(output.join("danmaku_font.rs"), code).unwrap();
    std::fs::write(output.join("danmaku_font.bin"), pixels).unwrap();
}

fn encode(code: u32, metrics: fontdue::Metrics, bitmap: &[u8], pixels: &mut Vec<u8>) -> String {
    let offset = pixels.len();
    // Fusion 12 字形按 1 位覆盖率保存，每个字形从完整字节开始
    for chunk in bitmap.chunks(8) {
        let mut byte = 0;
        for (bit, &coverage) in chunk.iter().enumerate() {
            if coverage >= 128 {
                byte |= 1 << bit;
            }
        }
        pixels.push(byte);
    }
    let x = metrics.xmin;
    let y = metrics.ymin;
    let width = metrics.width;
    let height = metrics.height;
    let advance = metrics.advance_width.round() as u16;
    format!("Glyph {{ code: {code}, offset: {offset}, x: {x}, y: {y}, width: {width}, height: {height}, advance: {advance} }}")
}
