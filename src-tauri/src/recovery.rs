//! Explicit, local editor-session recovery. No automatic capture or clipboard access.
use base64::{engine::general_purpose::STANDARD, Engine};
use image::{ImageFormat, ImageReader};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs::{self, File},
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

type Result<T> = std::result::Result<T, String>;
pub const SCHEMA_VERSION: u32 = 1;
pub const MAX_SESSION_BYTES: usize = 256 * 1024 * 1024;
const MAX_IMAGE_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryPoint {
    pub id: String,
    pub created_at: u64,
    pub app_version: String,
    pub schema_version: u32,
    pub reason: String,
    pub total_bytes: u64,
    pub path: String,
    pub compatible: bool,
}
#[derive(Serialize)]
pub struct RecoveredSession {
    pub session: String,
    pub point: RecoveryPoint,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Manifest {
    id: String,
    created_at: u64,
    app_version: String,
    schema_version: u32,
    reason: String,
    total_bytes: u64,
    sha256: String,
}
fn string<'a>(value: &'a Value, key: &str, max: usize) -> Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| s.len() <= max)
        .ok_or_else(|| format!("恢复会话的 {key} 字段无效"))
}
fn number(value: &Value, key: &str, min: f64, max: f64) -> Result<f64> {
    value
        .get(key)
        .and_then(Value::as_f64)
        .filter(|n| n.is_finite() && *n >= min && *n <= max)
        .ok_or_else(|| format!("恢复会话的 {key} 数值超出范围"))
}
fn color(value: &Value, key: &str) -> Result<()> {
    let text = string(value, key, 7)?;
    if (text.len() == 4 || text.len() == 7)
        && text.starts_with('#')
        && text[1..].bytes().all(|b| b.is_ascii_hexdigit())
    {
        Ok(())
    } else {
        Err("恢复会话的颜色格式无效".into())
    }
}
fn dimensions(data: &str, max_pixels: u64) -> Result<(u32, u32)> {
    let (prefix, encoded) = data.split_once(',').ok_or("图片数据格式无效")?;
    let expected = match prefix {
        "data:image/png;base64" => ImageFormat::Png,
        "data:image/jpeg;base64" => ImageFormat::Jpeg,
        "data:image/webp;base64" => ImageFormat::WebP,
        _ => return Err("恢复点仅支持 PNG、JPEG 或 WebP 图片".into()),
    };
    if encoded.len() > MAX_IMAGE_BYTES.div_ceil(3) * 4 {
        return Err("图片文件超过 64 MiB".into());
    }
    let bytes = STANDARD
        .decode(encoded)
        .map_err(|_| "图片 Base64 数据无效")?;
    if bytes.len() > MAX_IMAGE_BYTES {
        return Err("图片文件超过 64 MiB".into());
    }
    let reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| "图片格式无效")?;
    if reader.format() != Some(expected) {
        return Err("图片内容与声明格式不一致".into());
    }
    let (width, height) = reader.into_dimensions().map_err(|_| "无法读取图片尺寸")?;
    if width == 0
        || height == 0
        || width > 16384
        || height > 16384
        || u64::from(width) * u64::from(height) > max_pixels
    {
        return Err("恢复会话图片尺寸超出限制".into());
    }
    Ok((width, height))
}

