#[path = "build/danmaku_font.rs"]
mod danmaku_font;
#[path = "build/font_cache.rs"]
mod font_cache;
#[path = "build/ui_fonts.rs"]
mod ui_fonts;

fn main() {
    font_cache::generate(
        "danmaku",
        &["fonts/fusion12.ttf", "build/danmaku_font.rs"],
        &["danmaku_font.rs", "danmaku_font.bin"],
        danmaku_font::generate,
    );
    font_cache::generate(
        "ui_fonts",
        &[
            "fonts/fusion8.ttf",
            "fonts/fusion10.ttf",
            "fonts/fusion12.ttf",
            "build/ui_fonts.rs",
        ],
        &["ui_fonts.rs", "ui_fonts.bin"],
        ui_fonts::generate,
    );
    // 图标在构建时栅格化，完整中文位图字体由 fonts::register 注册
    let config = slint_build::CompilerConfiguration::new()
        .embed_resources(slint_build::EmbedResourcesKind::EmbedForSoftwareRenderer);
    slint_build::compile_with_config("ui/app.slint", config).unwrap();
}
