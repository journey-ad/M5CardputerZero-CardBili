#[path = "build/danmaku_font.rs"]
mod danmaku_font;
#[path = "build/emoji_font.rs"]
mod emoji_font;
#[path = "build/font_cache.rs"]
mod font_cache;
#[path = "build/ui_fonts.rs"]
mod ui_fonts;

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux")
        && std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("aarch64")
    {
        let sdk = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("lib");
        println!("cargo:rustc-link-search=native={}", sdk.display());
        println!("cargo:rustc-link-arg=-Wl,-rpath,$ORIGIN");
        if std::env::var("PROFILE").as_deref() != Ok("release") {
            println!("cargo:rustc-link-arg=-Wl,-rpath,{}", sdk.display());
        }
    }
    font_cache::generate(
        "emoji",
        &[
            "fonts/Mona12Emoji.ttf",
            "fonts/Mona12ColorEmoji.ttf",
            "build/emoji_font.rs",
        ],
        &["emoji_font.rs", "emoji_color.bin", "emoji_mono.bin"],
        emoji_font::generate,
    );
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
