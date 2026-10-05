use std::collections::HashMap;

use font8x8::{UnicodeFonts, BASIC_FONTS};
use image::{imageops::FilterType, Rgba, RgbaImage};

use crate::{
    font::{FontSpec, FontSystem},
    layout::{DisplayItem, DisplayItemKind},
};

pub type DecodedImages = HashMap<usize, RgbaImage>;

pub fn rasterize(items: &[DisplayItem], width: u32, height: u32) -> RgbaImage {
    let fonts = FontSystem::system();
    rasterize_with_images_and_fonts(items, width, height, &DecodedImages::new(), &fonts)
}

pub fn rasterize_with_images(
    items: &[DisplayItem],
    width: u32,
    height: u32,
    images: &DecodedImages,
) -> RgbaImage {
    let fonts = FontSystem::system();
    rasterize_with_images_and_fonts(items, width, height, images, &fonts)
}

pub fn rasterize_with_images_and_fonts(
    items: &[DisplayItem],
    width: u32,
    height: u32,
    images: &DecodedImages,
    fonts: &FontSystem,
) -> RgbaImage {
    let mut image = RgbaImage::from_pixel(width.max(1), height.max(1), Rgba([255, 255, 255, 255]));

    for item in items {
        if let Some(background) = item.background {
            fill_rect(
                &mut image,
                item.x,
                item.y,
                item.width,
                item.height,
                background,
            );
        }
        if item.border_width > 0 {
            if let Some(border_color) = item.border_color {
                draw_border(
                    &mut image,
                    item.x,
                    item.y,
                    item.width,
                    item.height,
                    item.border_width,
                    border_color,
                );
            }
        }

        match item.kind {
            DisplayItemKind::Text => draw_text(
                &mut image,
                fonts,
                item.x,
                item.y,
                &item.text,
                item.color,
                item.font_size,
                &item.font_families,
                item.font_weight,
                item.font_italic,
            ),
            DisplayItemKind::Image => {
                if let Some(source) = images.get(&item.node_id) {
                    draw_image(&mut image, source, item.x, item.y, item.width, item.height);
                } else {
                    draw_image_placeholder(&mut image, fonts, item);
                }
            }
            DisplayItemKind::Box => {}
        }
    }

    image
}

fn draw_image(canvas: &mut RgbaImage, source: &RgbaImage, x: u32, y: u32, width: u32, height: u32) {
    let resized = if source.width() == width && source.height() == height {
        source.clone()
    } else {
        image::imageops::resize(source, width.max(1), height.max(1), FilterType::Triangle)
    };

    for pixel_y in 0..resized.height() {
        for pixel_x in 0..resized.width() {
            let target_x = x.saturating_add(pixel_x);
            let target_y = y.saturating_add(pixel_y);
            if target_x >= canvas.width() || target_y >= canvas.height() {
                continue;
            }
            let source_pixel = resized.get_pixel(pixel_x, pixel_y);
            blend_pixel(canvas.get_pixel_mut(target_x, target_y), *source_pixel);
        }
    }
}

fn blend_pixel(target: &mut Rgba<u8>, source: Rgba<u8>) {
    let alpha = u16::from(source[3]);
    if alpha == 255 {
        *target = source;
        return;
    }
    if alpha == 0 {
        return;
    }

    let inverse = 255u16.saturating_sub(alpha);
    for channel in 0..3 {
        target[channel] = ((u16::from(source[channel]) * alpha
            + u16::from(target[channel]) * inverse)
            / 255) as u8;
    }
    target[3] = 255;
}

fn draw_image_placeholder(canvas: &mut RgbaImage, fonts: &FontSystem, item: &DisplayItem) {
    fill_rect(
        canvas,
        item.x,
        item.y,
        item.width,
        item.height,
        [238, 238, 238, 255],
    );
    if !item.text.is_empty() {
        draw_text(
            canvas,
            fonts,
            item.x.saturating_add(2),
            item.y.saturating_add(2),
            &item.text,
            [70, 70, 70, 255],
            item.font_size.min(16),
            &item.font_families,
            item.font_weight,
            item.font_italic,
        );
    }
}

fn draw_border(
    image: &mut RgbaImage,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    border_width: u32,
    color: [u8; 4],
) {
    if width == 0 || height == 0 || border_width == 0 {
        return;
    }
    let border = border_width.min(width).min(height);
    fill_rect(image, x, y, width, border, color);
    fill_rect(
        image,
        x,
        y.saturating_add(height.saturating_sub(border)),
        width,
        border,
        color,
    );
    fill_rect(image, x, y, border, height, color);
    fill_rect(
        image,
        x.saturating_add(width.saturating_sub(border)),
        y,
        border,
        height,
        color,
    );
}

