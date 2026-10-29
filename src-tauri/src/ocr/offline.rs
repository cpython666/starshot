//! CPU-only Latin text OCR used by the short-lived Windows worker.
use super::{OcrLine, MAX_LINES};
use image::DynamicImage;
use ocrs::{ImageSource, OcrEngine, OcrEngineParams, TextItem};
use rten::Model;

const DETECTION_MODEL: &[u8] = include_bytes!("../../models/ocrs/text-detection.rten");
const RECOGNITION_MODEL: &[u8] = include_bytes!("../../models/ocrs/text-recognition.rten");

pub(super) fn recognize(image: &DynamicImage) -> Result<Vec<OcrLine>, String> {
    let engine = OcrEngine::new(OcrEngineParams {
        detection_model: Some(
            Model::load_static_slice(DETECTION_MODEL).map_err(|_| "内置文字定位模型无法加载")?,
        ),
        recognition_model: Some(
            Model::load_static_slice(RECOGNITION_MODEL).map_err(|_| "内置文字识别模型无法加载")?,
        ),
        debug: false,
        ..Default::default()
    })
    .map_err(|_| "本地文字识别引擎无法初始化")?;
    let rgb = image.to_rgb8();
    let source = ImageSource::from_bytes(rgb.as_raw(), rgb.dimensions())
        .map_err(|_| "无法准备文字识别像素")?;
    let input = engine
        .prepare_input(source)
        .map_err(|_| "无法准备文字识别输入")?;
    let words = engine
        .detect_words(&input)
        .map_err(|_| "本地文字位置识别失败")?;
    if words.len() > 10_000 {
        return Err("图片文字过多，请裁剪后分区域识别".into());
    }
    let lines = engine.find_text_lines(&input, &words);
    if lines.len() > MAX_LINES {
        return Err("图片文字过多，请裁剪后分区域识别".into());
    }
    let recognized = engine
        .recognize_text(&input, &lines)
        .map_err(|_| "本地文字识别失败")?;
    Ok(recognized
        .into_iter()
        .flatten()
        .map(|line| {
            let bounds = line.bounding_rect();
            OcrLine {
                text: line.to_string(),
                x: f64::from(bounds.left()),
                y: f64::from(bounds.top()),
                width: f64::from(bounds.width()),
                height: f64::from(bounds.height()),
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    #[test]
    fn bundled_models_match_the_original_pinned_release() {
        assert_eq!(
            format!("{:x}", Sha256::digest(DETECTION_MODEL)),
            "f15cfb56bd02c4bf478a20343986504a1f01e1665c2b3a0ad66340f054b1b5ca"
        );
        assert_eq!(
            format!("{:x}", Sha256::digest(RECOGNITION_MODEL)),
            "e484866d4cce403175bd8d00b128feb08ab42e208de30e42cd9889d8f1735a6e"
        );
    }

    #[test]
    fn recognizes_generated_fixture() {
        let image =
            image::load_from_memory(include_bytes!("../../../tests/fixtures/ocr-latin.png"))
                .unwrap();
        let lines = recognize(&image).unwrap();
        assert!(lines.iter().any(|line| line.text.contains("1234567890")));
        assert!(lines
            .iter()
            .any(|line| line.text.contains("demo@example.test")));
        assert!(lines
            .iter()
            .all(|line| line.width > 0.0 && line.height > 0.0));
    }
}
