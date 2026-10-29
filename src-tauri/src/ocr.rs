//! Explicit local OCR. Pixels and recognized text stay in memory and are never logged.
use crate::imaging;
use image::{imageops::FilterType, DynamicImage};
use serde::{Deserialize, Serialize};

const MAX_OCR_EDGE: u32 = 4096;
const MAX_LINES: usize = 2000;
const MAX_TEXT_BYTES: usize = 200_000;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OcrLine {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Serialize)]
pub struct OcrResult {
    pub width: u32,
    pub height: u32,
    pub lines: Vec<OcrLine>,
}

/// Call on a blocking worker only, after main-window/update-state authorization.
pub fn recognize(data_url: &str) -> Result<OcrResult, String> {
    let source = imaging::decode_data_url(data_url)?;
    imaging::validate_capture_size(source.width(), source.height())?;
    let (width, height) = (source.width(), source.height());
    let prepared = prepare_image(source, MAX_OCR_EDGE);
    let (ocr_width, ocr_height) = (prepared.width(), prepared.height());
    let lines = platform_recognize(&prepared)?;
    Ok(OcrResult {
        width,
        height,
        lines: checked_lines(lines, ocr_width, ocr_height, width, height)?,
    })
}

fn prepare_image(source: DynamicImage, max_edge: u32) -> DynamicImage {
    let mut rgba = if source.width().max(source.height()) > max_edge {
        source
            .resize(max_edge, max_edge, FilterType::Lanczos3)
            .into_rgba8()
    } else {
        source.into_rgba8()
    };
    for pixel in rgba.pixels_mut() {
        let alpha = u32::from(pixel[3]);
        for channel in &mut pixel.0[..3] {
            *channel = ((u32::from(*channel) * alpha + 255 * (255 - alpha) + 127) / 255) as u8;
        }
        pixel[3] = 255;
    }
    DynamicImage::ImageRgba8(rgba)
}

fn checked_lines(
    lines: Vec<OcrLine>,
    ocr_width: u32,
    ocr_height: u32,
    width: u32,
    height: u32,
) -> Result<Vec<OcrLine>, String> {
    if lines.len() > MAX_LINES
        || lines.iter().map(|line| line.text.len()).sum::<usize>() > MAX_TEXT_BYTES
    {
        return Err("图片文字过多，请裁剪后分区域识别".into());
    }
    let scale_x = f64::from(width) / f64::from(ocr_width);
    let scale_y = f64::from(height) / f64::from(ocr_height);
    let mut result = Vec::new();
    for line in lines {
        if line.text.trim().is_empty() {
            continue;
        }
        if ![line.x, line.y, line.width, line.height]
            .into_iter()
            .all(f64::is_finite)
            || line.width <= 0.0
            || line.height <= 0.0
        {
            return Err("系统文字识别返回了无效位置，请重试或手动打码".into());
        }
        // Include a small edge around the OCR glyph box; never invent character boxes.
        let x = (line.x * scale_x - 2.0)
            .floor()
            .clamp(0.0, f64::from(width));
        let y = (line.y * scale_y - 2.0)
            .floor()
            .clamp(0.0, f64::from(height));
        let right = ((line.x + line.width) * scale_x + 2.0)
            .ceil()
            .clamp(0.0, f64::from(width));
        let bottom = ((line.y + line.height) * scale_y + 2.0)
            .ceil()
            .clamp(0.0, f64::from(height));
        if right > x && bottom > y {
            result.push(OcrLine {
                text: line.text,
                x,
                y,
                width: right - x,
                height: bottom - y,
            });
        }
    }
    Ok(result)
}

#[cfg(all(feature = "desktop", target_os = "macos"))]
fn platform_recognize(image: &DynamicImage) -> Result<Vec<OcrLine>, String> {
    use image::ImageFormat;
    use objc2::{rc::autoreleasepool, AnyThread};
    use objc2_foundation::{NSArray, NSData, NSDictionary, NSString};
    use objc2_vision::{
        VNImageRequestHandler, VNRecognizeTextRequest, VNRequest, VNRequestTextRecognitionLevel,
    };
    use std::io::Cursor;

    autoreleasepool(|_| {
        let mut encoded = Cursor::new(Vec::new());
        image
            .write_to(&mut encoded, ImageFormat::Png)
            .map_err(|_| "无法准备本地文字识别图片")?;
        let data = NSData::with_bytes(encoded.get_ref());
        let handler = VNImageRequestHandler::initWithData_options(
            VNImageRequestHandler::alloc(),
            &data,
            &NSDictionary::new(),
        );
        let request = VNRecognizeTextRequest::new();
        request.setRecognitionLevel(VNRequestTextRecognitionLevel::Accurate);
        // Do not autocorrect passwords, tokens, or addresses into dictionary words.
        request.setUsesLanguageCorrection(false);
        let supported = unsafe { request.supportedRecognitionLanguagesAndReturnError() }
            .map_err(|_| "系统文字识别语言不可用，请检查 macOS 版本")?;
        let languages: Vec<_> = ["zh-Hans", "zh-Hant", "en-US"]
            .into_iter()
            .filter(|language| supported.iter().any(|item| item.to_string() == *language))
            .map(NSString::from_str)
            .collect();
        if languages.is_empty() {
            return Err("系统未提供中文或英文文字识别".into());
        }
        request.setRecognitionLanguages(&NSArray::from_retained_slice(&languages));
        let requests: [&VNRequest; 1] = [&request];
        handler
            .performRequests_error(&NSArray::from_slice(&requests))
            .map_err(|_| "本地文字识别失败，请重试或裁剪为更小区域")?;
        let mut lines = Vec::new();
        if let Some(observations) = request.results() {
            if observations.len() > MAX_LINES {
                return Err("图片文字过多，请裁剪后分区域识别".into());
            }
            for observation in observations {
                let candidates = observation.topCandidates(1);
                let Some(candidate) = candidates.firstObject() else {
                    continue;
                };
                let bounds = unsafe { observation.boundingBox() };
                lines.push(OcrLine {
                    text: candidate.string().to_string(),
                    x: bounds.origin.x * f64::from(image.width()),
                    y: (1.0 - bounds.origin.y - bounds.size.height) * f64::from(image.height()),
                    width: bounds.size.width * f64::from(image.width()),
                    height: bounds.size.height * f64::from(image.height()),
                });
            }
        }
        Ok(lines)
    })
}

