use std::{
    collections::HashMap,
    fmt,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, OnceLock,
    },
};

use fontdb::{Database, Family, Query, Source, Style as DbStyle, Weight, ID};
use fontdue::{Font, FontSettings};
use oxifont_webfont::{decode_auto, detect_format, FontFormat};
use rustybuzz::{shape, Face as BuzzFace, UnicodeBuffer};
use unicode_segmentation::UnicodeSegmentation;

static SYSTEM_FONT_DATABASE: OnceLock<Database> = OnceLock::new();
static SYSTEM_FONT_SCAN_COUNT: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontSpec {
    pub families: Vec<String>,
    pub weight: u16,
    pub italic: bool,
    pub size_px: u32,
}

impl FontSpec {
    pub fn new(families: &[String], weight: u16, italic: bool, size_px: u32) -> Self {
        Self {
            families: if families.is_empty() {
                vec!["sans-serif".to_string()]
            } else {
                families.to_vec()
            },
            weight: weight.clamp(1, 1000),
            italic,
            size_px: size_px.max(1),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextMetrics {
    pub width: u32,
    pub line_height: u32,
    pub ascent: u32,
}

#[derive(Debug, Clone)]
pub struct RasterGlyph {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub coverage: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct RasterText {
    pub width: u32,
    pub line_height: u32,
    pub ascent: u32,
    pub glyphs: Vec<RasterGlyph>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebFontFace {
    pub family: String,
    pub sources: Vec<String>,
    pub weight: u16,
    pub italic: bool,
}

#[derive(Debug, Clone)]
struct FontRun {
    face_id: ID,
    text: String,
}

pub struct FontSystem {
    db: Database,
    aliases: HashMap<String, Vec<ID>>,
}

impl fmt::Debug for FontSystem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FontSystem")
            .field("faces", &self.db.len())
            .field("aliases", &self.aliases.len())
            .finish()
    }
}

impl Default for FontSystem {
    fn default() -> Self {
        Self::system()
    }
}

impl FontSystem {
    pub fn empty() -> Self {
        Self {
            db: Database::new(),
            aliases: HashMap::new(),
        }
    }

    pub fn system() -> Self {
        Self {
            db: cached_system_database().clone(),
            aliases: HashMap::new(),
        }
    }

    pub fn system_scan_count() -> usize {
        SYSTEM_FONT_SCAN_COUNT.load(Ordering::Relaxed)
    }

    pub fn face_count(&self) -> usize {
        self.db.len()
    }

    pub fn has_alias(&self, family: &str) -> bool {
        self.aliases
            .get(&normalize_family(family))
            .is_some_and(|ids| !ids.is_empty())
    }

    pub fn register_font_bytes(&mut self, family_alias: &str, bytes: Vec<u8>) -> usize {
        let bytes = normalize_web_font_bytes(bytes);
        let data: Arc<dyn AsRef<[u8]> + Send + Sync> = Arc::new(bytes);
        let ids = self.db.load_font_source(Source::Binary(data));
        let count = ids.len();
        if count > 0 {
            self.aliases
                .entry(normalize_family(family_alias))
                .or_default()
                .extend(ids.iter().copied());
        }
        count
    }

    pub fn measure_text(&self, text: &str, spec: &FontSpec) -> TextMetrics {
        let runs = self.plan_runs(text, spec);
        if runs.is_empty() {
            return fallback_metrics(text, spec.size_px);
        }

        let mut width = 0u32;
        let mut line_height = 0u32;
        let mut ascent = 0u32;
        let mut measured_any = false;

        for run in runs {
            let metrics = self
                .db
                .with_face_data(run.face_id, |data, index| {
                    shape_metrics(data, index, &run.text, spec)
                })
                .flatten();

            if let Some(metrics) = metrics {
                measured_any = true;
                width = width.saturating_add(metrics.width);
                line_height = line_height.max(metrics.line_height);
                ascent = ascent.max(metrics.ascent);
            } else {
                let fallback = fallback_metrics(&run.text, spec.size_px);
                width = width.saturating_add(fallback.width);
                line_height = line_height.max(fallback.line_height);
                ascent = ascent.max(fallback.ascent);
            }
        }

        if measured_any {
            TextMetrics {
                width,
                line_height: line_height.max(fallback_line_height(spec.size_px)),
                ascent: ascent.max(1),
            }
        } else {
            fallback_metrics(text, spec.size_px)
        }
    }

    pub fn rasterize_text(&self, text: &str, spec: &FontSpec) -> Option<RasterText> {
        let runs = self.plan_runs(text, spec);
        if runs.is_empty() {
            return None;
        }

        let mut shaped_runs = Vec::with_capacity(runs.len());
        let mut ascent = 0u32;
        let mut line_height = 0u32;

        for run in runs {
            let raster = self
                .db
                .with_face_data(run.face_id, |data, index| {
                    rasterize_shaped_text(data, index, &run.text, spec)
                })
                .flatten()?;
            ascent = ascent.max(raster.ascent);
            line_height = line_height.max(raster.line_height);
            shaped_runs.push(raster);
        }

        let mut pen_x = 0i32;
        let mut glyphs = Vec::new();
        for mut run in shaped_runs {
            let baseline_shift = ascent.saturating_sub(run.ascent) as i32;
            for glyph in &mut run.glyphs {
                glyph.x = glyph.x.saturating_add(pen_x);
                glyph.y = glyph.y.saturating_add(baseline_shift);
            }
            pen_x = pen_x.saturating_add(run.width as i32);
            glyphs.extend(run.glyphs);
        }

        Some(RasterText {
            width: pen_x.max(0) as u32,
            line_height: line_height.max(fallback_line_height(spec.size_px)),
            ascent: ascent.max(1),
            glyphs,
        })
    }

    pub fn fallback_run_count(&self, text: &str, spec: &FontSpec) -> usize {
        self.plan_runs(text, spec).len()
    }

    fn plan_runs(&self, text: &str, spec: &FontSpec) -> Vec<FontRun> {
        let Some(primary) = self.resolve_id(spec) else {
            return Vec::new();
        };

        let mut runs = Vec::<FontRun>::new();
        let mut selected_faces = HashMap::new();
        for cluster in text.graphemes(true) {
            let face_id = *selected_faces.entry(cluster).or_insert_with(|| {
                if self.face_supports_cluster(primary, cluster) {
                    primary
                } else {
                    self.resolve_fallback_id(spec, cluster).unwrap_or(primary)
                }
            });

            if let Some(last) = runs.last_mut() {
                if last.face_id == face_id {
                    last.text.push_str(cluster);
                    continue;
                }
            }
            runs.push(FontRun {
                face_id,
                text: cluster.to_string(),
            });
        }
        runs
    }

    fn face_supports_cluster(&self, id: ID, cluster: &str) -> bool {
        self.db
            .with_face_data(id, |data, index| face_covers_cluster(data, index, cluster))
            .unwrap_or(false)
    }

    fn resolve_fallback_id(&self, spec: &FontSpec, cluster: &str) -> Option<ID> {
        for family in &spec.families {
            if let Some(ids) = self.aliases.get(&normalize_family(family)) {
                if let Some(id) = best_supported_face(&self.db, ids, spec, cluster) {
                    return Some(id);
                }
                continue;
            }

            if let Some(id) = self.query_family(spec, family) {
                let names = &self.db.face(id)?.families;
                let ids = self
                    .db
                    .faces()
                    .filter(|face| {
                        face.families.iter().any(|(name, _)| {
                            names
                                .iter()
                                .any(|(wanted, _)| name.eq_ignore_ascii_case(wanted))
                        })
                    })
                    .map(|face| face.id)
                    .collect::<Vec<_>>();
                if let Some(id) = best_supported_face(&self.db, &ids, spec, cluster) {
                    return Some(id);
                }
            }
        }

        self.db
            .faces()
            .filter_map(|face| {
                if !self.face_supports_cluster(face.id, cluster) {
                    return None;
                }
                let style_penalty = style_penalty(spec.italic, face.style);
                let weight_penalty = i32::from(face.weight.0).abs_diff(i32::from(spec.weight));
                Some((style_penalty.saturating_add(weight_penalty), face.id))
            })
            .min_by_key(|(score, _)| *score)
            .map(|(_, id)| id)
    }

    fn resolve_id(&self, spec: &FontSpec) -> Option<ID> {
        for family in &spec.families {
            if let Some(ids) = self.aliases.get(&normalize_family(family)) {
                if let Some(id) = best_alias_face(&self.db, ids, spec) {
                    return Some(id);
                }
            }
            if let Some(id) = self.query_family(spec, family) {
                return Some(id);
            }
        }

        self.query_family(spec, "sans-serif")
            .or_else(|| self.db.faces().next().map(|face| face.id))
    }

    fn query_family(&self, spec: &FontSpec, family: &str) -> Option<ID> {
        self.db.query(&Query {
            families: &[family_query(family)],
            weight: Weight(spec.weight),
            style: if spec.italic {
                DbStyle::Italic
            } else {
                DbStyle::Normal
            },
            ..Query::default()
        })
    }
}

fn face_covers_cluster(data: &[u8], index: u32, cluster: &str) -> bool {
    let Some(face) = BuzzFace::from_slice(data, index) else {
        return false;
    };
    cluster.chars().all(|character| {
        is_default_ignorable(character)
            || face
                .glyph_index(character)
                .is_some_and(|glyph| glyph.0 != 0)
    })
}

fn is_default_ignorable(character: char) -> bool {
    // Unicode 17.0 DerivedCoreProperties.txt: Default_Ignorable_Code_Point.
    // Keep these controls in the shaping buffer, without demanding cmap glyphs.
    matches!(
        character as u32,
        0x00ad
            | 0x034f
            | 0x061c
            | 0x115f..=0x1160
            | 0x17b4..=0x17b5
            | 0x180b..=0x180f
            | 0x200b..=0x200f
            | 0x202a..=0x202e
            | 0x2060..=0x206f
            | 0x3164
            | 0xfe00..=0xfe0f
            | 0xfeff
            | 0xffa0
            | 0xfff0..=0xfff8
            | 0x1bca0..=0x1bca3
            | 0x1d173..=0x1d17a
            | 0xe0000..=0xe0fff
    )
}

fn cached_system_database() -> &'static Database {
    SYSTEM_FONT_DATABASE.get_or_init(|| {
        SYSTEM_FONT_SCAN_COUNT.fetch_add(1, Ordering::Relaxed);
        let mut db = Database::new();
        db.load_system_fonts();
        configure_generic_families(&mut db);
        db
    })
}

fn normalize_web_font_bytes(bytes: Vec<u8>) -> Vec<u8> {
    match detect_format(&bytes) {
        FontFormat::Woff1 | FontFormat::Woff2 => decode_auto(&bytes)
            .map(|decoded| decoded.sfnt)
            .unwrap_or(bytes),
        _ => bytes,
    }
}

fn family_query(value: &str) -> Family<'_> {
    match normalize_family(value).as_str() {
        "serif" => Family::Serif,
        "sans-serif" | "system-ui" | "ui-sans-serif" => Family::SansSerif,
        "monospace" | "ui-monospace" => Family::Monospace,
        "cursive" => Family::Cursive,
        "fantasy" => Family::Fantasy,
        _ => Family::Name(value),
    }
}

fn style_penalty(italic: bool, style: DbStyle) -> u32 {
    match (italic, style) {
        (true, DbStyle::Italic | DbStyle::Oblique) | (false, DbStyle::Normal) => 0,
        _ => 1500,
    }
}

fn best_alias_face(db: &Database, ids: &[ID], spec: &FontSpec) -> Option<ID> {
    ids.iter()
        .copied()
        .filter_map(|id| {
            let face = db.face(id)?;
            let penalty = style_penalty(spec.italic, face.style)
                .saturating_add(i32::from(face.weight.0).abs_diff(i32::from(spec.weight)));
            Some((penalty, id))
        })
        .min_by_key(|(score, _)| *score)
        .map(|(_, id)| id)
}

fn best_supported_face(db: &Database, ids: &[ID], spec: &FontSpec, cluster: &str) -> Option<ID> {
    ids.iter()
        .copied()
        .filter_map(|id| {
            let supported = db
                .with_face_data(id, |data, index| face_covers_cluster(data, index, cluster))
                .unwrap_or(false);
            if !supported {
                return None;
            }
            let face = db.face(id)?;
            let penalty = style_penalty(spec.italic, face.style)
                .saturating_add(i32::from(face.weight.0).abs_diff(i32::from(spec.weight)));
            Some((penalty, id))
        })
        .min_by_key(|(score, _)| *score)
        .map(|(_, id)| id)
}

fn configure_generic_families(db: &mut Database) {
    if let Some(name) = first_installed_family(
        db,
        &[
            "Arial",
            "Helvetica",
            "Segoe UI",
            "Inter",
            "Noto Sans",
            "DejaVu Sans",
            "Liberation Sans",
        ],
    ) {
        db.set_sans_serif_family(name);
    }
    if let Some(name) = first_installed_family(
        db,
        &[
            "Times New Roman",
            "Times",
            "Noto Serif",
            "DejaVu Serif",
            "Liberation Serif",
        ],
    ) {
        db.set_serif_family(name);
    }
    if let Some(name) = first_installed_family(
        db,
        &[
            "Consolas",
            "SFMono-Regular",
            "Noto Sans Mono",
            "DejaVu Sans Mono",
            "Liberation Mono",
        ],
    ) {
        db.set_monospace_family(name);
    }
}

fn first_installed_family(db: &Database, candidates: &[&str]) -> Option<String> {
    for candidate in candidates {
        if let Some((name, _)) = db
            .faces()
            .flat_map(|face| face.families.iter())
            .find(|(name, _)| name.eq_ignore_ascii_case(candidate))
        {
            return Some(name.clone());
        }
    }
    None
}

fn normalize_family(value: &str) -> String {
    value
        .trim()
        .trim_matches(|character| character == '"' || character == '\'')
        .to_ascii_lowercase()
}

fn shape_metrics(data: &[u8], face_index: u32, text: &str, spec: &FontSpec) -> Option<TextMetrics> {
    let face = BuzzFace::from_slice(data, face_index)?;
    let mut buffer = UnicodeBuffer::new();
    buffer.push_str(text);
    buffer.guess_segment_properties();
    let shaped = shape(&face, &[], buffer);
    let units_per_em = face.units_per_em().max(1) as f32;
    let scale = spec.size_px as f32 / units_per_em;
    let advance = shaped
        .glyph_positions()
        .iter()
        .map(|position| position.x_advance as f32 * scale)
        .sum::<f32>()
        .abs();

    let settings = FontSettings {
        collection_index: face_index,
        ..FontSettings::default()
    };
    let font = Font::from_bytes(data, settings).ok()?;
    let line = font.horizontal_line_metrics(spec.size_px as f32);
    let line_height = line
        .map(|metrics| metrics.new_line_size.ceil().max(spec.size_px as f32) as u32)
        .unwrap_or_else(|| fallback_line_height(spec.size_px));
    let ascent = line
        .map(|metrics| metrics.ascent.ceil().max(0.0) as u32)
        .unwrap_or(spec.size_px);

    Some(TextMetrics {
        width: advance.ceil().max(0.0) as u32,
        line_height,
        ascent,
    })
}

fn rasterize_shaped_text(
    data: &[u8],
    face_index: u32,
    text: &str,
    spec: &FontSpec,
) -> Option<RasterText> {
    let face = BuzzFace::from_slice(data, face_index)?;
    let mut buffer = UnicodeBuffer::new();
    buffer.push_str(text);
    buffer.guess_segment_properties();
    let shaped = shape(&face, &[], buffer);
    let units_per_em = face.units_per_em().max(1) as f32;
    let scale = spec.size_px as f32 / units_per_em;

    let settings = FontSettings {
        collection_index: face_index,
        ..FontSettings::default()
    };
    let font = Font::from_bytes(data, settings).ok()?;
    let line = font.horizontal_line_metrics(spec.size_px as f32);
    let line_height = line
        .map(|metrics| metrics.new_line_size.ceil().max(spec.size_px as f32) as u32)
        .unwrap_or_else(|| fallback_line_height(spec.size_px));
    let ascent = line
        .map(|metrics| metrics.ascent.ceil().max(0.0) as u32)
        .unwrap_or(spec.size_px);

    let mut pen_x = 0.0f32;
    let mut glyphs = Vec::with_capacity(shaped.len());
    for (info, position) in shaped.glyph_infos().iter().zip(shaped.glyph_positions()) {
        let glyph_id = u16::try_from(info.glyph_id).ok()?;
        let (metrics, coverage) = font.rasterize_indexed(glyph_id, spec.size_px as f32);
        let x_offset = position.x_offset as f32 * scale;
        let y_offset = position.y_offset as f32 * scale;
        let x = (pen_x + x_offset + metrics.xmin as f32).round() as i32;
        let top_from_baseline = metrics.ymin as f32 + metrics.height as f32 + y_offset;
        let y = (ascent as f32 - top_from_baseline).round() as i32;

        glyphs.push(RasterGlyph {
            x,
            y,
            width: metrics.width as u32,
            height: metrics.height as u32,
            coverage,
        });
        pen_x += position.x_advance as f32 * scale;
    }

    Some(RasterText {
        width: pen_x.abs().ceil().max(0.0) as u32,
        line_height,
        ascent,
        glyphs,
    })
}

fn fallback_metrics(text: &str, font_size: u32) -> TextMetrics {
    TextMetrics {
        width: fallback_glyph_width(font_size).saturating_mul(text.chars().count() as u32),
        line_height: fallback_line_height(font_size),
        ascent: font_size,
    }
}

pub fn fallback_glyph_width(font_size: u32) -> u32 {
    font_size.max(8).div_ceil(2).max(4)
}

pub fn fallback_line_height(font_size: u32) -> u32 {
    font_size.saturating_mul(5).saturating_div(4).max(10)
}

pub fn parse_font_face_rules(source: &str) -> Vec<WebFontFace> {
    let mut rules = Vec::new();
    let lower = source.to_ascii_lowercase();
    let mut cursor = 0usize;

    while let Some(offset) = lower[cursor..].find("@font-face") {
        let start = cursor + offset + "@font-face".len();
        let Some(open_offset) = source[start..].find('{') else {
            break;
        };
        let open = start + open_offset;
        let Some(close_offset) = source[open + 1..].find('}') else {
            break;
        };
        let close = open + 1 + close_offset;
        let body = &source[open + 1..close];

        let mut family = None;
        let mut sources = Vec::new();
        let mut weight = 400u16;
        let mut italic = false;

        for declaration in body.split(';') {
            let Some((name, value)) = declaration.split_once(':') else {
                continue;
            };
            match name.trim().to_ascii_lowercase().as_str() {
                "font-family" => {
                    let value = value
                        .trim()
                        .trim_matches(|character| character == '"' || character == '\'');
                    if !value.is_empty() {
                        family = Some(value.to_string());
                    }
                }
                "src" => {
                    sources.extend(extract_urls(value));
                }
                "font-weight" => {
                    weight = parse_font_weight(value).unwrap_or(weight);
                }
                "font-style" => {
                    italic = matches!(
                        value.trim().to_ascii_lowercase().as_str(),
                        "italic" | "oblique"
                    );
                }
                _ => {}
            }
        }

        if let Some(family) = family {
            if !sources.is_empty() {
                rules.push(WebFontFace {
                    family,
                    sources,
                    weight,
                    italic,
                });
            }
        }

        cursor = close + 1;
    }

    rules
}

fn extract_urls(value: &str) -> Vec<String> {
    let mut urls = Vec::new();
    let lower = value.to_ascii_lowercase();
    let mut cursor = 0usize;
    while let Some(offset) = lower[cursor..].find("url(") {
        let start = cursor + offset + 4;
        let Some(close_offset) = value[start..].find(')') else {
            break;
        };
        let close = start + close_offset;
        let url = value[start..close]
            .trim()
            .trim_matches(|character| character == '"' || character == '\'')
            .trim();
        if !url.is_empty() && !url.starts_with("data:") {
            urls.push(url.to_string());
        }
        cursor = close + 1;
    }
    urls
}

pub fn parse_font_weight(value: &str) -> Option<u16> {
    match value.trim().to_ascii_lowercase().as_str() {
        "normal" => Some(400),
        "bold" => Some(700),
        other => other
            .parse::<u16>()
            .ok()
            .map(|weight| weight.clamp(1, 1000)),
    }
}

#[cfg(test)]
#[path = "font_contract_tests.rs"]
mod contract_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_font_scan_is_cached_across_font_systems() {
        let _first = FontSystem::system();
        let scans_after_first = FontSystem::system_scan_count();
        let _second = FontSystem::system();
        assert_eq!(FontSystem::system_scan_count(), scans_after_first);
        assert!(scans_after_first <= 1);
    }

