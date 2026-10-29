use base64::{engine::general_purpose::STANDARD, Engine};
use image::{DynamicImage, ImageFormat, ImageReader};
use serde::{Deserialize, Serialize};
use std::{
    io::{Cursor, Write},
    path::Path,
};

pub const MAX_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_PIXELS: u64 = 64_000_000;
pub const MAX_CAPTURE_PIXELS: u64 = 40_000_000;
pub const MAX_EDGE: u32 = 16_384;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    pub fn validate(&self, width: u32, height: u32) -> Result<(), String> {
        if self.width == 0
            || self.height == 0
            || self.x.checked_add(self.width).is_none_or(|x| x > width)
            || self.y.checked_add(self.height).is_none_or(|y| y > height)
        {
            return Err("截图选区超出画面，请重新选择".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImagePayload {
    pub data_url: String,
    pub width: u32,
    pub height: u32,
    pub name: String,
}

pub fn validate_size(width: u32, height: u32) -> Result<(), String> {
    if width == 0
        || height == 0
        || width > MAX_EDGE
        || height > MAX_EDGE
        || u64::from(width) * u64::from(height) > MAX_PIXELS
    {
        return Err("图片尺寸过大；每边最多 16384 像素，总面积最多 6400 万像素".into());
    }
    Ok(())
}

pub fn validate_capture_size(width: u32, height: u32) -> Result<(), String> {
    validate_size(width, height)?;
    if u64::from(width) * u64::from(height) > MAX_CAPTURE_PIXELS {
        return Err("单块屏幕截图最多支持 4000 万像素".into());
    }
    Ok(())
}

pub fn atomic_save(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if bytes.len() > MAX_BYTES {
        return Err("图片文件超过 64 MB".into());
    }
    let parent = path.parent().ok_or("保存位置无效")?;
    let mut pending = tempfile::NamedTempFile::new_in(parent)
        .map_err(|e| format!("无法创建临时导出文件：{e}"))?;
    pending
        .write_all(bytes)
        .map_err(|e| format!("写入图片失败，原文件未改动：{e}"))?;
    pending
        .as_file()
        .sync_all()
        .map_err(|e| format!("写入图片失败，原文件未改动：{e}"))?;
    pending
        .persist(path)
        .map_err(|e| format!("替换图片失败，原文件未改动：{}", e.error))?;
    Ok(())
}

pub fn decode_data_url(data: &str) -> Result<DynamicImage, String> {
    if data.len() > MAX_BYTES * 4 / 3 + 128 {
        return Err("图片文件超过 64 MB".into());
    }
    let (encoded, expected_format) =
        if let Some(encoded) = data.strip_prefix("data:image/png;base64,") {
            (encoded, ImageFormat::Png)
        } else if let Some(encoded) = data.strip_prefix("data:image/jpeg;base64,") {
            (encoded, ImageFormat::Jpeg)
        } else {
            return Err("仅支持 PNG 或 JPEG 图片".into());
        };
    let bytes = STANDARD.decode(encoded).map_err(|_| "图片编码无效")?;
    if bytes.len() > MAX_BYTES {
        return Err("图片文件超过 64 MB".into());
    }
    let reader = ImageReader::new(Cursor::new(&bytes))
        .with_guessed_format()
        .map_err(|_| "无法识别图片")?;
    if reader.format() != Some(expected_format) {
        return Err("图片内容与声明的 PNG / JPEG 格式不一致".into());
    }
    let (width, height) = reader.into_dimensions().map_err(|_| "图片已损坏")?;
    validate_size(width, height)?;
    let mut reader = ImageReader::new(Cursor::new(&bytes))
        .with_guessed_format()
        .map_err(|_| "无法识别图片")?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_EDGE);
    limits.max_image_height = Some(MAX_EDGE);
    limits.max_alloc = Some(MAX_PIXELS * 8);
    reader.limits(limits);
    reader.decode().map_err(|_| "图片解码失败".to_string())
}

pub fn png_data_url(image: &DynamicImage) -> Result<String, String> {
    validate_size(image.width(), image.height())?;
    let mut out = Cursor::new(Vec::new());
    image
        .write_to(&mut out, ImageFormat::Png)
        .map_err(|e| format!("PNG 编码失败：{e}"))?;
    if out.get_ref().len() > MAX_BYTES {
        return Err("图片文件超过 64 MB".into());
    }
    Ok(format!(
        "data:image/png;base64,{}",
        STANDARD.encode(out.into_inner())
    ))
}

pub fn payload(image: &DynamicImage, name: String) -> Result<ImagePayload, String> {
    Ok(ImagePayload {
        data_url: png_data_url(image)?,
        width: image.width(),
        height: image.height(),
        name,
    })
}

// Match the pin's CSS rotate(...deg) scaleX(...): flip the source first,
// then rotate clockwise. Window size and preview opacity never affect pixels.
pub fn orient_pin(
    image: DynamicImage,
    rotation: u16,
    flipped: bool,
) -> Result<DynamicImage, String> {
    if ![0, 90, 180, 270].contains(&rotation) {
        return Err("贴图方向应为 0°、90°、180° 或 270°".into());
    }
    let image = if flipped { image.fliph() } else { image };
    Ok(match rotation {
        90 => image.rotate90(),
        180 => image.rotate180(),
        270 => image.rotate270(),
        _ => image,
    })
}

// The overlay may supply its flattened annotations, but never a different-sized image
// or a rectangle outside the frozen frame owned by that window.
pub fn capture_selection(
    frame: &DynamicImage,
    rect: Rect,
    annotated_png: Option<&str>,
) -> Result<DynamicImage, String> {
    rect.validate(frame.width(), frame.height())?;
    if let Some(data) = annotated_png {
        if !data.starts_with("data:image/png;base64,") {
            return Err("截图标注只接受 PNG 图片".into());
        }
        let image = decode_data_url(data)?;
        if image.width() != rect.width || image.height() != rect.height {
            return Err("标注图片尺寸与截图选区不一致，请重试".into());
        }
        Ok(image)
    } else {
        Ok(frame.crop_imm(rect.x, rect.y, rect.width, rect.height))
    }
}

pub fn filename(suggested: &str) -> String {
    let clean: String = suggested
        .chars()
        .filter(|c| !c.is_control() && !"/\\:*?\"<>|".contains(*c))
        .take(100)
        .collect();
    let clean = clean.trim().trim_matches('.');
    if clean.is_empty() {
        "星镜截图.png".into()
    } else {
        clean.into()
    }
}

// x/y/window sizes are in the capture backend's common coordinate system.
// Scale is 2 on a Retina display whose window coordinates use macOS points.
pub fn intersect_region(
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    origin_x: i32,
    origin_y: i32,
    scale: f64,
    image_width: u32,
    image_height: u32,
) -> Option<Rect> {
    if !scale.is_finite() || scale <= 0.0 {
        return None;
    }
    let left = ((f64::from(x) - f64::from(origin_x)) * scale)
        .round()
        .clamp(0.0, f64::from(image_width));
    let top = ((f64::from(y) - f64::from(origin_y)) * scale)
        .round()
        .clamp(0.0, f64::from(image_height));
    let right = ((f64::from(x) + f64::from(width) - f64::from(origin_x)) * scale)
        .round()
        .clamp(0.0, f64::from(image_width));
    let bottom = ((f64::from(y) + f64::from(height) - f64::from(origin_y)) * scale)
        .round()
        .clamp(0.0, f64::from(image_height));
    if right - left < 8.0 || bottom - top < 8.0 {
        return None;
    }
    Some(Rect {
        x: left as u32,
        y: top as u32,
        width: (right - left) as u32,
        height: (bottom - top) as u32,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pin_exports_preserve_source_pixels_alpha_and_display_orientation() {
        let source = DynamicImage::ImageRgba8(image::RgbaImage::from_fn(3, 2, |x, y| {
            let id = (y * 3 + x + 1) as u8;
            image::Rgba([id, 20, 30, id * 40])
        }));
        for (rotation, flipped, size, expected) in [
            (0, false, (3, 2), vec![1, 2, 3, 4, 5, 6]),
            (0, true, (3, 2), vec![3, 2, 1, 6, 5, 4]),
            (90, false, (2, 3), vec![4, 1, 5, 2, 6, 3]),
            (90, true, (2, 3), vec![6, 3, 5, 2, 4, 1]),
            (180, false, (3, 2), vec![6, 5, 4, 3, 2, 1]),
            (270, false, (2, 3), vec![3, 6, 2, 5, 1, 4]),
        ] {
            let result = orient_pin(source.clone(), rotation, flipped)
                .unwrap()
                .into_rgba8();
            assert_eq!(result.dimensions(), size);
            assert_eq!(result.pixels().map(|p| p[0]).collect::<Vec<_>>(), expected);
            assert!(result.pixels().all(|p| p[3] == p[0] * 40));
        }
        assert!(orient_pin(source, 45, false).is_err());
    }
    #[test]
    fn png_roundtrip_and_crop_use_original_pixels() {
        let mut rgba = image::RgbaImage::new(4, 3);
        rgba.put_pixel(2, 1, image::Rgba([22, 44, 66, 255]));
        let image = DynamicImage::ImageRgba8(rgba);
        let decoded = decode_data_url(&png_data_url(&image).unwrap()).unwrap();
        let rect = Rect {
            x: 2,
            y: 1,
            width: 1,
            height: 1,
        };
        rect.validate(decoded.width(), decoded.height()).unwrap();
        assert_eq!(
            decoded
                .crop_imm(rect.x, rect.y, rect.width, rect.height)
                .to_rgba8()
                .get_pixel(0, 0)
                .0,
            [22, 44, 66, 255]
        );
    }
    #[test]
    fn quick_capture_uses_exact_crop_or_matching_flattened_png() {
        let frame = DynamicImage::ImageRgba8(image::RgbaImage::from_fn(8, 6, |x, y| {
            image::Rgba([x as u8, y as u8, 127, 255])
        }));
        let rect = Rect {
            x: 2,
            y: 1,
            width: 3,
            height: 2,
        };
        let cropped = capture_selection(&frame, rect, None).unwrap().into_rgba8();
        assert_eq!(cropped.dimensions(), (3, 2));
        assert_eq!(cropped.get_pixel(0, 0).0, [2, 1, 127, 255]);
        assert_eq!(cropped.get_pixel(2, 1).0, [4, 2, 127, 255]);
        let mask = DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            3,
            2,
            image::Rgba([0, 0, 0, 255]),
        ));
        let png = png_data_url(&mask).unwrap();
        assert_eq!(
            capture_selection(&frame, rect, Some(&png))
                .unwrap()
                .into_rgba8(),
            mask.into_rgba8()
        );
        let wrong = png_data_url(&DynamicImage::new_rgba8(3, 3)).unwrap();
        assert!(capture_selection(&frame, rect, Some(&wrong)).is_err());
        assert!(capture_selection(&frame, Rect { x: 7, ..rect }, Some(&png)).is_err());
        assert!(capture_selection(&frame, Rect { width: 0, ..rect }, None).is_err());
        assert!(capture_selection(&frame, rect, Some("data:image/jpeg;base64,broken")).is_err());
        assert!(capture_selection(&frame, rect, Some("data:image/png;base64,broken")).is_err());
        let mut jpeg = Cursor::new(Vec::new());
        DynamicImage::new_rgb8(rect.width, rect.height)
            .write_to(&mut jpeg, ImageFormat::Jpeg)
            .unwrap();
        let mislabeled = format!(
            "data:image/png;base64,{}",
            STANDARD.encode(jpeg.into_inner())
        );
        assert!(capture_selection(&frame, rect, Some(&mislabeled)).is_err());
    }
    #[test]
    fn rejects_corrupt_oversized_and_out_of_bounds() {
        assert!(decode_data_url("data:image/svg+xml;base64,PHN2Zz4=").is_err());
        assert!(decode_data_url("data:image/png;base64,broken").is_err());
        assert!(validate_size(10_000, 10_000).is_err());
        assert!(Rect {
            x: u32::MAX,
            y: 0,
            width: 2,
            height: 1
        }
        .validate(400, 400)
        .is_err());
        assert!(Rect {
            x: 0,
            y: 0,
            width: 0,
            height: 1
        }
        .validate(400, 400)
        .is_err());
    }
    #[test]
    fn region_conversion_handles_retina_and_negative_monitor_origins() {
        assert_eq!(
            intersect_region(-1900, 20, 400, 200, -1920, 0, 2.0, 3840, 2160),
            Some(Rect {
                x: 40,
                y: 40,
                width: 800,
                height: 400
            })
        );
        assert_eq!(
            intersect_region(-2000, -50, 200, 100, -1920, 0, 1.0, 1920, 1080),
            Some(Rect {
                x: 0,
                y: 0,
                width: 120,
                height: 50
            })
        );
        assert!(intersect_region(3000, 20, 40, 50, 0, 0, 1.0, 1920, 1080).is_none());
    }
    #[test]
    fn export_limit_and_capture_limit_are_distinct() {
        assert!(validate_size(8000, 8000).is_ok());
        assert!(validate_size(8001, 8000).is_err());
        assert!(validate_capture_size(8000, 5000).is_ok());
        assert!(validate_capture_size(8000, 8000).is_err());
    }
    #[test]
    fn atomic_export_replaces_successfully_and_preserves_on_failure() {
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("image.png");
        std::fs::write(&destination, b"original").unwrap();
        atomic_save(&destination, b"replacement").unwrap();
        assert_eq!(std::fs::read(&destination).unwrap(), b"replacement");
        let occupied = directory.path().join("occupied");
        std::fs::create_dir(&occupied).unwrap();
        std::fs::write(occupied.join("keep"), b"original").unwrap();
        assert!(atomic_save(&occupied, b"new").is_err());
        assert_eq!(std::fs::read(occupied.join("keep")).unwrap(), b"original");
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 2);
    }
    #[test]
    fn safe_suggested_names() {
        assert_eq!(filename("../../photo.png"), "photo.png");
        assert_eq!(filename("../"), "星镜截图.png");
    }
}
