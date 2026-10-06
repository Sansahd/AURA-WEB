use std::collections::HashMap;

use crate::{
    css::{self, Declaration, Stylesheet},
    dom::{Document, NodeKind},
};

type Specificity = (u16, u16, u16);
type CascadeWinner = (bool, Specificity, usize, String);
type CascadeMap = HashMap<String, CascadeWinner>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Display {
    None,
    Block,
    Inline,
    Flex,
    Grid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlexDirection {
    Row,
    Column,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JustifyContent {
    Start,
    Center,
    End,
    SpaceBetween,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlignItems {
    Start,
    Center,
    End,
    Stretch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineHeight {
    Px(u32),
    RatioPerMille(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhiteSpace {
    Normal,
    NoWrap,
    Pre,
    PreWrap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RelativeLength {
    pub percent_per_mille: u32,
    pub px_offset: i32,
}

impl RelativeLength {
    pub fn resolve(self, basis: u32) -> u32 {
        let percent = i64::from(basis)
            .saturating_mul(i64::from(self.percent_per_mille))
            .saturating_div(1000);
        percent
            .saturating_add(i64::from(self.px_offset))
            .clamp(0, i64::from(u32::MAX)) as u32
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComputedStyle {
    pub display: Display,
    pub color: [u8; 4],
    pub background: Option<[u8; 4]>,
    pub font_size: u32,
    pub line_height: Option<LineHeight>,
    pub white_space: WhiteSpace,
    pub font_families: Vec<String>,
    pub font_weight: u16,
    pub font_italic: bool,
    pub margin_top: u32,
    pub margin_right: u32,
    pub margin_bottom: u32,
    pub margin_left: u32,
    pub padding_top: u32,
    pub padding_right: u32,
    pub padding_bottom: u32,
    pub padding_left: u32,
    pub border_width: u32,
    pub border_color: Option<[u8; 4]>,
    pub gap: u32,
    pub flex_direction: FlexDirection,
    pub justify_content: JustifyContent,
    pub align_items: AlignItems,
    pub flex_grow: u32,
    pub grid_columns: u32,
    pub width: Option<u32>,
    pub width_relative: Option<RelativeLength>,
    pub min_width: Option<u32>,
    pub min_width_relative: Option<RelativeLength>,
    pub max_width: Option<u32>,
    pub max_width_relative: Option<RelativeLength>,
    pub height: Option<u32>,
    pub min_height: Option<u32>,
    pub max_height: Option<u32>,
    pub custom_properties: HashMap<String, String>,
}

impl ComputedStyle {
    pub fn resolved_width(&self, basis: u32) -> Option<u32> {
        self.width
            .or_else(|| self.width_relative.map(|value| value.resolve(basis)))
    }

    pub fn resolved_min_width(&self, basis: u32) -> Option<u32> {
        self.min_width
            .or_else(|| self.min_width_relative.map(|value| value.resolve(basis)))
    }

    pub fn resolved_max_width(&self, basis: u32) -> Option<u32> {
        self.max_width
            .or_else(|| self.max_width_relative.map(|value| value.resolve(basis)))
    }

    pub fn line_height(&self) -> u32 {
        match self.line_height {
            Some(LineHeight::Px(value)) => value.max(1),
            Some(LineHeight::RatioPerMille(ratio)) => self
                .font_size
                .saturating_mul(ratio)
                .saturating_div(1000)
                .max(1),
            None => self.font_size.saturating_mul(5).saturating_div(4).max(1),
        }
    }
}

impl Default for ComputedStyle {
    fn default() -> Self {
        Self {
            display: Display::Inline,
            color: [24, 24, 24, 255],
            background: None,
            font_size: 16,
            line_height: None,
            white_space: WhiteSpace::Normal,
            font_families: vec!["sans-serif".to_string()],
            font_weight: 400,
            font_italic: false,
            margin_top: 0,
            margin_right: 0,
            margin_bottom: 0,
            margin_left: 0,
            padding_top: 0,
            padding_right: 0,
            padding_bottom: 0,
            padding_left: 0,
            border_width: 0,
            border_color: None,
            gap: 0,
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::Start,
            align_items: AlignItems::Stretch,
            flex_grow: 0,
            grid_columns: 1,
            width: None,
            width_relative: None,
            min_width: None,
            min_width_relative: None,
            max_width: None,
            max_width_relative: None,
            height: None,
            min_height: None,
            max_height: None,
            custom_properties: HashMap::new(),
        }
    }
}

pub fn compute_styles(document: &Document, stylesheet: &Stylesheet) -> Vec<ComputedStyle> {
    let mut styles = vec![ComputedStyle::default(); document.nodes.len()];
    compute_node(document, stylesheet, document.root, None, &mut styles);
    styles
}

fn compute_node(
    document: &Document,
    stylesheet: &Stylesheet,
    id: usize,
    inherited: Option<&ComputedStyle>,
    styles: &mut [ComputedStyle],
) {
    let mut style = initial_style(document, id);
    if let Some(parent) = inherited {
        style.color = parent.color;
        style.font_size = parent.font_size;
        style.line_height = parent.line_height;
        style.white_space = parent.white_space;
        style.font_families = parent.font_families.clone();
        style.font_weight = parent.font_weight;
        style.font_italic = parent.font_italic;
        style.custom_properties = parent.custom_properties.clone();
        apply_user_agent_defaults(document, id, &mut style);
    }

    let mut winners: CascadeMap = HashMap::new();

    for rule in &stylesheet.rules {
        let specificity = rule
            .selectors
            .iter()
            .filter(|selector| css::selector_matches(document, id, selector))
            .map(|selector| selector.specificity)
            .max();

        let Some(specificity) = specificity else {
            continue;
        };
        for declaration in &rule.declarations {
            apply_candidate(&mut winners, declaration, specificity, rule.order);
        }
    }

    if let Some(inline) = document.attribute(id, "style") {
        for declaration in css::parse_declarations(inline) {
            apply_candidate(&mut winners, &declaration, (u16::MAX, 0, 0), usize::MAX);
        }
    }

    let mut declarations = winners
        .into_iter()
        .map(|(name, (important, _, _, value))| Declaration {
            name,
            value,
            important,
        })
        .collect::<Vec<_>>();
    declarations.sort_by(|a, b| a.name.cmp(&b.name));

    for declaration in declarations
        .iter()
        .filter(|declaration| declaration.name.starts_with("--"))
    {
        style
            .custom_properties
            .insert(declaration.name.clone(), declaration.value.clone());
    }

    let resolved = declarations
        .into_iter()
        .filter(|declaration| !declaration.name.starts_with("--"))
        .filter_map(|declaration| {
            resolve_css_value(&declaration.value, &style.custom_properties, 0).map(|value| {
                Declaration {
                    name: declaration.name,
                    value,
                    important: declaration.important,
                }
            })
        })
        .collect::<Vec<_>>();
    apply_declarations(&mut style, &resolved);

    styles[id] = style.clone();
    for child in &document.nodes[id].children {
        compute_node(document, stylesheet, *child, Some(&style), styles);
    }
}

fn apply_candidate(
    winners: &mut CascadeMap,
    declaration: &Declaration,
    specificity: Specificity,
    order: usize,
) {
    let should_replace = winners
        .get(&declaration.name)
        .map(|(current_important, current_specificity, current_order, _)| {
            declaration.important > *current_important
                || (declaration.important == *current_important
                    && (specificity > *current_specificity
                        || (specificity == *current_specificity && order >= *current_order)))
        })
        .unwrap_or(true);

    if should_replace {
        winners.insert(
            declaration.name.clone(),
            (
                declaration.important,
                specificity,
                order,
                declaration.value.clone(),
            ),
        );
    }
}

fn initial_style(document: &Document, id: usize) -> ComputedStyle {
    let mut style = ComputedStyle::default();
    apply_user_agent_defaults(document, id, &mut style);
    style
}

fn apply_user_agent_defaults(document: &Document, id: usize, style: &mut ComputedStyle) {
    let Some(tag) = document.element_tag(id) else {
        if matches!(document.nodes[id].kind, NodeKind::Document) {
            style.display = Display::Block;
        }
        return;
    };

    style.display = match tag {
        "head" | "style" | "script" | "noscript" | "meta" | "link" | "title" => Display::None,
        "html" | "body" | "main" | "section" | "article" | "header" | "footer" | "nav" | "div"
        | "p" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "ul" | "ol" | "li" | "pre" | "form"
        | "blockquote" => Display::Block,
        _ => Display::Inline,
    };

    match tag {
        "h1" => {
            style.font_size = 32;
            style.margin_top = 12;
            style.margin_bottom = 10;
        }
        "h2" => {
            style.font_size = 26;
            style.margin_top = 10;
            style.margin_bottom = 8;
        }
        "h3" => {
            style.font_size = 21;
            style.margin_top = 8;
            style.margin_bottom = 6;
        }
        "p" => {
            style.margin_top = 6;
            style.margin_bottom = 8;
        }
        "pre" => {
            style.white_space = WhiteSpace::Pre;
            style.font_families = vec!["monospace".to_string()];
        }
        "body" => {
            style.margin_left = 8;
            style.margin_right = 8;
            style.margin_top = 8;
        }
        _ => {}
    }
}

fn apply_declarations(style: &mut ComputedStyle, declarations: &[Declaration]) {
    for declaration in declarations {
        let value = declaration.value.trim();
        match declaration.name.as_str() {
            "display" => {
                style.display = match value {
                    "none" => Display::None,
                    "block" => Display::Block,
                    "inline" => Display::Inline,
                    "flex" | "inline-flex" => Display::Flex,
                    "grid" | "inline-grid" => Display::Grid,
                    _ => style.display,
                };
            }
            "color" => {
                if let Some(color) = parse_color(value) {
                    style.color = color;
                }
            }
            "background" | "background-color" => {
                style.background = parse_color(value);
            }
            "font-size" => {
                if let Some(value) = parse_font_size(value, style.font_size) {
                    style.font_size = value.clamp(4, 256);
                }
            }
            "line-height" => {
                style.line_height = parse_line_height(value);
            }
            "white-space" => {
                style.white_space = match value {
                    "nowrap" => WhiteSpace::NoWrap,
                    "pre" => WhiteSpace::Pre,
                    "pre-wrap" => WhiteSpace::PreWrap,
                    "normal" => WhiteSpace::Normal,
                    _ => style.white_space,
                };
            }
            "font-family" => {
                let families = parse_font_families(value);
                if !families.is_empty() {
                    style.font_families = families;
                }
            }
            "font-weight" => {
                style.font_weight =
                    crate::font::parse_font_weight(value).unwrap_or(style.font_weight);
            }
            "font-style" => {
                style.font_italic = matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "italic" | "oblique"
                );
            }
            "margin" => apply_margin_shorthand(style, value),
            "margin-top" => style.margin_top = parse_px(value).unwrap_or(style.margin_top),
            "margin-right" => style.margin_right = parse_px(value).unwrap_or(style.margin_right),
            "margin-bottom" => style.margin_bottom = parse_px(value).unwrap_or(style.margin_bottom),
            "margin-left" => style.margin_left = parse_px(value).unwrap_or(style.margin_left),
            "padding" => apply_padding_shorthand(style, value),
            "padding-top" => style.padding_top = parse_px(value).unwrap_or(style.padding_top),
            "padding-right" => style.padding_right = parse_px(value).unwrap_or(style.padding_right),
            "padding-bottom" => {
                style.padding_bottom = parse_px(value).unwrap_or(style.padding_bottom)
            }
            "padding-left" => style.padding_left = parse_px(value).unwrap_or(style.padding_left),
            "border-width" => style.border_width = parse_px(value).unwrap_or(style.border_width),
            "border-color" => {
                if let Some(color) = parse_color(value) {
                    style.border_color = Some(color);
                }
            }
            "border" => apply_border_shorthand(style, value),
            "gap" | "column-gap" | "row-gap" => style.gap = parse_px(value).unwrap_or(style.gap),
            "flex-direction" => {
                style.flex_direction = match value {
                    "column" | "column-reverse" => FlexDirection::Column,
                    "row" | "row-reverse" => FlexDirection::Row,
                    _ => style.flex_direction,
                };
            }
            "justify-content" => {
                style.justify_content = match value {
                    "center" => JustifyContent::Center,
                    "end" | "flex-end" => JustifyContent::End,
                    "space-between" => JustifyContent::SpaceBetween,
                    "start" | "flex-start" => JustifyContent::Start,
                    _ => style.justify_content,
                };
            }
            "align-items" => {
                style.align_items = match value {
                    "center" => AlignItems::Center,
                    "end" | "flex-end" => AlignItems::End,
                    "start" | "flex-start" => AlignItems::Start,
                    "stretch" => AlignItems::Stretch,
                    _ => style.align_items,
                };
            }
            "flex-grow" => {
                style.flex_grow = value
                    .parse::<f32>()
                    .ok()
                    .map(|value| value.max(0.0).round() as u32)
                    .unwrap_or(style.flex_grow);
            }
            "grid-template-columns" => {
                style.grid_columns = parse_grid_column_count(value).unwrap_or(style.grid_columns);
            }
            "width" => {
                (style.width, style.width_relative) = parse_horizontal_length(value);
            }
            "min-width" => {
                (style.min_width, style.min_width_relative) = parse_horizontal_length(value);
            }
            "max-width" => {
                (style.max_width, style.max_width_relative) = parse_horizontal_length(value);
            }
            "height" => style.height = parse_px(value),
            "min-height" => style.min_height = parse_px(value),
            "max-height" => style.max_height = parse_px(value),
            _ => {}
        }
    }
}

fn resolve_css_value(
    value: &str,
    custom_properties: &HashMap<String, String>,
    depth: usize,
) -> Option<String> {
    if depth > 8 {
        return None;
    }

    let Some(start) = value.find("var(") else {
        return Some(value.to_string());
    };
    let open = start + 3;
    let mut nesting = 1usize;
    let mut close = None;
    for (offset, character) in value[open + 1..].char_indices() {
        match character {
            '(' => nesting = nesting.saturating_add(1),
            ')' => {
                nesting = nesting.saturating_sub(1);
                if nesting == 0 {
                    close = Some(open + 1 + offset);
                    break;
                }
            }
            _ => {}
        }
    }
    let close = close?;
    let inner = value[open + 1..close].trim();

    let mut comma = None;
    let mut nested = 0usize;
    for (index, character) in inner.char_indices() {
        match character {
            '(' => nested = nested.saturating_add(1),
            ')' => nested = nested.saturating_sub(1),
            ',' if nested == 0 => {
                comma = Some(index);
                break;
            }
            _ => {}
        }
    }

    let (name, fallback) = match comma {
        Some(index) => (inner[..index].trim(), Some(inner[index + 1..].trim())),
        None => (inner, None),
    };
    if !name.starts_with("--") {
        return None;
    }

    let replacement = match custom_properties.get(name) {
        Some(value) => resolve_css_value(value, custom_properties, depth + 1)?,
        None => resolve_css_value(fallback?, custom_properties, depth + 1)?,
    };

    let mut combined = String::new();
    combined.push_str(&value[..start]);
    combined.push_str(&replacement);
    combined.push_str(&value[close + 1..]);
    resolve_css_value(&combined, custom_properties, depth + 1)
}

fn parse_grid_column_count(value: &str) -> Option<u32> {
    let value = value.trim();
    if let Some(repeated) = value.strip_prefix("repeat(") {
        let (count, _) = repeated.split_once(',')?;
        return count
            .trim()
            .parse::<u32>()
            .ok()
            .map(|count| count.clamp(1, 12));
    }

    let count = value
        .split_ascii_whitespace()
        .filter(|track| !track.trim().is_empty())
        .count() as u32;
    (count > 0).then_some(count.clamp(1, 12))
}

fn parse_font_families(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(|family| {
            family
                .trim()
                .trim_matches(|character| character == '"' || character == '\'')
                .trim()
                .to_string()
        })
        .filter(|family| !family.is_empty())
        .collect()
}

fn apply_padding_shorthand(style: &mut ComputedStyle, value: &str) {
    let values = value
        .split_ascii_whitespace()
        .filter_map(parse_px)
        .collect::<Vec<_>>();
    match values.as_slice() {
        [all] => {
            style.padding_top = *all;
            style.padding_right = *all;
            style.padding_bottom = *all;
            style.padding_left = *all;
        }
        [vertical, horizontal] => {
            style.padding_top = *vertical;
            style.padding_bottom = *vertical;
            style.padding_left = *horizontal;
            style.padding_right = *horizontal;
        }
        [top, horizontal, bottom] => {
            style.padding_top = *top;
            style.padding_left = *horizontal;
            style.padding_right = *horizontal;
            style.padding_bottom = *bottom;
        }
        [top, right, bottom, left] => {
            style.padding_top = *top;
            style.padding_right = *right;
            style.padding_bottom = *bottom;
            style.padding_left = *left;
        }
        _ => {}
    }
}

fn apply_border_shorthand(style: &mut ComputedStyle, value: &str) {
    for token in value.split_ascii_whitespace() {
        if let Some(width) = parse_px(token) {
            style.border_width = width;
        }
        if let Some(color) = parse_color(token) {
            style.border_color = Some(color);
        }
    }
}

fn apply_margin_shorthand(style: &mut ComputedStyle, value: &str) {
    let values = value
        .split_ascii_whitespace()
        .filter_map(parse_px)
        .collect::<Vec<_>>();
    match values.as_slice() {
        [all] => {
            style.margin_top = *all;
            style.margin_right = *all;
            style.margin_bottom = *all;
            style.margin_left = *all;
        }
        [vertical, horizontal] => {
            style.margin_top = *vertical;
            style.margin_bottom = *vertical;
            style.margin_left = *horizontal;
            style.margin_right = *horizontal;
        }
        [top, horizontal, bottom] => {
            style.margin_top = *top;
            style.margin_left = *horizontal;
            style.margin_right = *horizontal;
            style.margin_bottom = *bottom;
        }
        [top, right, bottom, left] => {
            style.margin_top = *top;
            style.margin_right = *right;
            style.margin_bottom = *bottom;
            style.margin_left = *left;
        }
        _ => {}
    }
}

fn parse_horizontal_length(value: &str) -> (Option<u32>, Option<RelativeLength>) {
    let value = value.trim().to_ascii_lowercase();
    if let Some(percent) = value.strip_suffix('%') {
        if let Ok(percent) = percent.trim().parse::<f32>() {
            return (
                None,
                Some(RelativeLength {
                    percent_per_mille: (percent.max(0.0) * 10.0).round() as u32,
                    px_offset: 0,
                }),
            );
        }
    }

    if let Some(body) = value.strip_prefix("calc(").and_then(|value| value.strip_suffix(')')) {
        let compact = body.replace(' ', "");
        let percent_end = compact.find('%');
        if let Some(percent_end) = percent_end {
            if let Ok(percent) = compact[..percent_end].parse::<f32>() {
                let rest = &compact[percent_end + 1..];
                let px_offset = if rest.is_empty() {
                    Some(0)
                } else if let Some(raw) = rest.strip_prefix('+').and_then(|raw| raw.strip_suffix("px")) {
                    raw.parse::<f32>().ok().map(|value| value.round() as i32)
                } else if let Some(raw) = rest.strip_prefix('-').and_then(|raw| raw.strip_suffix("px")) {
                    raw.parse::<f32>().ok().map(|value| -(value.round() as i32))
                } else {
                    None
                };
                if let Some(px_offset) = px_offset {
                    return (
                        None,
                        Some(RelativeLength {
                            percent_per_mille: (percent.max(0.0) * 10.0).round() as u32,
                            px_offset,
                        }),
                    );
                }
            }
        }
    }

    (parse_px(&value), None)
}

fn parse_font_size(value: &str, inherited_size: u32) -> Option<u32> {
    let value = value.trim().to_ascii_lowercase();
    if let Some(raw) = value.strip_suffix("rem") {
        return raw
            .trim()
            .parse::<f32>()
            .ok()
            .map(|factor| (factor.max(0.0) * 16.0).round() as u32);
    }
    if let Some(raw) = value.strip_suffix("em") {
        return raw
            .trim()
            .parse::<f32>()
            .ok()
            .map(|factor| (factor.max(0.0) * inherited_size as f32).round() as u32);
    }
    if let Some(raw) = value.strip_suffix('%') {
        return raw
            .trim()
            .parse::<f32>()
            .ok()
            .map(|percent| (percent.max(0.0) * inherited_size as f32 / 100.0).round() as u32);
    }
    parse_px(&value)
}

fn parse_line_height(value: &str) -> Option<LineHeight> {
    let value = value.trim().to_ascii_lowercase();
    if value == "normal" {
        return None;
    }
    if let Some(raw) = value.strip_suffix('%') {
        return raw
            .trim()
            .parse::<f32>()
            .ok()
            .map(|percent| LineHeight::RatioPerMille((percent.max(0.0) * 10.0).round() as u32));
    }
    if value.ends_with("px") {
        return parse_px(&value).map(LineHeight::Px);
    }
    value
        .parse::<f32>()
        .ok()
        .map(|factor| LineHeight::RatioPerMille((factor.max(0.0) * 1000.0).round() as u32))
}

pub fn parse_px(value: &str) -> Option<u32> {
    let value = value.trim().strip_suffix("px").unwrap_or(value.trim());
    value
        .parse::<f32>()
        .ok()
        .map(|number| number.max(0.0) as u32)
}

pub fn parse_color(value: &str) -> Option<[u8; 4]> {
    let value = value.trim().to_ascii_lowercase();
    match value.as_str() {
        "black" => Some([0, 0, 0, 255]),
        "white" => Some([255, 255, 255, 255]),
        "red" => Some([255, 0, 0, 255]),
        "green" => Some([0, 128, 0, 255]),
        "blue" => Some([0, 0, 255, 255]),
        "yellow" => Some([255, 255, 0, 255]),
        "orange" => Some([255, 165, 0, 255]),
        "purple" => Some([128, 0, 128, 255]),
        "gray" | "grey" => Some([128, 128, 128, 255]),
        "transparent" => Some([0, 0, 0, 0]),
        _ if value.starts_with('#') => parse_hex_color(&value),
        _ if value.starts_with("rgb(") || value.starts_with("rgba(") => parse_rgb_color(&value),
        _ => None,
    }
}

fn parse_hex_color(value: &str) -> Option<[u8; 4]> {
    let hex = value.strip_prefix('#')?;
    match hex.len() {
        3 => Some([
            u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?,
            u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?,
            u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?,
            255,
        ]),
        4 => Some([
            u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?,
            u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?,
            u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?,
            u8::from_str_radix(&hex[3..4].repeat(2), 16).ok()?,
        ]),
        6 => Some([
            u8::from_str_radix(&hex[0..2], 16).ok()?,
            u8::from_str_radix(&hex[2..4], 16).ok()?,
            u8::from_str_radix(&hex[4..6], 16).ok()?,
            255,
        ]),
        8 => Some([
            u8::from_str_radix(&hex[0..2], 16).ok()?,
            u8::from_str_radix(&hex[2..4], 16).ok()?,
            u8::from_str_radix(&hex[4..6], 16).ok()?,
            u8::from_str_radix(&hex[6..8], 16).ok()?,
        ]),
        _ => None,
    }
}

fn parse_rgb_color(value: &str) -> Option<[u8; 4]> {
    let open = value.find('(')?;
    let close = value.rfind(')')?;
    let body = value[open + 1..close].replace('/', ",");
    let parts = body
        .split(',')
        .flat_map(|part| part.split_ascii_whitespace())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if parts.len() < 3 || parts.len() > 4 {
        return None;
    }

    let channel = |raw: &str| -> Option<u8> {
        if let Some(percent) = raw.trim().strip_suffix('%') {
            return percent
                .trim()
                .parse::<f32>()
                .ok()
                .map(|value| (value.clamp(0.0, 100.0) * 2.55).round() as u8);
        }
        raw.trim()
            .parse::<f32>()
            .ok()
            .map(|value| value.clamp(0.0, 255.0).round() as u8)
    };
    let alpha = |raw: &str| -> Option<u8> {
        if let Some(percent) = raw.trim().strip_suffix('%') {
            return percent
                .trim()
                .parse::<f32>()
                .ok()
                .map(|value| (value.clamp(0.0, 100.0) * 2.55).round() as u8);
        }
        raw.trim()
            .parse::<f32>()
            .ok()
            .map(|value| (value.clamp(0.0, 1.0) * 255.0).round() as u8)
    };

    Some([
        channel(parts[0])?,
        channel(parts[1])?,
        channel(parts[2])?,
        if parts.len() == 4 { alpha(parts[3])? } else { 255 },
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{css, html};

    #[test]
    fn percentage_and_calc_widths_resolve_against_containing_block() {
        let document = html::parse(
            "<div id='half'></div><div id='calc'></div><div id='max'></div>",
        );
        let sheet = css::parse_stylesheet(
            "#half{width:50%}#calc{width:calc(100% - 24px)}#max{max-width:75%}",
        );
        let styles = compute_styles(&document, &sheet);
        let half = document.find_by_id("half").unwrap();
        let calc = document.find_by_id("calc").unwrap();
        let max = document.find_by_id("max").unwrap();

        assert_eq!(styles[half].resolved_width(400), Some(200));
        assert_eq!(styles[calc].resolved_width(400), Some(376));
        assert_eq!(styles[max].resolved_max_width(400), Some(300));
    }

    #[test]
    fn modern_colors_relative_font_sizes_and_constraints_parse() {
        assert_eq!(parse_color("#1234"), Some([17, 34, 51, 68]));
        assert_eq!(parse_color("#11223380"), Some([17, 34, 51, 128]));
        assert_eq!(parse_color("rgb(100% 0% 50%)"), Some([255, 0, 128, 255]));
        assert_eq!(parse_color("rgba(10,20,30,0.5)"), Some([10, 20, 30, 128]));

        let document = html::parse("<div id='root'><span id='child'>x</span></div>");
        let sheet = css::parse_stylesheet(
            "#root{font-size:20px;line-height:1.5}#child{font-size:150%;min-width:10px;max-width:50px;min-height:4px;max-height:40px}",
        );
        let styles = compute_styles(&document, &sheet);
        let child = document.find_by_id("child").unwrap();
        assert_eq!(styles[child].font_size, 30);
        assert_eq!(styles[child].line_height(), 45);
        assert_eq!(styles[child].min_width, Some(10));
        assert_eq!(styles[child].max_width, Some(50));
        assert_eq!(styles[child].min_height, Some(4));
        assert_eq!(styles[child].max_height, Some(40));
    }

    #[test]
    fn white_space_is_inherited_and_pre_defaults_to_preformatted() {
        let document = html::parse(
            "<pre id='pre'><span id='child'>A  B</span></pre><div id='nowrap'><span id='n'>x</span></div>",
        );
        let sheet = css::parse_stylesheet("#nowrap{white-space:nowrap}");
        let styles = compute_styles(&document, &sheet);
        assert_eq!(
            styles[document.find_by_id("pre").unwrap()].white_space,
            WhiteSpace::Pre
        );
        assert_eq!(
            styles[document.find_by_id("child").unwrap()].white_space,
            WhiteSpace::Pre
        );
        assert_eq!(
            styles[document.find_by_id("n").unwrap()].white_space,
            WhiteSpace::NoWrap
        );
    }

    #[test]
    fn user_agent_element_defaults_follow_inheritance_but_precede_author_css() {
        let document = html::parse(
            "<div style='font-size:18px;white-space:normal'><h1 id='title'>Title</h1><pre id='pre'>A   B</pre></div>",
        );
        let sheet = css::parse_stylesheet("#title{font-size:40px}");
        let styles = compute_styles(&document, &sheet);

        let title = document.find_by_id("title").unwrap();
        let pre = document.find_by_id("pre").unwrap();

        assert_eq!(styles[title].font_size, 40);
        assert_eq!(styles[pre].white_space, WhiteSpace::Pre);
        assert_eq!(styles[pre].font_families, vec!["monospace".to_string()]);
    }

    #[test]
    fn important_overrides_higher_specificity_non_important() {
        let document = html::parse("<div id='target' class='card' style='color:green'>x</div>");
        let sheet = css::parse_stylesheet(
            "#target{color:red}.card{color:blue!important}",
        );
        let styles = compute_styles(&document, &sheet);
        let target = document.find_by_id("target").unwrap();
        assert_eq!(styles[target].color, [0, 0, 255, 255]);
    }

    #[test]
    fn custom_properties_are_inherited_and_resolved() {
        let document = crate::html::parse("<html><body><div id='box'>x</div></body></html>");
        let sheet = crate::css::parse_stylesheet(
            ":root{--accent:#123456;--space:12px}#box{color:var(--accent);padding:var(--space);background:var(--missing,#ffffff)}",
        );
        let styles = compute_styles(&document, &sheet);
        let box_id = document.find_by_id("box").unwrap();

        assert_eq!(styles[box_id].color, [18, 52, 86, 255]);
        assert_eq!(styles[box_id].padding_top, 12);
        assert_eq!(styles[box_id].padding_left, 12);
        assert_eq!(styles[box_id].background, Some([255, 255, 255, 255]));
    }

    #[test]
    fn computes_padding_border_and_flex_properties() {
        let document = html::parse("<div id='row'><span>A</span><span>B</span></div>");
        let sheet = css::parse_stylesheet(
            "#row{display:flex;padding:8px 12px;border:2px solid red;gap:6px;justify-content:center;align-items:center}",
        );
        let styles = compute_styles(&document, &sheet);
        let row = document.find_by_id("row").unwrap();

        assert_eq!(styles[row].display, Display::Flex);
        assert_eq!(styles[row].padding_top, 8);
        assert_eq!(styles[row].padding_left, 12);
        assert_eq!(styles[row].border_width, 2);
        assert_eq!(styles[row].border_color, Some([255, 0, 0, 255]));
        assert_eq!(styles[row].gap, 6);
        assert_eq!(styles[row].justify_content, JustifyContent::Center);
        assert_eq!(styles[row].align_items, AlignItems::Center);
    }

    #[test]
    fn font_properties_are_parsed_and_inherited() {
        let document = html::parse("<main id='root'><span id='child'>Text</span></main>");
        let sheet = css::parse_stylesheet(
            "#root{font-family:'DejaVu Sans', sans-serif;font-weight:700;font-style:italic;font-size:18px}",
        );
        let styles = compute_styles(&document, &sheet);
        let root = document.find_by_id("root").unwrap();
        let child = document.find_by_id("child").unwrap();

        assert_eq!(
            styles[root].font_families,
            vec!["DejaVu Sans".to_string(), "sans-serif".to_string()]
        );
        assert_eq!(styles[root].font_weight, 700);
        assert!(styles[root].font_italic);
        assert_eq!(styles[child].font_families, styles[root].font_families);
        assert_eq!(styles[child].font_weight, 700);
        assert!(styles[child].font_italic);
        assert_eq!(styles[child].font_size, 18);
    }

    #[test]
    fn cascade_prefers_id_and_inherits_color() {
        let document = html::parse("<main id='root'><p class='note'><span>Text</span></p></main>");
        let sheet = css::parse_stylesheet(
            "main{color:#112233} .note{color:red} #root .note{color:#00ff00}",
        );
        let styles = compute_styles(&document, &sheet);
        let paragraph = document
            .nodes
            .iter()
            .position(|node| matches!(&node.kind, NodeKind::Element { tag, .. } if tag == "p"))
            .unwrap();
        let span = document
            .nodes
            .iter()
            .position(|node| matches!(&node.kind, NodeKind::Element { tag, .. } if tag == "span"))
            .unwrap();
        assert_eq!(styles[paragraph].color, [0, 255, 0, 255]);
        assert_eq!(styles[span].color, [0, 255, 0, 255]);
    }
}