#[cfg(all(feature = "desktop", target_os = "windows"))]
fn platform_recognize(image: &DynamicImage) -> Result<Vec<OcrLine>, String> {
    worker::recognize(image)
}

#[cfg(not(all(feature = "desktop", any(target_os = "macos", target_os = "windows"))))]
fn platform_recognize(_: &DynamicImage) -> Result<Vec<OcrLine>, String> {
    Err("本地文字识别需要 macOS 或 Windows 桌面版".into())
}

#[cfg(all(feature = "desktop", target_os = "windows"))]
mod offline;
#[cfg(all(feature = "desktop", target_os = "windows"))]
mod worker;

/// Must run before Tauri initializes its single-instance and window state.
/// The child receives an explicit image on stdin; it cannot capture the desktop.
pub fn run_worker_if_requested() -> bool {
    #[cfg(all(feature = "desktop", target_os = "windows"))]
    if std::env::args().nth(1).as_deref() == Some("--starshot-ocr-worker") {
        worker::run();
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaled_boxes_cover_original_coordinates_and_clip_at_edges() {
        let lines = checked_lines(
            vec![OcrLine {
                text: "EXAMPLE".into(),
                x: 10.25,
                y: 20.25,
                width: 30.5,
                height: 10.5,
            }],
            100,
            100,
            200,
            300,
        )
        .unwrap();
        assert_eq!(
            (lines[0].x, lines[0].y, lines[0].width, lines[0].height),
            (18.0, 58.0, 66.0, 37.0)
        );
        let edge = checked_lines(
            vec![OcrLine {
                text: "EDGE".into(),
                x: -1.0,
                y: 98.0,
                width: 8.0,
                height: 9.0,
            }],
            100,
            100,
            100,
            100,
        )
        .unwrap();
        assert_eq!(
            (edge[0].x, edge[0].y, edge[0].width, edge[0].height),
            (0.0, 96.0, 9.0, 4.0)
        );
    }

    #[test]
    fn rejects_invalid_results_and_large_input() {
        assert!(checked_lines(
            vec![OcrLine {
                text: "EXAMPLE".into(),
                x: f64::NAN,
                y: 0.0,
                width: 10.0,
                height: 10.0
            }],
            100,
            100,
            100,
            100
        )
        .is_err());
        assert!(recognize("data:image/png;base64,broken").is_err());
        assert!(checked_lines(
            vec![OcrLine {
                text: "x".repeat(MAX_TEXT_BYTES + 1),
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 10.0
            }],
            100,
            100,
            100,
            100
        )
        .is_err());
    }

    #[test]
    fn transparency_is_composited_in_memory_and_size_is_bounded() {
        let source = DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            8,
            4,
            image::Rgba([20, 40, 60, 0]),
        ));
        let prepared = prepare_image(source, 4).to_rgba8();
        assert_eq!(prepared.dimensions(), (4, 2));
        assert!(prepared
            .pixels()
            .all(|pixel| pixel.0 == [255, 255, 255, 255]));
    }

    #[cfg(all(feature = "desktop", target_os = "macos"))]
    #[test]
    fn vision_recognizes_generated_fixture() {
        let image =
            image::load_from_memory(include_bytes!("../../tests/fixtures/ocr-latin.png")).unwrap();
        let result = recognize(&imaging::png_data_url(&image).unwrap()).unwrap();
        assert_eq!((result.width, result.height), (1200, 500));
        assert!(result
            .lines
            .iter()
            .any(|line| line.text.contains("1234567890")));
        assert!(result
            .lines
            .iter()
            .any(|line| line.text.contains("demo@example.test")));
        assert!(result.lines.iter().all(|line| line.x >= 0.0
            && line.y >= 0.0
            && line.x + line.width <= 1200.0
            && line.y + line.height <= 500.0));
    }
}
