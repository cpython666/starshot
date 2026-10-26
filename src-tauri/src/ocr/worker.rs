//! A disposable process releases the inference allocator and thread pools after each scan.
use super::{offline, OcrLine};
use crate::imaging;
use image::DynamicImage;
use std::{
    io::{Read, Write},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

const INPUT_LIMIT: u64 = (imaging::MAX_BYTES * 4 / 3 + 128) as u64;
const OUTPUT_LIMIT: u64 = 2 * 1024 * 1024;
const TIMEOUT: Duration = Duration::from_secs(90);

pub(super) fn recognize(image: &DynamicImage) -> Result<Vec<OcrLine>, String> {
    let data = imaging::png_data_url(image)?;
    let mut command = Command::new(std::env::current_exe().map_err(|_| "无法定位本地识别程序")?);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let mut child = command
        .arg("--starshot-ocr-worker")
        .env("RTEN_NUM_THREADS", "2")
        .env("RAYON_NUM_THREADS", "2")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "无法启动本地识别，请检查应用是否完整")?;
    let mut stdin = child.stdin.take().ok_or("无法连接本地识别输入")?;
    let stdout = child.stdout.take().ok_or("无法连接本地识别输出")?;
    let writer = std::thread::spawn(move || stdin.write_all(data.as_bytes()));
    let reader = std::thread::spawn(move || {
        let mut output = Vec::new();
        stdout
            .take(OUTPUT_LIMIT + 1)
            .read_to_end(&mut output)
            .map(|_| output)
    });
    let started = Instant::now();
    let exit = loop {
        match child.try_wait() {
            Ok(Some(exit)) => break Ok(exit),
            Ok(None) if started.elapsed() < TIMEOUT => {
                std::thread::sleep(Duration::from_millis(30))
            }
            Ok(None) => break Err("识别超过 90 秒，请裁剪为更小区域后重试"),
            Err(_) => break Err("本地识别进程异常"),
        }
    };
    if exit.is_err() {
        let _ = child.kill();
        let _ = child.wait();
    }
    let wrote = writer.join().map_err(|_| "本地识别输入失败")?;
    let output = reader
        .join()
        .map_err(|_| "本地识别输出失败")?
        .map_err(|_| "无法读取本地识别结果")?;
    let exit = exit?;
    if !exit.success() || wrote.is_err() {
        return Err("本地文字识别未完成，请重试或手动打码".into());
    }
    if output.len() as u64 > OUTPUT_LIMIT {
        return Err("图片文字过多，请裁剪后分区域识别".into());
    }
    serde_json::from_slice::<Result<Vec<OcrLine>, String>>(&output)
        .map_err(|_| "本地识别结果无效".to_string())?
}

pub(super) fn run() {
    let result = (|| -> Result<Vec<OcrLine>, String> {
        let mut input = String::new();
        std::io::stdin()
            .take(INPUT_LIMIT + 1)
            .read_to_string(&mut input)
            .map_err(|_| "图片输入无效")?;
        if input.len() as u64 > INPUT_LIMIT {
            return Err("图片输入超过限制".into());
        }
        let image = imaging::decode_data_url(&input)?;
        if image.width().max(image.height()) > super::MAX_OCR_EDGE {
            return Err("文字识别图片超过尺寸限制".into());
        }
        offline::recognize(&image)
    })();
    // A single bounded JSON message is the only output; never print pixels/text to logs.
    if let Ok(mut output) = serde_json::to_vec(&result) {
        if output.len() as u64 > OUTPUT_LIMIT {
            let too_large: Result<Vec<OcrLine>, String> =
                Err("图片文字过多，请裁剪后分区域识别".into());
            output = serde_json::to_vec(&too_large).unwrap_or_default();
        }
        let _ = std::io::stdout().write_all(&output);
    }
}
