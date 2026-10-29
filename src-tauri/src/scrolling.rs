//! Explicit, downward-only scroll stitching. No screen access or timers live here.
use crate::imaging::{self, MAX_CAPTURE_PIXELS, MAX_EDGE};
use image::{codecs::png::PngEncoder, imageops, ExtendedColorType, ImageEncoder, RgbaImage};
use std::io::{self, Write};

type Result<T> = std::result::Result<T, String>;
const MAX_FRAMES: u32 = 100;
const NO_MATCH: &str = "没有找到可靠的重叠内容，已保留长图。请向上滚回一些，保留至少三分之一原画面，等页面静止后再采集；选区请避开动态内容。";

pub struct ScrollImage {
    image: RgbaImage,
    previous: RgbaImage,
    frames: u32,
}
impl ScrollImage {
    pub fn new(image: RgbaImage) -> Result<Self> {
        Self::new_with_png_limit(image, imaging::MAX_BYTES)
    }
    fn new_with_png_limit(image: RgbaImage, byte_limit: usize) -> Result<Self> {
        if image.width() < 64 || image.height() < 64 {
            return Err("长截图选区至少需要 64 × 64 像素".into());
        }
        validate_dimensions(image.width(), image.height())?;
        validate_png_bytes(&image, byte_limit)?;
        Ok(Self {
            previous: image.clone(),
            image,
            frames: 1,
        })
    }
    pub fn image(&self) -> &RgbaImage {
        &self.image
    }
    pub fn frames(&self) -> u32 {
        self.frames
    }
    /// Returns false for an unchanged view. Every failure leaves both images intact.
    pub fn append(&mut self, next: RgbaImage) -> Result<bool> {
        self.append_with_png_limit(next, imaging::MAX_BYTES)
    }
    fn append_with_png_limit(&mut self, next: RgbaImage, byte_limit: usize) -> Result<bool> {
        if self.previous.dimensions() != next.dimensions() {
            return Err("采集区域尺寸已变化，请重新开始长截图".into());
        }
        if unchanged(&self.previous, &next) {
            return Ok(false);
        }
        if self.frames >= MAX_FRAMES {
            return Err("已达到 100 屏上限，请先保存当前长图".into());
        }
        let (shift, footer) = overlap(&self.previous, &next)?;
        let height = self.image.height().checked_add(shift).ok_or("长图过高")?;
        validate_dimensions(next.width(), height)?;
        let mut joined = RgbaImage::new(next.width(), height);
        // The old sticky footer is replaced by new content and the latest footer.
        imageops::replace(&mut joined, &self.image, 0, 0);
        let tail = imageops::crop_imm(
            &next,
            0,
            next.height() - footer - shift,
            next.width(),
            shift + footer,
        )
        .to_image();
        imageops::replace(
            &mut joined,
            &tail,
            0,
            i64::from(self.image.height() - footer),
        );
        // A valid pixel count does not guarantee the PNG will fit the export IPC
        // budget. Validate before committing, so every retained result can finish.
        validate_png_bytes(&joined, byte_limit)?;
        self.image = joined;
        self.previous = next;
        self.frames += 1;
        Ok(true)
    }
}
// Encode the borrowed pixels with the same defaults as imaging::png_data_url.
// Discard the output and fail as soon as its byte budget is exhausted; no second
// full image, PNG buffer, or base64 string is retained for this preflight.
fn validate_png_bytes(image: &RgbaImage, byte_limit: usize) -> Result<()> {
    struct Budget {
        remaining: usize,
        exceeded: bool,
    }
    impl Write for Budget {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if bytes.len() > self.remaining {
                self.exceeded = true;
                return Err(io::Error::new(
                    io::ErrorKind::FileTooLarge,
                    "PNG byte budget exceeded",
                ));
            }
            self.remaining -= bytes.len();
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let mut budget = Budget {
        remaining: byte_limit,
        exceeded: false,
    };
    let encoded = PngEncoder::new(&mut budget).write_image(
        image.as_raw(),
        image.width(),
        image.height(),
        ExtendedColorType::Rgba8,
    );
    if budget.exceeded {
        return Err("长图的 PNG 文件超过 64 MiB 上限，已保留原有内容。请保存当前结果。".into());
    }
    encoded.map_err(|error| format!("长图 PNG 编码失败，已保留原有内容：{error}"))
}

fn validate_dimensions(width: u32, height: u32) -> Result<()> {
    if width > MAX_EDGE
        || height > MAX_EDGE
        || u64::from(width) * u64::from(height) > MAX_CAPTURE_PIXELS
    {
        return Err(
            "长图已达到上限：每边最多 16384 像素、总计 4000 万像素。请先保存当前结果。".into(),
        );
    }
    imaging::validate_size(width, height)
}
fn difference(a: &[u8; 4], b: &[u8; 4]) -> u32 {
    (0..3).map(|i| u32::from(a[i].abs_diff(b[i]))).sum()
}
fn row_difference(a: &RgbaImage, b: &RgbaImage, ay: u32, by: u32, columns: u32) -> f64 {
    let mut total = 0_u64;
    for i in 0..columns {
        // Exclude the outer 3% (scrollbars and thin frame borders).
        let x = a.width() * (3 * columns + 94 * i) / (100 * columns);
        total += u64::from(difference(&a.get_pixel(x, ay).0, &b.get_pixel(x, by).0));
    }
    total as f64 / f64::from(columns * 3)
}
fn unchanged(a: &RgbaImage, b: &RgbaImage) -> bool {
    let mut changed = 0;
    for i in 0..128 {
        let y = (a.height() - 1) * i / 127;
        if row_difference(a, b, y, y, 64) > 0.8 {
            changed += 1;
        }
    }
    changed <= 1
}
fn score(
    a: &RgbaImage,
    b: &RgbaImage,
    top: u32,
    bottom: u32,
    shift: u32,
    rows: u32,
    columns: u32,
) -> f64 {
    let count = bottom - top - shift;
    let samples = count.min(rows);
    let mut total = 0.0;
    for i in 0..samples {
        let y = top + (count - 1) * i / (samples - 1).max(1);
        total += row_difference(a, b, y + shift, y, columns);
    }
    total / f64::from(samples)
}
fn overlap(a: &RgbaImage, b: &RgbaImage) -> Result<(u32, u32)> {
    let height = a.height();
    let mut top = 0;
    while top < height / 3 && row_difference(a, b, top, top, 64) <= 0.6 {
        top += 1;
    }
    let mut footer = 0;
    while footer < height / 3
        && row_difference(a, b, height - footer - 1, height - footer - 1, 64) <= 0.6
    {
        footer += 1;
    }
    let bottom = height - footer;
    let min_overlap = 32.max((bottom - top) / 3);
    let max_shift = (bottom - top).saturating_sub(min_overlap);
    let informative: Vec<_> = (top..bottom - 1)
        .filter(|y| row_difference(b, b, *y, *y + 1, 32) > 2.0)
        .collect();
    let mut candidates = Vec::new();
    // ponytail: bounded sampled search, then dense validation; reject ambiguous/repeated
    // patterns instead of adding a computer-vision dependency or guessing a seam.
    for shift in 1..=max_shift {
        // Search on rows that carry vertical information so a sparse document's
        // white margins cannot crowd the real seam out of the candidate budget.
        let count = informative.partition_point(|y| *y < bottom - shift);
        if count < 8 {
            continue;
        }
        let samples = count.min(32);
        let value = (0..samples)
            .map(|i| {
                let y = informative[(count - 1) * i / (samples - 1)];
                row_difference(a, b, y + shift, y, 32)
            })
            .sum::<f64>()
            / samples as f64;
        if value <= 8.0 {
            candidates.push((shift, value));
        }
    }
    candidates.sort_by(|a, b| a.1.total_cmp(&b.1));
    candidates.truncate(16);
    let mut matches = Vec::new();
    for (shift, _) in candidates {
        let value = score(a, b, top, bottom, shift, 1024, 64);
        if value <= 2.0 {
            matches.push((shift, value));
        }
    }
    matches.sort_by(|a, b| a.1.total_cmp(&b.1));
    let &(shift, best) = matches.first().ok_or(NO_MATCH)?;
    if matches
        .iter()
        .skip(1)
        .any(|(other, value)| other.abs_diff(shift) > 2 && *value <= best + 0.75)
    {
        return Err(NO_MATCH.into());
    }
    // A white area or a vertical stripe cannot prove the scroll distance.
    let overlap = bottom - top - shift;
    let textured = (top..top + overlap - 1)
        .filter(|y| row_difference(b, b, *y, *y + 1, 32) > 2.0)
        .count();
    if textured < 8 {
        return Err(NO_MATCH.into());
    }
    // Check every row, not only the search samples; animated islands and horizontal
    // motion must not silently enter a guessed overlap.
    let bad_rows = (top..bottom - shift)
        .filter(|y| row_difference(a, b, *y + shift, *y, 64) > 8.0)
        .count();
    if bad_rows * 100 > overlap as usize * 3 {
        return Err(NO_MATCH.into());
    }
    Ok((shift, footer))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;
    fn page() -> RgbaImage {
        RgbaImage::from_fn(128, 900, |x, y| {
            let hash = x
                .wrapping_mul(77)
                .wrapping_add(y.wrapping_mul(137))
                .wrapping_mul(y + 97);
            Rgba([
                (hash % 251) as u8,
                ((hash / 3) % 247) as u8,
                ((hash / 7) % 239) as u8,
                255,
            ])
        })
    }
    fn viewport(page: &RgbaImage, y: u32) -> RgbaImage {
        imageops::crop_imm(page, 0, y, 128, 240).to_image()
    }
    #[test]
    fn overlap_stitches_exact_original_pixels_and_duplicate_is_noop() {
        let original = page();
        let mut scroll = ScrollImage::new(viewport(&original, 0)).unwrap();
        assert!(!scroll.append(viewport(&original, 0)).unwrap());
        assert!(scroll.append(viewport(&original, 120)).unwrap());
        assert!(scroll.append(viewport(&original, 240)).unwrap());
        assert_eq!(scroll.frames(), 3);
        assert_eq!(
            scroll.image(),
            &imageops::crop_imm(&original, 0, 0, 128, 480).to_image()
        );
    }
    #[test]
    fn keeps_sticky_header_and_footer_once() {
        let original = page();
        let sticky = |offset: u32| {
            let mut frame = RgbaImage::from_pixel(128, 240, Rgba([31, 41, 51, 255]));
            imageops::replace(
                &mut frame,
                &imageops::crop_imm(&original, 0, offset, 128, 190).to_image(),
                0,
                30,
            );
            frame
        };
        let mut scroll = ScrollImage::new(sticky(0)).unwrap();
        assert!(scroll.append(sticky(100)).unwrap());
        assert!(scroll.append(sticky(200)).unwrap());
        assert_eq!(scroll.image().height(), 440);
        assert_eq!(
            imageops::crop_imm(scroll.image(), 0, 30, 128, 390).to_image(),
            imageops::crop_imm(&original, 0, 0, 128, 390).to_image()
        );
        assert_eq!(scroll.image().get_pixel(10, 439), &Rgba([31, 41, 51, 255]));
    }
    #[test]
    fn rejects_missing_overlap_reverse_scroll_or_resize_and_preserves_result() {
        let original = page();
        let mut scroll = ScrollImage::new(viewport(&original, 120)).unwrap();
        let before = scroll.image().clone();
        assert!(scroll.append(viewport(&original, 600)).is_err());
        assert!(scroll.append(viewport(&original, 0)).is_err());
        assert!(scroll.append(RgbaImage::new(64, 64)).is_err());
        assert_eq!(scroll.image(), &before);
        assert_eq!(scroll.frames(), 1);
    }
    #[test]
    fn sparse_document_rows_match_without_guessing_in_white_margins() {
        let mut original = RgbaImage::from_pixel(128, 900, Rgba([250, 250, 250, 255]));
        for line in 0..40_u32 {
            let y = line * 22 + 8;
            let length = 30 + (line * 47) % 80;
            for py in y..y + 3 {
                for x in 12..12 + length {
                    original.put_pixel(x, py, Rgba([30, 40, 50, 255]));
                }
            }
        }
        let mut scroll = ScrollImage::new(viewport(&original, 0)).unwrap();
        assert!(scroll.append(viewport(&original, 101)).unwrap());
        assert_eq!(
            scroll.image(),
            &imageops::crop_imm(&original, 0, 0, 128, 341).to_image()
        );
    }
    #[test]
    fn animated_overlap_is_rejected_without_mutating_existing_pixels() {
        let original = page();
        let mut scroll = ScrollImage::new(viewport(&original, 0)).unwrap();
        let mut changed = viewport(&original, 110);
        for y in 40..90 {
            for x in 20..110 {
                changed.put_pixel(x, y, Rgba([255, 255, 255, 255]));
            }
        }
        assert!(scroll.append(changed).is_err());
        assert_eq!(scroll.image(), &viewport(&original, 0));
        assert_eq!(scroll.frames(), 1);
    }
    #[test]
    fn png_budget_matches_export_and_rejects_new_content_without_mutation() {
        let original = page();
        let first = viewport(&original, 0);
        // Measure the real finish-path encoding, not a raw-pixel size estimate.
        let mut encoded = io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(first.clone())
            .write_to(&mut encoded, image::ImageFormat::Png)
            .unwrap();
        let limit = encoded.get_ref().len();
        assert!(validate_png_bytes(&first, limit).is_ok());
        assert!(validate_png_bytes(&first, limit - 1).is_err());
        assert!(ScrollImage::new_with_png_limit(first.clone(), limit - 1).is_err());
        let mut scroll = ScrollImage::new_with_png_limit(first.clone(), limit).unwrap();
        let error = scroll
            .append_with_png_limit(viewport(&original, 120), limit)
            .unwrap_err();
        assert!(error.contains("PNG"));
        assert_eq!(scroll.image(), &first);
        assert_eq!(scroll.previous, first);
        assert_eq!(scroll.frames(), 1);
        // A rejected append must not move the overlap anchor either.
        assert!(scroll.append(viewport(&original, 120)).unwrap());
        assert_eq!(
            scroll.image(),
            &imageops::crop_imm(&original, 0, 0, 128, 360).to_image()
        );
        assert_eq!(scroll.frames(), 2);
    }
    #[test]
    fn refuses_repeating_pattern_and_checks_limits_before_allocation() {
        let stripe = |offset: u32| {
            RgbaImage::from_fn(128, 240, |_, y| {
                Rgba([(((y + offset) % 20) * 12) as u8, 0, 0, 255])
            })
        };
        let mut scroll = ScrollImage::new(stripe(0)).unwrap();
        assert!(scroll.append(stripe(7)).is_err());
        assert!(validate_dimensions(4000, 10_001).is_err());
        assert!(validate_dimensions(64, 16_385).is_err());
        assert!(ScrollImage::new(RgbaImage::new(63, 64)).is_err());
        scroll.frames = MAX_FRAMES;
        assert!(scroll.append(stripe(8)).is_err());
    }
}
