use std::sync::OnceLock;

use skia_safe::{FontMgr, FontStyle, Typeface};

pub fn default_typeface() -> Typeface {
    DEFAULT_TYPEFACE
        .get_or_init(|| {
            let font_mgr = FontMgr::new();
            for family in font_mgr.family_names() {
                println!("Family: {}", family);
            }

            font_mgr
                .legacy_make_typeface("Noto Sans CJK JP", FontStyle::default())
                .unwrap()
        })
        .clone()
}

static DEFAULT_TYPEFACE: OnceLock<Typeface> = OnceLock::new();