    #[test]
    fn fallback_metrics_remain_available_without_system_fonts() {
        let fonts = FontSystem::empty();
        let spec = FontSpec::new(&["sans-serif".to_string()], 400, false, 16);
        let metrics = fonts.measure_text("Quantic", &spec);
        assert!(metrics.width > 0);
        assert!(metrics.line_height >= 16);
        assert!(fonts.rasterize_text("Quantic", &spec).is_none());
    }

    #[test]
    fn mixed_text_can_be_planned_without_losing_characters() {
        let fonts = FontSystem::system();
        if fonts.face_count() == 0 {
            return;
        }
        let spec = FontSpec::new(&["sans-serif".to_string()], 400, false, 18);
        let text = "Quantic Ω العربية";
        let runs = fonts.plan_runs(text, &spec);
        assert!(!runs.is_empty());
        assert_eq!(
            runs.iter().map(|run| run.text.as_str()).collect::<String>(),
            text
        );
        let metrics = fonts.measure_text(text, &spec);
        assert!(metrics.width > 0);
        assert!(metrics.line_height >= 18);
    }

    #[test]
    fn woff1_roundtrip_can_be_registered_when_a_system_sfnt_is_available() {
        let system = FontSystem::system();
        let raw = system.db.faces().find_map(|face| {
            system.db.with_face_data(face.id, |data, _| {
                let magic = data.get(..4)?;
                matches!(magic, [0x00, 0x01, 0x00, 0x00] | [b'O', b'T', b'T', b'O'])
                    .then(|| data.to_vec())
            })?
        });
        let Some(raw) = raw else {
            return;
        };
        let Ok(woff) = oxifont_webfont::encode_woff1(&raw) else {
            return;
        };
        let mut target = FontSystem::empty();
        assert!(target.register_font_bytes("QWoff1", woff) > 0);
        assert!(target.has_alias("QWoff1"));
    }