fn fill_rect(image: &mut RgbaImage, x: u32, y: u32, width: u32, height: u32, color: [u8; 4]) {
    let max_x = x.saturating_add(width).min(image.width());
    let max_y = y.saturating_add(height).min(image.height());
    for pixel_y in y.min(image.height())..max_y {
        for pixel_x in x.min(image.width())..max_x {
            image.put_pixel(pixel_x, pixel_y, Rgba(color));
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_text(
    image: &mut RgbaImage,
    fonts: &FontSystem,
    x: u32,
    y: u32,
    text: &str,
    color: [u8; 4],
    font_size: u32,
    font_families: &[String],
    font_weight: u16,
    font_italic: bool,
) {
    let spec = FontSpec::new(font_families, font_weight, font_italic, font_size);
    if let Some(run) = fonts.rasterize_text(text, &spec) {
        for glyph in run.glyphs {
            draw_raster_glyph(image, x, y, &glyph, color);
        }
        return;
    }

    draw_bitmap_fallback(image, x, y, text, color, font_size);
}

fn draw_raster_glyph(
    image: &mut RgbaImage,
    origin_x: u32,
    origin_y: u32,
    glyph: &crate::font::RasterGlyph,
    color: [u8; 4],
) {
    let base_x = i64::from(origin_x) + i64::from(glyph.x);
    let base_y = i64::from(origin_y) + i64::from(glyph.y);
    for glyph_y in 0..glyph.height {
        for glyph_x in 0..glyph.width {
            let target_x = base_x + i64::from(glyph_x);
            let target_y = base_y + i64::from(glyph_y);
            if target_x < 0
                || target_y < 0
                || target_x >= i64::from(image.width())
                || target_y >= i64::from(image.height())
            {
                continue;
            }
            let index = glyph_y as usize * glyph.width as usize + glyph_x as usize;
            let Some(coverage) = glyph.coverage.get(index).copied() else {
                continue;
            };
            if coverage == 0 {
                continue;
            }
            let source_alpha = (u16::from(color[3]) * u16::from(coverage) / 255) as u8;
            blend_pixel(
                image.get_pixel_mut(target_x as u32, target_y as u32),
                Rgba([color[0], color[1], color[2], source_alpha]),
            );
        }
    }
}

fn draw_bitmap_fallback(
    image: &mut RgbaImage,
    mut x: u32,
    y: u32,
    text: &str,
    color: [u8; 4],
    font_size: u32,
) {
    let scale = (font_size / 8).clamp(1, 6);
    for character in text.chars() {
        if let Some(glyph) = BASIC_FONTS.get(character) {
            draw_glyph(image, x, y, glyph, color, scale);
        }
        x = x.saturating_add(8 * scale);
        if x >= image.width() {
            break;
        }
    }
}

fn draw_glyph(image: &mut RgbaImage, x: u32, y: u32, glyph: [u8; 8], color: [u8; 4], scale: u32) {
    for (row, bits) in glyph.iter().enumerate() {
        for column in 0..8u32 {
            if bits & (1 << column) == 0 {
                continue;
            }
            let base_x = x.saturating_add(column.saturating_mul(scale));
            let base_y = y.saturating_add((row as u32).saturating_mul(scale));
            for dy in 0..scale {
                for dx in 0..scale {
                    let pixel_x = base_x.saturating_add(dx);
                    let pixel_y = base_y.saturating_add(dy);
                    if pixel_x < image.width() && pixel_y < image.height() {
                        image.put_pixel(pixel_x, pixel_y, Rgba(color));
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_item() -> DisplayItem {
        DisplayItem {
            kind: DisplayItemKind::Text,
            node_id: 0,
            depth: 0,
            x: 0,
            y: 0,
            width: 40,
            height: 16,
            text: "A".into(),
            color: [0, 0, 0, 255],
            background: None,
            border_width: 0,
            border_color: None,
            font_size: 16,
            font_families: vec!["sans-serif".to_string()],
            font_weight: 400,
            font_italic: false,
        }
    }

    #[test]
    fn draws_non_white_pixels_for_text() {
        let image = rasterize(&[text_item()], 64, 32);
        assert!(image.pixels().any(|pixel| pixel.0 != [255, 255, 255, 255]));
    }

    #[test]
    fn real_font_raster_path_produces_pixels_when_system_fonts_exist() {
        let fonts = FontSystem::system();
        if fonts.face_count() == 0 {
            return;
        }
        let item = text_item();
        let rendered =
            rasterize_with_images_and_fonts(&[item], 96, 40, &DecodedImages::new(), &fonts);
        assert!(rendered
            .pixels()
            .any(|pixel| pixel.0 != [255, 255, 255, 255]));
    }

    #[test]
    fn paints_box_border() {
        let item = DisplayItem {
            kind: DisplayItemKind::Box,
            node_id: 9,
            depth: 0,
            x: 2,
            y: 2,
            width: 20,
            height: 12,
            text: String::new(),
            color: [0, 0, 0, 255],
            background: Some([255, 255, 255, 255]),
            border_width: 2,
            border_color: Some([255, 0, 0, 255]),
            font_size: 16,
            font_families: vec!["sans-serif".to_string()],
            font_weight: 400,
            font_italic: false,
        };
        let rendered = rasterize(&[item], 30, 20);
        assert_eq!(rendered.get_pixel(2, 2).0, [255, 0, 0, 255]);
        assert_eq!(rendered.get_pixel(3, 10).0, [255, 0, 0, 255]);
    }

    #[test]
    fn paints_decoded_image_pixels() {
        let item = DisplayItem {
            kind: DisplayItemKind::Image,
            node_id: 7,
            depth: 0,
            x: 4,
            y: 4,
            width: 8,
            height: 8,
            text: "hero".into(),
            color: [0, 0, 0, 255],
            background: None,
            border_width: 0,
            border_color: None,
            font_size: 12,
            font_families: vec!["sans-serif".to_string()],
            font_weight: 400,
            font_italic: false,
        };
        let source = RgbaImage::from_pixel(2, 2, Rgba([200, 20, 30, 255]));
        let mut images = DecodedImages::new();
        images.insert(7, source);
        let rendered = rasterize_with_images(&[item], 20, 20, &images);
        assert_eq!(rendered.get_pixel(6, 6).0, [200, 20, 30, 255]);
    }
}
