use std::collections::HashMap;

use unicode_segmentation::UnicodeSegmentation;

use crate::{
    dom::{Document, NodeKind},
    font::{FontSpec, FontSystem},
    style::{AlignItems, ComputedStyle, Display, FlexDirection, JustifyContent, WhiteSpace},
};

pub type ImageDimensions = HashMap<usize, (u32, u32)>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayItemKind {
    Box,
    Text,
    Image,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayItem {
    pub kind: DisplayItemKind,
    pub node_id: usize,
    pub depth: usize,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub text: String,
    pub color: [u8; 4],
    pub background: Option<[u8; 4]>,
    pub border_width: u32,
    pub border_color: Option<[u8; 4]>,
    pub font_size: u32,
    pub font_families: Vec<String>,
    pub font_weight: u16,
    pub font_italic: bool,
}

#[derive(Debug, Clone)]
pub struct LayoutResult {
    pub items: Vec<DisplayItem>,
    pub content_height: u32,
}

pub fn build_display_list(
    document: &Document,
    styles: &[ComputedStyle],
    viewport_width: u32,
) -> LayoutResult {
    let fonts = FontSystem::system();
    build_display_list_with_resources(
        document,
        styles,
        viewport_width,
        &ImageDimensions::new(),
        &fonts,
    )
}

pub fn build_display_list_with_images(
    document: &Document,
    styles: &[ComputedStyle],
    viewport_width: u32,
    image_dimensions: &ImageDimensions,
) -> LayoutResult {
    let fonts = FontSystem::system();
    build_display_list_with_resources(document, styles, viewport_width, image_dimensions, &fonts)
}

pub fn build_display_list_with_resources(
    document: &Document,
    styles: &[ComputedStyle],
    viewport_width: u32,
    image_dimensions: &ImageDimensions,
    fonts: &FontSystem,
) -> LayoutResult {
    let mut items = Vec::new();
    let width = viewport_width.max(240);
    let height = layout_block(
        document,
        styles,
        image_dimensions,
        fonts,
        document.root,
        0,
        0,
        width,
        0,
        &mut items,
    );
    LayoutResult {
        items,
        content_height: height.max(64),
    }
}

#[allow(clippy::too_many_arguments)]
fn layout_block(
    document: &Document,
    styles: &[ComputedStyle],
    image_dimensions: &ImageDimensions,
    fonts: &FontSystem,
    id: usize,
    x: u32,
    y: u32,
    containing_width: u32,
    depth: usize,
    out: &mut Vec<DisplayItem>,
) -> u32 {
    let style = &styles[id];
    if style.display == Display::None {
        return 0;
    }

    if let NodeKind::Text(text) = &document.nodes[id].kind {
        return layout_text_run(id, text, style, fonts, x, y, containing_width, depth, out);
    }

    let is_document = matches!(document.nodes[id].kind, NodeKind::Document);
    let margin_left = if is_document { 0 } else { style.margin_left };
    let margin_right = if is_document { 0 } else { style.margin_right };
    let margin_top = if is_document { 0 } else { style.margin_top };
    let margin_bottom = if is_document { 0 } else { style.margin_bottom };
    let border = if is_document { 0 } else { style.border_width };

    let box_x = x.saturating_add(margin_left);
    let available = containing_width
        .saturating_sub(margin_left)
        .saturating_sub(margin_right)
        .max(1);
    let min_width = style
        .resolved_min_width(available)
        .unwrap_or(1)
        .min(available)
        .max(1);
    let max_width = style
        .resolved_max_width(available)
        .unwrap_or(available)
        .min(available)
        .max(min_width);
    let box_width = style
        .resolved_width(available)
        .unwrap_or(available)
        .clamp(min_width, max_width)
        .max(1);
    let box_y = y.saturating_add(margin_top);

    let horizontal_insets = border
        .saturating_mul(2)
        .saturating_add(style.padding_left)
        .saturating_add(style.padding_right);
    let vertical_insets = border
        .saturating_mul(2)
        .saturating_add(style.padding_top)
        .saturating_add(style.padding_bottom);
    let content_x = box_x
        .saturating_add(border)
        .saturating_add(style.padding_left);
    let content_y = box_y
        .saturating_add(border)
        .saturating_add(style.padding_top);
    let content_width = box_width.saturating_sub(horizontal_insets).max(1);

    let box_index = if is_document {
        None
    } else {
        out.push(DisplayItem {
            kind: DisplayItemKind::Box,
            node_id: id,
            depth,
            x: box_x,
            y: box_y,
            width: box_width,
            height: 0,
            text: String::new(),
            color: style.color,
            background: style.background,
            border_width: style.border_width,
            border_color: style.border_color,
            font_size: style.font_size,
            font_families: style.font_families.clone(),
            font_weight: style.font_weight,
            font_italic: style.font_italic,
        });
        Some(out.len() - 1)
    };

    let content_height = match style.display {
        Display::Flex => layout_flex_children(
            document,
            styles,
            image_dimensions,
            fonts,
            id,
            content_x,
            content_y,
            content_width,
            depth,
            style,
            out,
        ),
        Display::Grid => layout_grid_children(
            document,
            styles,
            image_dimensions,
            fonts,
            id,
            content_x,
            content_y,
            content_width,
            depth,
            style,
            out,
        ),
        _ => layout_normal_children(
            document,
            styles,
            image_dimensions,
            fonts,
            id,
            content_x,
            content_y,
            content_width,
            depth,
            style,
            out,
        ),
    };

    let minimum_height = style.line_height().saturating_add(vertical_insets);
    let mut own_height = style
        .height
        .unwrap_or_else(|| content_height.saturating_add(vertical_insets))
        .max(minimum_height);
    if let Some(min_height) = style.min_height {
        own_height = own_height.max(min_height);
    }
    if let Some(max_height) = style.max_height {
        own_height = own_height.min(max_height.max(1));
    }

    if let Some(index) = box_index {
        out[index].height = own_height;
    }

    margin_top
        .saturating_add(own_height)
        .saturating_add(margin_bottom)
}

#[allow(clippy::too_many_arguments)]
fn layout_normal_children(
    document: &Document,
    styles: &[ComputedStyle],
    image_dimensions: &ImageDimensions,
    fonts: &FontSystem,
    id: usize,
    content_x: u32,
    content_y: u32,
    content_width: u32,
    depth: usize,
    style: &ComputedStyle,
    out: &mut Vec<DisplayItem>,
) -> u32 {
    let mut cursor_y = content_y;
    let mut inline_x = content_x;
    let mut line_height = 0u32;

    for child in &document.nodes[id].children {
        let child_style = &styles[*child];
        if child_style.display == Display::None {
            continue;
        }

        let child_is_block = matches!(document.nodes[*child].kind, NodeKind::Element { .. })
            && matches!(
                child_style.display,
                Display::Block | Display::Flex | Display::Grid
            );

        if child_is_block {
            if inline_x > content_x {
                cursor_y = cursor_y.saturating_add(line_height.max(style.line_height()));
                inline_x = content_x;
                line_height = 0;
            }
            let used = layout_block(
                document,
                styles,
                image_dimensions,
                fonts,
                *child,
                content_x,
                cursor_y,
                content_width,
                depth + 1,
                out,
            );
            cursor_y = cursor_y.saturating_add(used);
        } else {
            let used = layout_inline(
                document,
                styles,
                image_dimensions,
                fonts,
                *child,
                &mut inline_x,
                &mut cursor_y,
                content_x,
                content_width,
                depth + 1,
                &mut line_height,
                out,
            );
            line_height = line_height.max(used);
        }
    }

    if inline_x > content_x {
        cursor_y = cursor_y.saturating_add(line_height.max(style.line_height()));
    }

    cursor_y.saturating_sub(content_y)
}

#[allow(clippy::too_many_arguments)]
fn layout_grid_children(
    document: &Document,
    styles: &[ComputedStyle],
    image_dimensions: &ImageDimensions,
    fonts: &FontSystem,
    id: usize,
    content_x: u32,
    content_y: u32,
    content_width: u32,
    depth: usize,
    style: &ComputedStyle,
    out: &mut Vec<DisplayItem>,
) -> u32 {
    let children = document.nodes[id]
        .children
        .iter()
        .copied()
        .filter(|child| styles[*child].display != Display::None)
        .collect::<Vec<_>>();

    if children.is_empty() {
        return 0;
    }

    let columns = style.grid_columns.max(1).min(children.len() as u32);
    let gap_total = style.gap.saturating_mul(columns.saturating_sub(1));
    let cell_width = content_width
        .saturating_sub(gap_total)
        .checked_div(columns)
        .unwrap_or(content_width)
        .max(1);

    let mut cursor_y = content_y;
    for row in children.chunks(columns as usize) {
        let mut row_height = 0u32;
        for (column, child) in row.iter().enumerate() {
            let x = content_x.saturating_add(
                (column as u32).saturating_mul(cell_width.saturating_add(style.gap)),
            );
            let used = layout_block(
                document,
                styles,
                image_dimensions,
                fonts,
                *child,
                x,
                cursor_y,
                cell_width,
                depth + 1,
                out,
            );
            row_height = row_height.max(used);
        }
        cursor_y = cursor_y.saturating_add(row_height);
        if row.len() == columns as usize && cursor_y > content_y {
            cursor_y = cursor_y.saturating_add(style.gap);
        }
    }

    if children.len() % columns as usize == 0 {
        cursor_y = cursor_y.saturating_sub(style.gap);
    }
    cursor_y.saturating_sub(content_y)
}

#[allow(clippy::too_many_arguments)]
fn layout_flex_children(
    document: &Document,
    styles: &[ComputedStyle],
    image_dimensions: &ImageDimensions,
    fonts: &FontSystem,
    id: usize,
    content_x: u32,
    content_y: u32,
    content_width: u32,
    depth: usize,
    style: &ComputedStyle,
    out: &mut Vec<DisplayItem>,
) -> u32 {
    let children = document.nodes[id]
        .children
        .iter()
        .copied()
        .filter(|child| styles[*child].display != Display::None)
        .collect::<Vec<_>>();

    if children.is_empty() {
        return 0;
    }

    match style.flex_direction {
        FlexDirection::Column => layout_flex_column(
            document,
            styles,
            image_dimensions,
            fonts,
            &children,
            content_x,
            content_y,
            content_width,
            depth,
            style.gap,
            out,
        ),
        FlexDirection::Row => layout_flex_row(
            document,
            styles,
            image_dimensions,
            fonts,
            &children,
            content_x,
            content_y,
            content_width,
            depth,
            style,
            out,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn layout_flex_column(
    document: &Document,
    styles: &[ComputedStyle],
    image_dimensions: &ImageDimensions,
    fonts: &FontSystem,
    children: &[usize],
    content_x: u32,
    content_y: u32,
    content_width: u32,
    depth: usize,
    gap: u32,
    out: &mut Vec<DisplayItem>,
) -> u32 {
    let mut cursor_y = content_y;
    for (index, child) in children.iter().enumerate() {
        if index > 0 {
            cursor_y = cursor_y.saturating_add(gap);
        }
        let used = layout_block(
            document,
            styles,
            image_dimensions,
            fonts,
            *child,
            content_x,
            cursor_y,
            content_width,
            depth + 1,
            out,
        );
        cursor_y = cursor_y.saturating_add(used);
    }
    cursor_y.saturating_sub(content_y)
}

#[allow(clippy::too_many_arguments)]
fn layout_flex_row(
    document: &Document,
    styles: &[ComputedStyle],
    image_dimensions: &ImageDimensions,
    fonts: &FontSystem,
    children: &[usize],
    content_x: u32,
    content_y: u32,
    content_width: u32,
    depth: usize,
    style: &ComputedStyle,
    out: &mut Vec<DisplayItem>,
) -> u32 {
    let base_gap_total = style
        .gap
        .saturating_mul(children.len().saturating_sub(1) as u32);

    let mut fixed = 0u32;
    let mut weight = 0u32;
    for child in children {
        let child_style = &styles[*child];
        let margins = child_style
            .margin_left
            .saturating_add(child_style.margin_right);
        if let Some(width) = child_style.resolved_width(content_width) {
            fixed = fixed.saturating_add(width.saturating_add(margins));
        } else {
            weight = weight.saturating_add(child_style.flex_grow.max(1));
        }
    }

    let remaining = content_width
        .saturating_sub(fixed)
        .saturating_sub(base_gap_total);
    let mut allocations = Vec::with_capacity(children.len());
    for child in children {
        let child_style = &styles[*child];
        let margins = child_style
            .margin_left
            .saturating_add(child_style.margin_right);
        let allocation = if let Some(width) = child_style.resolved_width(content_width) {
            width.saturating_add(margins)
        } else {
            remaining
                .saturating_mul(child_style.flex_grow.max(1))
                .checked_div(weight.max(1))
                .unwrap_or(1)
                .max(1)
        };
        allocations.push(allocation);
    }

    let occupied = allocations
        .iter()
        .copied()
        .fold(base_gap_total, u32::saturating_add);
    let free = content_width.saturating_sub(occupied);
    let mut cursor_x = content_x;
    let mut gap = style.gap;

    match style.justify_content {
        JustifyContent::Center => cursor_x = cursor_x.saturating_add(free / 2),
        JustifyContent::End => cursor_x = cursor_x.saturating_add(free),
        JustifyContent::SpaceBetween if children.len() > 1 => {
            gap = gap.saturating_add(free / (children.len() as u32 - 1));
        }
        _ => {}
    }

    let mut records = Vec::with_capacity(children.len());
    let mut max_height = 0u32;

    for (index, child) in children.iter().enumerate() {
        let start = out.len();
        let used = layout_block(
            document,
            styles,
            image_dimensions,
            fonts,
            *child,
            cursor_x,
            content_y,
            allocations[index],
            depth + 1,
            out,
        );
        let end = out.len();
        max_height = max_height.max(used);
        records.push((start, end, used));

        cursor_x = cursor_x.saturating_add(allocations[index]);
        if index + 1 < children.len() {
            cursor_x = cursor_x.saturating_add(gap);
        }
    }

    for (start, end, used) in records {
        let delta = match style.align_items {
            AlignItems::Center => max_height.saturating_sub(used) / 2,
            AlignItems::End => max_height.saturating_sub(used),
            AlignItems::Start | AlignItems::Stretch => 0,
        };
        if delta > 0 {
            for item in &mut out[start..end] {
                item.y = item.y.saturating_add(delta);
            }
        }
    }

    max_height
}

#[allow(clippy::too_many_arguments)]
fn layout_inline(
    document: &Document,
    styles: &[ComputedStyle],
    image_dimensions: &ImageDimensions,
    fonts: &FontSystem,
    id: usize,
    inline_x: &mut u32,
    cursor_y: &mut u32,
    line_start_x: u32,
    line_width: u32,
    depth: usize,
    line_height: &mut u32,
    out: &mut Vec<DisplayItem>,
) -> u32 {
    let style = &styles[id];
    match &document.nodes[id].kind {
        NodeKind::Text(text) => layout_inline_text(
            id,
            text,
            style,
            fonts,
            inline_x,
            cursor_y,
            line_start_x,
            line_width,
            depth,
            line_height,
            out,
        ),
        NodeKind::Element { tag, .. } if tag == "img" => layout_inline_image(
            document,
            image_dimensions,
            id,
            style,
            inline_x,
            cursor_y,
            line_start_x,
            line_width,
            depth,
            line_height,
            out,
        ),
        _ => {
            let mut max_height = 0;
            for child in &document.nodes[id].children {
                max_height = max_height.max(layout_inline(
                    document,
                    styles,
                    image_dimensions,
                    fonts,
                    *child,
                    inline_x,
                    cursor_y,
                    line_start_x,
                    line_width,
                    depth + 1,
                    line_height,
                    out,
                ));
            }
            max_height
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn layout_inline_image(
    document: &Document,
    image_dimensions: &ImageDimensions,
    id: usize,
    style: &ComputedStyle,
    inline_x: &mut u32,
    cursor_y: &mut u32,
    line_start_x: u32,
    line_width: u32,
    depth: usize,
    line_height: &mut u32,
    out: &mut Vec<DisplayItem>,
) -> u32 {
    let natural = image_dimensions.get(&id).copied().unwrap_or((16, 16));
    let html_width = document.attribute(id, "width").and_then(parse_dimension);
    let html_height = document.attribute(id, "height").and_then(parse_dimension);

    let mut requested_width = style
        .resolved_width(line_width)
        .or(html_width)
        .unwrap_or(natural.0)
        .max(1);
    if let Some(min_width) = style.resolved_min_width(line_width) {
        requested_width = requested_width.max(min_width);
    }
    if let Some(max_width) = style.resolved_max_width(line_width) {
        requested_width = requested_width.min(max_width.max(1));
    }
    let mut width = requested_width.min(line_width.max(1));
    let requested_height = style.height.or(html_height).unwrap_or_else(|| {
        natural
            .1
            .saturating_mul(requested_width)
            .checked_div(natural.0.max(1))
            .unwrap_or(natural.1)
            .max(1)
    });
    let mut height = requested_height.max(1);
    if let Some(min_height) = style.min_height {
        height = height.max(min_height);
    }
    if let Some(max_height) = style.max_height {
        height = height.min(max_height.max(1));
    }

    if width < requested_width {
        height = height
            .saturating_mul(width)
            .checked_div(requested_width.max(1))
            .unwrap_or(height)
            .max(1);
    }

    if *inline_x > line_start_x
        && inline_x.saturating_add(width) > line_start_x.saturating_add(line_width)
    {
        *cursor_y = cursor_y.saturating_add((*line_height).max(style.line_height()));
        *inline_x = line_start_x;
        *line_height = 0;
    }

    if width > line_width {
        width = line_width.max(1);
    }

    out.push(DisplayItem {
        kind: DisplayItemKind::Image,
        node_id: id,
        depth,
        x: *inline_x,
        y: *cursor_y,
        width,
        height,
        text: document.attribute(id, "alt").unwrap_or("").to_string(),
        color: style.color,
        background: style.background,
        border_width: 0,
        border_color: None,
        font_size: style.font_size,
        font_families: style.font_families.clone(),
        font_weight: style.font_weight,
        font_italic: style.font_italic,
    });
    *inline_x = inline_x.saturating_add(width);
    *line_height = (*line_height).max(height);
    height
}

#[allow(clippy::too_many_arguments)]
fn layout_inline_text(
    id: usize,
    text: &str,
    style: &ComputedStyle,
    fonts: &FontSystem,
    inline_x: &mut u32,
    cursor_y: &mut u32,
    line_start_x: u32,
    line_width: u32,
    depth: usize,
    line_height: &mut u32,
    out: &mut Vec<DisplayItem>,
) -> u32 {
    let spec = FontSpec::new(
        &style.font_families,
        style.font_weight,
        style.font_italic,
        style.font_size,
    );
    let height = fonts
        .measure_text("Mg", &spec)
        .line_height
        .max(style.line_height());

    match style.white_space {
        WhiteSpace::Normal => layout_collapsed_text(
            id,
            text,
            style,
            fonts,
            &spec,
            inline_x,
            cursor_y,
            line_start_x,
            line_width,
            depth,
            line_height,
            height,
            true,
            out,
        ),
        WhiteSpace::NoWrap => layout_collapsed_text(
            id,
            text,
            style,
            fonts,
            &spec,
            inline_x,
            cursor_y,
            line_start_x,
            line_width,
            depth,
            line_height,
            height,
            false,
            out,
        ),
        WhiteSpace::Pre => layout_preserved_text(
            id,
            text,
            style,
            fonts,
            &spec,
            inline_x,
            cursor_y,
            line_start_x,
            line_width,
            depth,
            line_height,
            height,
            false,
            out,
        ),
        WhiteSpace::PreWrap => layout_preserved_text(
            id,
            text,
            style,
            fonts,
            &spec,
            inline_x,
            cursor_y,
            line_start_x,
            line_width,
            depth,
            line_height,
            height,
            true,
            out,
        ),
    }

    height
}

#[allow(clippy::too_many_arguments)]
fn layout_collapsed_text(
    id: usize,
    text: &str,
    style: &ComputedStyle,
    fonts: &FontSystem,
    spec: &FontSpec,
    inline_x: &mut u32,
    cursor_y: &mut u32,
    line_start_x: u32,
    line_width: u32,
    depth: usize,
    line_height: &mut u32,
    height: u32,
    allow_wrap: bool,
    out: &mut Vec<DisplayItem>,
) {
    let measured_space = fonts.measure_text(" ", spec).width.max(1);
    for word in text.split_whitespace() {
        let word_width = fonts.measure_text(word, spec).width.max(1);
        let space_width = if *inline_x > line_start_x {
            measured_space
        } else {
            0
        };
        if allow_wrap
            && *inline_x > line_start_x
            && inline_x
                .saturating_add(space_width)
                .saturating_add(word_width)
                > line_start_x.saturating_add(line_width)
        {
            advance_line(
                inline_x,
                cursor_y,
                line_start_x,
                line_height,
                height,
            );
        }

        if space_width > 0 {
            *inline_x = inline_x.saturating_add(space_width);
        }
        push_text_item(
            id,
            word,
            style,
            *inline_x,
            *cursor_y,
            word_width,
            height,
            depth,
            out,
        );
        *inline_x = inline_x.saturating_add(word_width);
        *line_height = (*line_height).max(height);
    }
}

#[allow(clippy::too_many_arguments)]
fn layout_preserved_text(
    id: usize,
    text: &str,
    style: &ComputedStyle,
    fonts: &FontSystem,
    spec: &FontSpec,
    inline_x: &mut u32,
    cursor_y: &mut u32,
    line_start_x: u32,
    line_width: u32,
    depth: usize,
    line_height: &mut u32,
    height: u32,
    allow_wrap: bool,
    out: &mut Vec<DisplayItem>,
) {
    for (line_index, line) in text.split('\n').enumerate() {
        if line_index > 0 {
            advance_line(
                inline_x,
                cursor_y,
                line_start_x,
                line_height,
                height,
            );
        }

        if !allow_wrap {
            if !line.is_empty() {
                let width = fonts.measure_text(line, spec).width.max(1);
                push_text_item(
                    id,
                    line,
                    style,
                    *inline_x,
                    *cursor_y,
                    width,
                    height,
                    depth,
                    out,
                );
                *inline_x = inline_x.saturating_add(width);
                *line_height = (*line_height).max(height);
            }
            continue;
        }

        let mut fragment = String::new();
        let mut fragment_width = 0u32;
        for grapheme in UnicodeSegmentation::graphemes(line, true) {
            let grapheme_width = fonts.measure_text(grapheme, spec).width.max(1);
            if *inline_x > line_start_x
                && inline_x
                    .saturating_add(fragment_width)
                    .saturating_add(grapheme_width)
                    > line_start_x.saturating_add(line_width)
            {
                if !fragment.is_empty() {
                    push_text_item(
                        id,
                        &fragment,
                        style,
                        *inline_x,
                        *cursor_y,
                        fragment_width,
                        height,
                        depth,
                        out,
                    );
                    *inline_x = inline_x.saturating_add(fragment_width);
                    fragment.clear();
                    fragment_width = 0;
                }
                advance_line(
                    inline_x,
                    cursor_y,
                    line_start_x,
                    line_height,
                    height,
                );
            }
            fragment.push_str(grapheme);
            fragment_width = fragment_width.saturating_add(grapheme_width);
        }
        if !fragment.is_empty() {
            push_text_item(
                id,
                &fragment,
                style,
                *inline_x,
                *cursor_y,
                fragment_width,
                height,
                depth,
                out,
            );
            *inline_x = inline_x.saturating_add(fragment_width);
            *line_height = (*line_height).max(height);
        }
    }
}

fn advance_line(
    inline_x: &mut u32,
    cursor_y: &mut u32,
    line_start_x: u32,
    line_height: &mut u32,
    minimum_height: u32,
) {
    *cursor_y = cursor_y.saturating_add((*line_height).max(minimum_height));
    *inline_x = line_start_x;
    *line_height = 0;
}

#[allow(clippy::too_many_arguments)]
fn push_text_item(
    id: usize,
    text: &str,
    style: &ComputedStyle,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    depth: usize,
    out: &mut Vec<DisplayItem>,
) {
    out.push(DisplayItem {
        kind: DisplayItemKind::Text,
        node_id: id,
        depth,
        x,
        y,
        width,
        height,
        text: text.to_string(),
        color: style.color,
        background: style.background,
        border_width: 0,
        border_color: None,
        font_size: style.font_size,
        font_families: style.font_families.clone(),
        font_weight: style.font_weight,
        font_italic: style.font_italic,
    });
}

#[allow(clippy::too_many_arguments)]
fn layout_text_run(
    id: usize,
    text: &str,
    style: &ComputedStyle,
    fonts: &FontSystem,
    x: u32,
    y: u32,
    width: u32,
    depth: usize,
    out: &mut Vec<DisplayItem>,
) -> u32 {
    let mut inline_x = x;
    let mut cursor_y = y;
    let mut line_height = 0;
    layout_inline_text(
        id,
        text,
        style,
        fonts,
        &mut inline_x,
        &mut cursor_y,
        x,
        width,
        depth,
        &mut line_height,
        out,
    );
    cursor_y
        .saturating_sub(y)
        .saturating_add(line_height.max(style.line_height()))
}

fn parse_dimension(value: &str) -> Option<u32> {
    value
        .trim()
        .trim_end_matches("px")
        .parse::<u32>()
        .ok()
        .filter(|value| *value > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{css, html, style};

    #[test]
    fn percentage_and_calc_widths_affect_layout() {
        let doc = html::parse(
            "<div id='half'>A</div><div id='calc'>B</div><div id='max'>C</div>",
        );
        let sheet = css::parse_stylesheet(
            "#half{width:50%}#calc{width:calc(100% - 20px)}#max{width:100%;max-width:75%}",
        );
        let styles = style::compute_styles(&doc, &sheet);
        let result = build_display_list(&doc, &styles, 400);

        let width_for = |id: &str| {
            let node = doc.find_by_id(id).unwrap();
            result
                .items
                .iter()
                .find(|item| item.kind == DisplayItemKind::Box && item.node_id == node)
                .map(|item| item.width)
                .unwrap()
        };

        assert_eq!(width_for("half"), 200);
        assert_eq!(width_for("calc"), 380);
        assert_eq!(width_for("max"), 300);
    }

    #[test]
    fn white_space_modes_control_collapsing_and_wrapping() {
        let doc = html::parse(
            "<div id='normal'>A   B</div><pre id='pre'>A   B\nC</pre><div id='nowrap'>A B C D</div>",
        );
        let sheet = css::parse_stylesheet("#nowrap{white-space:nowrap}");
        let styles = style::compute_styles(&doc, &sheet);
        let result = build_display_list(&doc, &styles, 48);

        let normal = doc.find_by_id("normal").unwrap();
        let pre = doc.find_by_id("pre").unwrap();
        let nowrap = doc.find_by_id("nowrap").unwrap();

        let normal_text = result
            .items
            .iter()
            .filter(|item| item.kind == DisplayItemKind::Text && {
                let mut parent = doc.nodes[item.node_id].parent;
                let mut inside = false;
                while let Some(id) = parent {
                    if id == normal {
                        inside = true;
                        break;
                    }
                    parent = doc.nodes[id].parent;
                }
                inside
            })
            .map(|item| item.text.as_str())
            .collect::<Vec<_>>();
        assert_eq!(normal_text, vec!["A", "B"]);

        let pre_lines = result
            .items
            .iter()
            .filter(|item| item.kind == DisplayItemKind::Text && {
                let mut parent = doc.nodes[item.node_id].parent;
                let mut inside = false;
                while let Some(id) = parent {
                    if id == pre {
                        inside = true;
                        break;
                    }
                    parent = doc.nodes[id].parent;
                }
                inside
            })
            .map(|item| item.text.clone())
            .collect::<Vec<_>>();
        assert!(pre_lines.contains(&"A   B".to_string()));
        assert!(pre_lines.contains(&"C".to_string()));

        let nowrap_lines = result
            .items
            .iter()
            .filter(|item| item.kind == DisplayItemKind::Text && {
                let mut parent = doc.nodes[item.node_id].parent;
                let mut inside = false;
                while let Some(id) = parent {
                    if id == nowrap {
                        inside = true;
                        break;
                    }
                    parent = doc.nodes[id].parent;
                }
                inside
            })
            .map(|item| item.y)
            .collect::<Vec<_>>();
        assert!(nowrap_lines.windows(2).all(|pair| pair[0] == pair[1]));
    }

    #[test]
    fn lays_out_blocks_vertically_and_wraps_inline_text() {
        let doc = html::parse(
            "<style>p{font-size:16px}</style><p>Hello world from Quantic Engine</p><p>Second</p>",
        );
        let sheet = css::parse_stylesheet("p{font-size:16px}");
        let styles = style::compute_styles(&doc, &sheet);
        let result = build_display_list(&doc, &styles, 120);
        let words = result
            .items
            .iter()
            .filter(|item| item.kind == DisplayItemKind::Text)
            .map(|item| item.text.as_str())
            .collect::<Vec<_>>();
        assert!(words.contains(&"Quantic"));
        assert!(result.content_height > 32);
    }

    #[test]
    fn real_font_metrics_drive_text_width_when_available_and_fallback_is_safe() {
        let doc = html::parse("<p id='p'>iiii WWWW</p>");
        let styles = style::compute_styles(&doc, &css::Stylesheet::default());
        let fallback = FontSystem::empty();
        let result = build_display_list_with_resources(
            &doc,
            &styles,
            320,
            &ImageDimensions::new(),
            &fallback,
        );
        let words = result
            .items
            .iter()
            .filter(|item| item.kind == DisplayItemKind::Text)
            .collect::<Vec<_>>();
        assert_eq!(words.len(), 2);
        assert!(words.iter().all(|item| item.width > 0));
    }

    #[test]
    fn image_participates_in_inline_layout() {
        let doc = html::parse("<p>Before <img id='hero' src='hero.png' alt='Hero'> after</p>");
        let styles = style::compute_styles(&doc, &css::Stylesheet::default());
        let image_id = doc.find_by_id("hero").unwrap();
        let mut dimensions = ImageDimensions::new();
        dimensions.insert(image_id, (80, 40));
        let result = build_display_list_with_images(&doc, &styles, 320, &dimensions);
        let image = result
            .items
            .iter()
            .find(|item| item.kind == DisplayItemKind::Image)
            .unwrap();
        assert_eq!((image.width, image.height), (80, 40));
        assert_eq!(image.text, "Hero");
    }

    #[test]
    fn flex_row_places_children_on_the_same_line_with_gap() {
        let doc = html::parse("<div id='row'><div id='a'>A</div><div id='b'>B</div></div>");
        let sheet =
            css::parse_stylesheet("#row{display:flex;gap:10px;padding:4px} #a,#b{width:60px}");
        let styles = style::compute_styles(&doc, &sheet);
        let result = build_display_list(&doc, &styles, 240);
        let a = doc.find_by_id("a").unwrap();
        let b = doc.find_by_id("b").unwrap();
        let a_box = result
            .items
            .iter()
            .find(|item| item.kind == DisplayItemKind::Box && item.node_id == a)
            .unwrap();
        let b_box = result
            .items
            .iter()
            .find(|item| item.kind == DisplayItemKind::Box && item.node_id == b)
            .unwrap();
        assert_eq!(a_box.y, b_box.y);
        assert!(b_box.x >= a_box.x.saturating_add(a_box.width).saturating_add(10));
    }

    #[test]
    fn padding_moves_content_inside_the_box() {
        let doc = html::parse("<div id='box'>Hello</div>");
        let sheet = css::parse_stylesheet("#box{padding:12px;border:2px solid red}");
        let styles = style::compute_styles(&doc, &sheet);
        let result = build_display_list(&doc, &styles, 240);
        let box_id = doc.find_by_id("box").unwrap();
        let box_item = result
            .items
            .iter()
            .find(|item| item.kind == DisplayItemKind::Box && item.node_id == box_id)
            .unwrap();
        let text = result
            .items
            .iter()
            .find(|item| item.kind == DisplayItemKind::Text && item.text == "Hello")
            .unwrap();
        assert!(text.x >= box_item.x + 14);
        assert!(text.y >= box_item.y + 14);
    }
}
