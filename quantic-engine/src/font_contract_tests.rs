use super::*;

const PRIMARY: &[u8] = include_bytes!("../tests/fixtures/fonts/primary.ttf");
const COMPLETE: &[u8] = include_bytes!("../tests/fixtures/fonts/complete.ttf");
const PARTIAL: &[u8] = include_bytes!("../tests/fixtures/fonts/partial.ttf");

fn fixture_fonts() -> FontSystem {
    let mut fonts = FontSystem::empty();
    assert_eq!(fonts.register_font_bytes("Primary", PRIMARY.to_vec()), 1);
    assert_eq!(fonts.register_font_bytes("Partial", PARTIAL.to_vec()), 1);
    assert_eq!(fonts.register_font_bytes("Complete", COMPLETE.to_vec()), 1);
    fonts
}

fn spec(families: &[&str]) -> FontSpec {
    FontSpec::new(
        &families
            .iter()
            .map(|family| family.to_string())
            .collect::<Vec<_>>(),
        400,
        false,
        20,
    )
}

#[test]
fn combining_accent_uses_one_complete_face_for_metrics_and_paint() {
    let fonts = fixture_fonts();
    let spec = spec(&["Primary", "Partial", "Complete"]);
    let text = "e\u{301}";
    assert_eq!(fonts.fallback_run_count(text, &spec), 1);
    assert_eq!(fonts.measure_text(text, &spec).width, 12);
    let raster = fonts
        .rasterize_text(text, &spec)
        .expect("real glyph raster");
    assert_eq!(raster.width, 12);
    assert!(raster
        .glyphs
        .iter()
        .any(|glyph| glyph.coverage.iter().any(|alpha| *alpha > 0)));
}

#[test]
fn emoji_joiner_sequence_stays_in_one_complete_face() {
    let fonts = fixture_fonts();
    let spec = spec(&["Primary", "Complete"]);
    let text = "\u{1f469}\u{200d}\u{1f4bb}";
    assert_eq!(fonts.fallback_run_count(text, &spec), 1);
    assert_eq!(fonts.measure_text(text, &spec).width, 24);
    assert_eq!(fonts.rasterize_text(text, &spec).unwrap().width, 24);
}

#[test]
fn regional_indicator_pair_stays_in_one_complete_face() {
    let fonts = fixture_fonts();
    let spec = spec(&["Primary", "Complete"]);
    let text = "\u{1f1eb}\u{1f1f7}";
    assert_eq!(fonts.fallback_run_count(text, &spec), 1);
    assert_eq!(fonts.measure_text(text, &spec).width, 24);
}

#[test]
fn emoji_modifier_stays_with_its_base() {
    let fonts = fixture_fonts();
    let spec = spec(&["Primary", "Complete"]);
    let text = "\u{1f44b}\u{1f3fb}";
    assert_eq!(fonts.fallback_run_count(text, &spec), 1);
    assert_eq!(fonts.measure_text(text, &spec).width, 24);
}

#[test]
fn indic_spacing_mark_stays_with_its_base() {
    let fonts = fixture_fonts();
    let spec = spec(&["Primary", "Complete"]);
    let text = "\u{915}\u{93f}";
    assert_eq!(fonts.fallback_run_count(text, &spec), 1);
    assert_eq!(fonts.measure_text(text, &spec).width, 12);
}

#[test]
fn missing_complete_face_keeps_the_cluster_intact() {
    let mut fonts = FontSystem::empty();
    assert_eq!(fonts.register_font_bytes("Primary", PRIMARY.to_vec()), 1);
    assert_eq!(fonts.register_font_bytes("Partial", PARTIAL.to_vec()), 1);
    let spec = spec(&["Primary", "Partial"]);
    let text = "e\u{301}";
    assert_eq!(fonts.fallback_run_count(text, &spec), 1);
    let runs = fonts.plan_runs(text, &spec);
    assert_eq!(runs[0].text, text);
}

#[test]
fn variation_selectors_do_not_force_a_face_switch() {
    let fonts = fixture_fonts();
    let spec = spec(&["Primary", "Complete"]);
    let text = "\u{2708}\u{fe0f}";
    assert_eq!(fonts.fallback_run_count(text, &spec), 1);
    assert_eq!(fonts.measure_text(text, &spec).width, 6);
}

#[test]
fn actual_family_precedes_a_later_web_font_alias() {
    let fonts = fixture_fonts();
    let spec = spec(&["Quantic Fixture Primary", "Complete"]);
    assert_eq!(fonts.measure_text("e", &spec).width, 6);
}

#[test]
fn adjacent_clusters_share_a_run_without_losing_text() {
    let fonts = fixture_fonts();
    let spec = spec(&["Primary", "Complete"]);
    let text = "e\u{301}e\u{308}";
    assert_eq!(fonts.fallback_run_count(text, &spec), 1);
    assert_eq!(fonts.measure_text(text, &spec).width, 24);
    let runs = fonts.plan_runs(text, &spec);
    assert_eq!(runs[0].text, text);
}

#[test]
fn web_font_aliases_remain_instance_local() {
    let mut first = FontSystem::empty();
    let mut second = FontSystem::empty();
    assert_eq!(first.register_font_bytes("PageFace", PRIMARY.to_vec()), 1);
    assert!(!second.has_alias("PageFace"));
    assert_eq!(second.register_font_bytes("PageFace", COMPLETE.to_vec()), 1);
    let spec = spec(&["PageFace"]);
    assert_eq!(first.measure_text("e", &spec).width, 6);
    assert_eq!(second.measure_text("e", &spec).width, 12);
}

#[test]
fn woff1_registration_uses_a_deterministic_fixture() {
    let bytes = oxifont_webfont::encode_woff1(PRIMARY).expect("fixture WOFF1 encoding");
    let mut fonts = FontSystem::empty();
    assert_eq!(fonts.register_font_bytes("Compressed", bytes), 1);
    assert_eq!(fonts.measure_text("e", &spec(&["Compressed"])).width, 6);
}

#[test]
fn woff2_registration_uses_a_deterministic_fixture() {
    let bytes = oxifont_webfont::encode_woff2(PRIMARY).expect("fixture WOFF2 encoding");
    let mut fonts = FontSystem::empty();
    assert_eq!(fonts.register_font_bytes("Compressed", bytes), 1);
    assert_eq!(fonts.measure_text("e", &spec(&["Compressed"])).width, 6);
}
