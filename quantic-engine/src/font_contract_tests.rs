use super::*;

fn system_spec() -> FontSpec {
    FontSpec::new(&["sans-serif".to_string()], 400, false, 20)
}

#[test]
fn system_font_database_is_available() {
    let fonts = FontSystem::system();
    assert!(fonts.face_count() > 0);
}

#[test]
fn system_font_measures_real_ascii_text() {
    let fonts = FontSystem::system();
    let metrics = fonts.measure_text("Gekko Browser", &system_spec());
    assert!(metrics.width > 0);
    assert!(metrics.line_height >= 20);
    assert!(metrics.ascent > 0);
}

#[test]
fn system_font_rasterization_produces_visible_glyphs() {
    let fonts = FontSystem::system();
    let raster = fonts
        .rasterize_text("Gekko", &system_spec())
        .expect("system font rasterization");
    assert!(raster.width > 0);
    assert!(raster.glyphs.iter().any(|glyph| {
        glyph.coverage.iter().any(|alpha| *alpha > 0)
    }));
}

#[test]
fn empty_database_uses_stable_fallback_metrics() {
    let fonts = FontSystem::empty();
    let spec = system_spec();
    let first = fonts.measure_text("fallback", &spec);
    let second = fonts.measure_text("fallback", &spec);
    assert_eq!(first, second);
    assert!(first.width > 0);
    assert!(first.line_height > 0);
}

#[test]
fn invalid_webfont_bytes_do_not_create_aliases() {
    let mut fonts = FontSystem::empty();
    assert_eq!(fonts.register_font_bytes("BrokenFace", vec![0, 1, 2, 3, 4]), 0);
    assert!(!fonts.has_alias("BrokenFace"));
}

#[test]
fn font_spec_clamps_invalid_weight_and_size() {
    let low = FontSpec::new(&[], 0, false, 0);
    let high = FontSpec::new(&[], 5000, false, 20);
    assert_eq!(low.weight, 1);
    assert_eq!(low.size_px, 1);
    assert_eq!(high.weight, 1000);
    assert_eq!(low.families, vec!["sans-serif".to_string()]);
}

#[test]
fn adjacent_text_keeps_nonzero_layout_metrics() {
    let fonts = FontSystem::system();
    let spec = system_spec();
    let a = fonts.measure_text("A", &spec);
    let b = fonts.measure_text("B", &spec);
    let ab = fonts.measure_text("AB", &spec);
    assert!(ab.width >= a.width.min(b.width));
    assert!(ab.line_height >= a.line_height.min(b.line_height));
}