/// Validate the bounded interchange format before writing or returning a snapshot.
/// Frontend additionally decodes pixels and validates editor semantics before replacing state.
pub fn validate_session(session: &str) -> Result<()> {
    if session.len() > MAX_SESSION_BYTES {
        return Err("编辑会话超过 256 MiB，无法创建恢复点".into());
    }
    let value: Value = serde_json::from_str(session).map_err(|_| "编辑会话不是有效的 JSON")?;
    if value.get("schemaVersion").and_then(Value::as_u64) != Some(u64::from(SCHEMA_VERSION)) {
        return Err("此编辑会话版本不受支持，请使用兼容版本的星镜".into());
    }
    if !["starry", "moon", "plain", "moss", "sakura"].contains(&string(&value, "theme", 20)?) {
        return Err("恢复会话的外观名称无效".into());
    }
    let current = string(&value, "currentId", 128)?;
    let documents = value
        .get("documents")
        .and_then(Value::as_array)
        .filter(|d| d.len() <= 6)
        .ok_or("恢复会话最多支持 6 张图片")?;
    let mut ids = HashSet::new();
    let mut pixels = 0_u64;
    for document in documents {
        let id = string(document, "id", 128)?;
        if id.is_empty() || !ids.insert(id) {
            return Err("恢复会话的图片标识重复或为空".into());
        }
        if string(document, "name", 500)?.is_empty() {
            return Err("恢复会话的图片名称为空".into());
        }
        let (width, height) =
            dimensions(string(document, "dataUrl", MAX_SESSION_BYTES)?, 40_000_000)?;
        if document.get("width").and_then(Value::as_u64) != Some(u64::from(width))
            || document.get("height").and_then(Value::as_u64) != Some(u64::from(height))
        {
            return Err("恢复会话声明的图片尺寸与文件不一致".into());
        }
        pixels += u64::from(width) * u64::from(height);
        if pixels > 48_000_000 {
            return Err("恢复会话的原图合计超过 4800 万像素".into());
        }
        let state = document
            .get("state")
            .filter(|s| s.is_object())
            .ok_or("图片编辑状态无效")?;
        if let Some(data) = state.get("backgroundDataUrl") {
            dimensions(data.as_str().ok_or("背景图片无效")?, 24_000_000)?;
        }
        if let Some(crop) = state.get("crop") {
            let x = number(crop, "x", 0.0, f64::from(width))?;
            let y = number(crop, "y", 0.0, f64::from(height))?;
            let w = number(crop, "width", 1.0, f64::from(width))?;
            let h = number(crop, "height", 1.0, f64::from(height))?;
            if x + w > f64::from(width) || y + h > f64::from(height) {
                return Err("恢复会话裁切范围超出原图".into());
            }
        }
        let style = state.get("style").ok_or("图片美化样式缺失")?;
        if !["gradient", "solid", "transparent", "image"].contains(&string(
            style,
            "background",
            20,
        )?) || !["auto", "1:1", "4:3", "16:9", "9:16"].contains(&string(style, "aspect", 10)?)
        {
            return Err("图片美化样式无效".into());
        }
        if string(style, "background", 20)? == "image" && state.get("backgroundDataUrl").is_none() {
            return Err("图片背景数据缺失".into());
        }
        for key in [
            "backgroundColor",
            "gradientFrom",
            "gradientTo",
            "borderColor",
        ] {
            color(style, key)?;
        }
        for (key, maximum) in [
            ("gradientAngle", 360.0),
            ("padding", 400.0),
            ("radius", 240.0),
            ("borderWidth", 24.0),
            ("shadow", 80.0),
        ] {
            number(style, key, 0.0, maximum)?;
        }
        let annotations = state
            .get("annotations")
            .and_then(Value::as_array)
            .filter(|a| a.len() <= 2000)
            .ok_or("单张图片标注超过 2000 条或格式无效")?;
        let mut point_count = 0;
        let mut annotation_ids = HashSet::new();
        for annotation in annotations {
            let annotation_id = string(annotation, "id", 128)?;
            if annotation_id.is_empty() || !annotation_ids.insert(annotation_id) {
                return Err("恢复会话的标注标识为空或重复".into());
            }
            if ![
                "rect",
                "ellipse",
                "arrow",
                "pen",
                "highlighter",
                "text",
                "number",
                "mosaic",
            ]
            .contains(&string(annotation, "type", 20)?)
            {
                return Err("不支持的标注类型".into());
            }
            color(annotation, "color")?;
            for key in ["x", "y", "width", "height"] {
                number(annotation, key, -16384.0, 16384.0)?;
            }
            number(annotation, "lineWidth", 1.0, 100.0)?;
            if annotation.get("text").is_some() {
                string(annotation, "text", 10000)?;
            }
            if annotation.get("fontSize").is_some() {
                number(annotation, "fontSize", 1.0, 256.0)?;
            }
            if annotation.get("number").is_some() {
                let index = number(annotation, "number", 1.0, 100000.0)?;
                if index.fract() != 0.0 {
                    return Err("标注序号必须为整数".into());
                }
            }
            if let Some(points) = annotation.get("points") {
                let points = points.as_array().ok_or("画笔路径无效")?;
                point_count += points.len();
                if point_count > 100_000 {
                    return Err("单张图片画笔路径超过 10 万点".into());
                }
                for point in points {
                    number(point, "x", -16384.0, 16384.0)?;
                    number(point, "y", -16384.0, 16384.0)?;
                }
            }
        }
    }
    if (!documents.is_empty() && !ids.contains(current))
        || (documents.is_empty() && !current.is_empty())
    {
        return Err("恢复会话的当前图片不存在".into());
    }
    Ok(())
}
fn valid_id(id: &str) -> bool {
    let parts: Vec<_> = id.split('-').collect();
    parts.len() == 3
        && parts[0] == "point"
        && (1..=20).contains(&parts[1].len())
        && parts[1].bytes().all(|b| b.is_ascii_digit())
        && parts[2].len() == 12
        && parts[2].bytes().all(|b| b.is_ascii_alphanumeric())
}
fn regular(path: &Path, directory: bool) -> Result<()> {
    let metadata = fs::symlink_metadata(path).map_err(|e| format!("恢复点文件无法访问：{e}"))?;
    if metadata.file_type().is_symlink()
        || (directory && !metadata.is_dir())
        || (!directory && !metadata.is_file())
    {
        return Err("恢复点路径不是受支持的本地文件".into());
    }
    Ok(())
}
fn root(default: &Path) -> Result<PathBuf> {
    let root = default.join("recovery");
    fs::create_dir_all(&root).map_err(|e| format!("无法创建恢复点目录：{e}"))?;
    regular(&root, true)?;
    private_directory(&root)?;
    Ok(root)
}
fn private_directory(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
    }
    Ok(())
}
fn write_synced(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = File::options()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|e| e.to_string())?;
    }
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|e| format!("恢复点写入失败：{e}"))
}
fn bounded_read(path: &Path, maximum: usize) -> Result<Vec<u8>> {
    regular(path, false)?;
    let file = File::open(path).map_err(|e| e.to_string())?;
    if file.metadata().map_err(|e| e.to_string())?.len() > maximum as u64 {
        return Err("恢复点文件超出大小限制".into());
    }
    let mut bytes = Vec::new();
    file.take(maximum as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > maximum {
        return Err("恢复点文件超出大小限制".into());
    }
    Ok(bytes)
}
fn manifest_at(root: &Path, id: &str) -> Result<Manifest> {
    if !valid_id(id) {
        return Err("恢复点标识无效".into());
    }
    let directory = root.join(id);
    regular(&directory, true)?;
    let manifest: Manifest =
        serde_json::from_slice(&bounded_read(&directory.join("manifest.json"), 16384)?)
            .map_err(|_| "恢复点清单无效")?;
    if manifest.id != id
        || manifest.total_bytes > MAX_SESSION_BYTES as u64
        || manifest.app_version.len() > 100
        || !["manual", "before-update"].contains(&manifest.reason.as_str())
        || manifest.sha256.len() != 64
        || !manifest.sha256.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err("恢复点清单内容无效".into());
    }
    Ok(manifest)
}
fn point(manifest: &Manifest, root: &Path) -> RecoveryPoint {
    RecoveryPoint {
        id: manifest.id.clone(),
        created_at: manifest.created_at,
        app_version: manifest.app_version.clone(),
        schema_version: manifest.schema_version,
        reason: manifest.reason.clone(),
        total_bytes: manifest.total_bytes,
        path: root.join(&manifest.id).to_string_lossy().into_owned(),
        compatible: manifest.schema_version == SCHEMA_VERSION,
    }
}
pub fn create(default: &Path, session: &str, reason: &str) -> Result<RecoveryPoint> {
    validate_session(session)?;
    if !["manual", "before-update"].contains(&reason) {
        return Err("恢复点原因无效".into());
    }
    let root = root(default)?;
    let stage = tempfile::Builder::new()
        .prefix(".creating-")
        .rand_bytes(12)
        .tempdir_in(&root)
        .map_err(|e| e.to_string())?;
    private_directory(stage.path())?;
    let created_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "系统时间无效")?
        .as_millis() as u64;
    let suffix = stage
        .path()
        .file_name()
        .and_then(|n| n.to_str())
        .and_then(|n| n.strip_prefix(".creating-"))
        .ok_or("恢复点标识创建失败")?;
    let id = format!("point-{created_at}-{suffix}");
    if !valid_id(&id) {
        return Err("恢复点标识创建失败".into());
    }
    let manifest = Manifest {
        id: id.clone(),
        created_at,
        app_version: env!("CARGO_PKG_VERSION").into(),
        schema_version: SCHEMA_VERSION,
        reason: reason.into(),
        total_bytes: session.len() as u64,
        sha256: format!("{:x}", Sha256::digest(session.as_bytes())),
    };
    write_synced(&stage.path().join("session.json"), session.as_bytes())?;
    write_synced(
        &stage.path().join("manifest.json"),
        &serde_json::to_vec(&manifest).map_err(|e| e.to_string())?,
    )?;
    #[cfg(unix)]
    File::open(stage.path())
        .and_then(|f| f.sync_all())
        .map_err(|e| format!("恢复点目录同步失败：{e}"))?;
    let destination = root.join(&id);
    if destination.exists() {
        return Err("恢复点标识冲突，请重试".into());
    }
    fs::rename(stage.path(), &destination).map_err(|e| format!("恢复点保存失败：{e}"))?;
    #[cfg(unix)]
    File::open(&root)
        .and_then(|f| f.sync_all())
        .map_err(|e| format!("恢复点目录同步失败：{e}"))?;
    Ok(point(&manifest, &root))
}
pub fn list(default: &Path) -> Result<Vec<RecoveryPoint>> {
    let root = default.join("recovery");
    if !root.exists() {
        return Ok(Vec::new());
    }
    regular(&root, true)?;
    let mut points = Vec::new();
    for entry in fs::read_dir(&root).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name();
        let Some(id) = name.to_str().filter(|id| valid_id(id)) else {
            continue;
        };
        if let Ok(manifest) = manifest_at(&root, id) {
            points.push(point(&manifest, &root));
        }
    }
    points.sort_by(|a, b| {
        b.created_at
            .cmp(&a.created_at)
            .then_with(|| b.id.cmp(&a.id))
    });
    Ok(points)
}
pub fn read(default: &Path, id: &str) -> Result<RecoveredSession> {
    let root = default.join("recovery");
    regular(&root, true)?;
    let manifest = manifest_at(&root, id)?;
    if manifest.schema_version != SCHEMA_VERSION {
        return Err("此恢复点由不兼容版本创建，不能在当前版本恢复".into());
    }
    let bytes = bounded_read(&root.join(id).join("session.json"), MAX_SESSION_BYTES)?;
    if bytes.len() as u64 != manifest.total_bytes
        || format!("{:x}", Sha256::digest(&bytes)) != manifest.sha256
    {
        return Err("恢复点完整性校验失败，当前编辑会话未改变".into());
    }
    let session = String::from_utf8(bytes).map_err(|_| "恢复点文字编码无效")?;
    validate_session(&session)?;
    Ok(RecoveredSession {
        session,
        point: point(&manifest, &root),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn fixture() -> Value {
        let image = image::DynamicImage::new_rgba8(2, 3);
        json!({"schemaVersion":1,"theme":"starry","currentId":"fixture","documents":[{
            "id":"fixture","name":"原创固定像素","dataUrl":crate::imaging::png_data_url(&image).unwrap(),"width":2,"height":3,
            "state":{"annotations":[],"style":{"background":"solid","backgroundColor":"#123456","gradientFrom":"#123456","gradientTo":"#abcdef","gradientAngle":135,"padding":72,"radius":16,"borderWidth":1,"borderColor":"#ffffff","shadow":32,"aspect":"auto"}}
        }]})
    }
    #[test]
    fn immutable_roundtrip_and_corruption_never_replace_session() {
        let directory = tempfile::tempdir().unwrap();
        let session = fixture().to_string();
        let first = create(directory.path(), &session, "manual").unwrap();
        let second = create(directory.path(), &session, "before-update").unwrap();
        assert_ne!(first.id, second.id);
        assert_eq!(list(directory.path()).unwrap().len(), 2);
        assert_eq!(read(directory.path(), &first.id).unwrap().session, session);
        fs::write(Path::new(&first.path).join("session.json"), "tampered").unwrap();
        assert!(read(directory.path(), &first.id).is_err());
        assert_eq!(read(directory.path(), &second.id).unwrap().session, session);
        for id in ["../session", "point-1-../secret", "/tmp/point", ""] {
            assert!(read(directory.path(), id).is_err());
        }
    }
    #[test]
    fn rejects_future_schema_and_invalid_session_bounds() {
        let session = fixture();
        validate_session(&session.to_string()).unwrap();
        for (pointer, replacement) in [
            ("/schemaVersion", json!(2)),
            ("/theme", json!("unknown")),
            ("/currentId", json!("missing")),
            ("/documents/0/width", json!(50000)),
            (
                "/documents/0/dataUrl",
                json!("https://example.com/image.png"),
            ),
            ("/documents/0/state/style/padding", json!(100000)),
            (
                "/documents/0/state/crop",
                json!({"x":1,"y":0,"width":2,"height":3}),
            ),
        ] {
            let mut invalid = session.clone();
            if let Some(target) = invalid.pointer_mut(pointer) {
                *target = replacement;
            } else {
                invalid["documents"][0]["state"]["crop"] = replacement;
            }
            assert!(validate_session(&invalid.to_string()).is_err(), "{pointer}");
        }
        let mut duplicate = session.clone();
        duplicate["documents"]
            .as_array_mut()
            .unwrap()
            .push(session["documents"][0].clone());
        assert!(validate_session(&duplicate.to_string()).is_err());
        let directory = tempfile::tempdir().unwrap();
        let point = create(directory.path(), &session.to_string(), "manual").unwrap();
        let file = Path::new(&point.path).join("manifest.json");
        let mut manifest: Value = serde_json::from_slice(&fs::read(&file).unwrap()).unwrap();
        manifest["schemaVersion"] = json!(2);
        fs::write(file, manifest.to_string()).unwrap();
        assert!(!list(directory.path()).unwrap()[0].compatible);
        assert!(read(directory.path(), &point.id).is_err());
    }
    #[test]
    fn accepts_empty_session_and_webp_but_rejects_disguised_format() {
        validate_session(r#"{"schemaVersion":1,"theme":"plain","currentId":"","documents":[]}"#)
            .unwrap();
        let mut session = fixture();
        let mut encoded = Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(2, 3)
            .write_to(&mut encoded, ImageFormat::WebP)
            .unwrap();
        session["documents"][0]["dataUrl"] = json!(format!(
            "data:image/webp;base64,{}",
            STANDARD.encode(encoded.into_inner())
        ));
        validate_session(&session.to_string()).unwrap();
        let data = session["documents"][0]["dataUrl"]
            .as_str()
            .unwrap()
            .replace("image/webp", "image/png");
        session["documents"][0]["dataUrl"] = json!(data);
        assert!(validate_session(&session.to_string()).is_err());
    }
    #[cfg(unix)]
    #[test]
    fn refuses_symlinked_snapshot_files() {
        let directory = tempfile::tempdir().unwrap();
        let point = create(directory.path(), &fixture().to_string(), "manual").unwrap();
        let file = Path::new(&point.path).join("session.json");
        let outside = directory.path().join("unrelated.json");
        fs::rename(&file, &outside).unwrap();
        std::os::unix::fs::symlink(outside, file).unwrap();
        assert!(read(directory.path(), &point.id).is_err());
    }
}
