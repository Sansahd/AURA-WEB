use image::{Rgba, RgbaImage};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViewportCompositor {
    width: u32,
    height: u32,
}

impl ViewportCompositor {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width: width.max(1),
            height: height.max(1),
        }
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn max_scroll_y(&self, document_height: u32) -> u32 {
        document_height.saturating_sub(self.height)
    }

    pub fn compose(&self, document: &RgbaImage, scroll_y: u32) -> RgbaImage {
        let mut viewport =
            RgbaImage::from_pixel(self.width, self.height, Rgba([255, 255, 255, 255]));
        let max_scroll = self.max_scroll_y(document.height());
        let origin_y = scroll_y.min(max_scroll);
        let copy_width = self.width.min(document.width());
        let available_height = document.height().saturating_sub(origin_y);
        let copy_height = self.height.min(available_height);

        for y in 0..copy_height {
            for x in 0..copy_width {
                let pixel = *document.get_pixel(x, origin_y + y);
                viewport.put_pixel(x, y, pixel);
            }
        }

        viewport
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compositor_clamps_scroll_and_crops_document() {
        let mut document = RgbaImage::from_pixel(2, 4, Rgba([0, 0, 0, 255]));
        document.put_pixel(0, 3, Rgba([255, 0, 0, 255]));

        let compositor = ViewportCompositor::new(2, 2);
        assert_eq!(compositor.max_scroll_y(4), 2);

        let viewport = compositor.compose(&document, 999);
        assert_eq!(viewport.get_pixel(0, 1), &Rgba([255, 0, 0, 255]));
    }
}