    #[test]
    fn woff2_roundtrip_can_be_registered_when_encoder_supports_the_system_font() {
        let system = FontSystem::system();
        let raw = system.db.faces().find_map(|face| {
            system.db.with_face_data(face.id, |data, _| {
                let magic = data.get(..4)?;
                matches!(magic, [0x00, 0x01, 0x00, 0x00] | [b'O', b'T', b'T', b'O'])
                    .then(|| data.to_vec())
            })?
        });
        let Some(raw) = raw else {
            return;
        };
        let Ok(woff2) = oxifont_webfont::encode_woff2(&raw) else {
            return;
        };
        let mut target = FontSystem::empty();
        assert!(target.register_font_bytes("QWoff2", woff2) > 0);
        assert!(target.has_alias("QWoff2"));
    }

    #[test]
    fn parses_font_face_sources_and_style() {
        let rules = parse_font_face_rules(
            "@font-face {
               font-family: 'Quantic Sans';
               src: local('Ignore'), url('/fonts/quantic.woff2') format('woff2'),
                    url(\"/fonts/quantic.ttf\") format('truetype');
               font-weight: 700;
               font-style: italic;
             }",
        );
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].family, "Quantic Sans");
        assert_eq!(
            rules[0].sources,
            vec!["/fonts/quantic.woff2", "/fonts/quantic.ttf"]
        );
        assert_eq!(rules[0].weight, 700);
        assert!(rules[0].italic);
    }
}
