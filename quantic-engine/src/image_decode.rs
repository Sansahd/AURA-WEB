use image::RgbaImage;

#[derive(Debug, Clone)]
pub struct DecodedImage {
    pub width: u32,
    pub height: u32,
    pub pixels: RgbaImage,
}

pub fn decode(bytes: &[u8]) -> Result<DecodedImage, image::ImageError> {
    let dynamic = image::load_from_memory(bytes)?;
    let pixels = dynamic.to_rgba8();
    Ok(DecodedImage {
        width: pixels.width(),
        height: pixels.height(),
        pixels,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_one_pixel_png() {
        const PNG: &[u8] = &[
            137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1,
            8, 6, 0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120, 156, 99, 248, 207,
            192, 240, 31, 0, 5, 0, 1, 255, 137, 153, 61, 29, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66,
            96, 130,
        ];
        let decoded = decode(PNG).unwrap();
        assert_eq!((decoded.width, decoded.height), (1, 1));
    }
}
