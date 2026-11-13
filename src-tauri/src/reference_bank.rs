use lmfpd::{Analysis, Detector, LoadOptions, Output};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashSet, VecDeque},
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};
use tauri::{AppHandle, Emitter, Manager};

const SOURCE: &str = "https://lm.ikale.io/data/";
const DETECTOR: &str = "shared_detector.bin";
const MANIFEST_LIMIT: u64 = 256 * 1024;
const DATA_LIMIT: u64 = 32 * 1024 * 1024;
const BUILTIN_DATE: &str = "2026-10-07T20:04:35.858118+00:00";
const INVALID: &str = "上游指纹库数据校验失败";
const UNAVAILABLE: &str = "指纹库状态暂时不可用";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BankInfo {
    built_at: String,
    model_count: usize,
    version: String,
}

#[derive(Clone, Serialize)]
pub struct BankStatus {
    bank: BankInfo,
    busy: bool,
    message: String,
    revision: u64,
}

struct Reference {
    info: BankInfo,
    detector: Detector,
}

struct Banks {
    status: BankStatus,
    versions: VecDeque<Arc<Reference>>,
}

pub struct BankState(Mutex<Banks>);

impl Default for BankState {
    fn default() -> Self {
        Self(Mutex::new(Banks {
            status: BankStatus {
                bank: BankInfo {
                    built_at: BUILTIN_DATE.into(),
                    model_count: 58,
                    version: "builtin".into(),
                },
                busy: false,
                message: String::new(),
                revision: 0,
            },
            versions: VecDeque::new(),
        }))
    }
}

impl Banks {
    fn activate(&mut self, reference: Reference) {
        self.status.bank = reference.info.clone();
        self.versions.push_front(Arc::new(reference));
        // shortcut: keep 11 versions in memory; pin per run if frequent updates make history age out.
        self.versions.truncate(11);
    }

    fn publish(&mut self, app: &AppHandle) {
        self.status.revision += 1;
        let _ = app.emit("probe-reference-changed", &self.status);
    }
}

#[derive(Deserialize)]
struct Manifest {
    schema: String,
    detector_schema: String,
    built_at: String,
    models: Vec<Model>,
    files: Files,
}

#[derive(Deserialize)]
struct Model {
    id: String,
}

#[derive(Deserialize)]
struct Files {
    #[serde(rename = "shared_detector.bin")]
    detector: DetectorFile,
}

#[derive(Deserialize)]
struct DetectorFile {
    parts: Vec<String>,
    bytes: u64,
    sha256: String,
}

fn manifest(bytes: &[u8]) -> Result<Manifest, &'static str> {
    let data: Manifest = serde_json::from_slice(bytes).map_err(|_| INVALID)?;
    if data.schema != "lmfpd-data-v1" || data.detector_schema != "shared-detector-v2" {
        return Err("上游指纹库格式已变化，需要升级 ModelLens");
    }
    let file = &data.files.detector;
    let ids: HashSet<_> = data.models.iter().map(|model| &model.id).collect();
    let parts: HashSet<_> = file.parts.iter().collect();
    if chrono::DateTime::parse_from_rfc3339(&data.built_at).is_err()
        || !(1..=1024).contains(&ids.len())
        || ids.len() != data.models.len()
        || ids
            .iter()
            .any(|id| id.is_empty() || id.len() > 512 || id.chars().any(char::is_control))
        || !(1..=DATA_LIMIT).contains(&file.bytes)
        || file.sha256.len() != 64
        || !file.sha256.bytes().all(|b| b.is_ascii_hexdigit())
        || !(1..=16).contains(&file.parts.len())
        || parts.len() != file.parts.len()
        || file.parts.iter().any(|part| {
            part.len() > 128
                || part
                    .strip_prefix("shared_detector.bin.")
                    .and_then(|name| name.strip_suffix(".zst"))
                    .is_none_or(|name| {
                        name.is_empty()
                            || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.')
                    })
        })
    {
        return Err(INVALID);
    }
    Ok(data)
}

fn decode(data: Manifest, bundle: &[u8], binary: &[u8]) -> Result<Reference, &'static str> {
    let version = data.files.detector.sha256;
    if format!("{:x}", Sha256::digest(binary)) != version {
        return Err(INVALID);
    }
    validate_binary(binary).ok_or(INVALID)?;
    // The SDK's local loader checks model order, response counts and detector dimensions.
    let staging = tempfile::tempdir().map_err(|_| "无法准备指纹库校验")?;
    fs::write(staging.path().join("unified_bank.json"), bundle)
        .and_then(|()| fs::write(staging.path().join(DETECTOR), binary))
        .map_err(|_| "无法准备指纹库校验")?;
    let detector = Detector::load(&LoadOptions {
        sources: vec![staging.path().to_string_lossy().into_owned()],
        cache: PathBuf::new(),
        ttl: 0.0,
    })
    .map_err(|_| INVALID)?;
    let output = Output {
        // Large steps pass the SDK's sequence check so validation exercises the ranker.
        text: (0..300)
            .map(|i| (i * 137 % 355 + 1).to_string())
            .collect::<Vec<_>>()
            .join(", "),
        expected_count: 300,
    };
    let check = detector
        .analyze(&[output.clone(), output.clone(), output], true)
        .map_err(|_| INVALID)?;
    if check.results.len() != data.models.len() || check.probability.is_none_or(|p| !p.is_finite())
    {
        return Err(INVALID);
    }
    Ok(Reference {
        info: BankInfo {
            built_at: data.built_at,
            model_count: data.models.len(),
            version,
        },
        detector,
    })
}

fn validate_binary(binary: &[u8]) -> Option<()> {
    use serde_json::Value;

    fn visit(value: &Value, count: u64, remaining: &mut u64) -> Option<()> {
        match value {
            Value::Array(values) => values
                .iter()
                .try_for_each(|value| visit(value, count, remaining)),
            Value::Object(fields) => {
                if let Some(offset) = fields.get("$f32") {
                    let length = fields.get("n")?.as_u64()?;
                    if offset.as_u64()?.checked_add(length)? > count {
                        return None;
                    }
                    // The encoder writes each vector once. Limit expansion before the SDK allocates it.
                    *remaining = remaining.checked_sub(length)?;
                    Some(())
                } else {
                    fields
                        .values()
                        .try_for_each(|value| visit(value, count, remaining))
                }
            }
            _ => Some(()),
        }
    }

    let header_length = u32::from_le_bytes(binary.get(..4)?.try_into().ok()?) as usize;
    if header_length as u64 > MANIFEST_LIMIT {
        return None;
    }
    let end = 4 + header_length;
    let floats = binary.get(end.next_multiple_of(4)..)?.len();
    if floats % 4 != 0 {
        return None;
    }
    let count = (floats / 4) as u64;
    let mut remaining = count;
    let header = serde_json::from_slice(binary.get(4..end)?).ok()?;
    visit(&header, count, &mut remaining)
}

fn bounded_read(mut reader: impl Read, limit: u64) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader.by_ref().take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(std::io::Error::other("fingerprint data exceeds limit"));
    }
    Ok(bytes)
}

fn read_cache(path: &Path) -> Option<Reference> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let mut file = options.open(path).ok()?;
    if !file.metadata().ok()?.is_file() {
        return None;
    }
    let mut length = [0; 4];
    file.read_exact(&mut length).ok()?;
    let length = u32::from_le_bytes(length);
    if u64::from(length) > MANIFEST_LIMIT {
        return None;
    }
    let mut bundle = vec![0; length as usize];
    file.read_exact(&mut bundle).ok()?;
    let data = manifest(&bundle).ok()?;
    let binary = bounded_read(file, DATA_LIMIT).ok()?;
    decode(data, &bundle, &binary).ok()
}

fn save_cache(path: &Path, bundle: &[u8], binary: &[u8]) -> std::io::Result<()> {
    let directory = path
        .parent()
        .ok_or_else(|| std::io::Error::other("missing cache directory"))?;
    fs::create_dir_all(directory)?;
    let mut temp = tempfile::NamedTempFile::new_in(directory)?;
    temp.write_all(&(bundle.len() as u32).to_le_bytes())?;
    temp.write_all(bundle)?;
    temp.write_all(binary)?;
    temp.as_file().sync_all()?;
    temp.persist(path)?;
    Ok(())
}

fn download(
    path: &Path,
    current: &BankInfo,
    fetch: impl Fn(&str, u64) -> Result<Vec<u8>, &'static str>,
) -> Result<Option<Reference>, &'static str> {
    let bundle = fetch("bundle.json", MANIFEST_LIMIT)?;
    let data = manifest(&bundle)?;
    let file = &data.files.detector;
    if file.sha256 == current.version {
        return Ok(None);
    }
    if chrono::DateTime::parse_from_rfc3339(&data.built_at).ok()
        < chrono::DateTime::parse_from_rfc3339(&current.built_at).ok()
    {
        return Err("上游指纹库版本早于本机版本");
    }
    let mut compressed = Vec::new();
    for part in &file.parts {
        let bytes = fetch(
            &format!("chunks/{part}"),
            file.bytes - compressed.len() as u64,
        )?;
        compressed.extend(bytes);
        if compressed.len() as u64 > file.bytes {
            return Err(INVALID);
        }
    }
    if compressed.len() as u64 != file.bytes {
        return Err(INVALID);
    }
    let decoder = ruzstd::decoding::StreamingDecoder::new_with_max_window_size(
        compressed.as_slice(),
        DATA_LIMIT,
    )
    .map_err(|_| INVALID)?;
    let binary = bounded_read(decoder, DATA_LIMIT).map_err(|_| INVALID)?;
    let reference = decode(data, &bundle, &binary)?;
    save_cache(path, &bundle, &binary).map_err(|_| "无法保存新版指纹库")?;
    Ok(Some(reference))
}

fn check(app: &AppHandle, startup: bool) -> Result<Option<Reference>, &'static str> {
    let state = app.state::<BankState>();
    let path = app
        .path()
        .app_cache_dir()
        .map_err(|_| "无法定位指纹库缓存目录")?
        .join("probe-reference.bin");
    if startup {
        if let Some(cached) = read_cache(&path) {
            let mut banks = state.0.lock().map_err(|_| UNAVAILABLE)?;
            banks.activate(cached);
            banks.publish(app);
        }
    }
    let current = state.0.lock().map_err(|_| UNAVAILABLE)?.status.bank.clone();
    let client = reqwest::blocking::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .connect_timeout(Duration::from_secs(5))
        .build()
        .map_err(|_| "无法初始化指纹库连接")?;
    let deadline = std::time::Instant::now() + Duration::from_secs(35);
    download(&path, &current, |file, limit| {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            return Err("指纹库下载超时");
        }
        let response = client
            .get(format!("{SOURCE}{file}"))
            .timeout(remaining)
            .send()
            .map_err(|_| "无法连接上游指纹库，或下载超时")?;
        if !response.status().is_success() {
            return Err("上游指纹库暂时不可用");
        }
        bounded_read(response, limit).map_err(|_| "指纹库下载未完成或超出大小限制")
    })
}

fn schedule(app: AppHandle, startup: bool) -> Result<(), String> {
    {
        let state = app.state::<BankState>();
        let mut banks = state.0.lock().map_err(|_| UNAVAILABLE)?;
        if banks.status.busy {
            return Ok(());
        }
        banks.status.busy = true;
        banks.status.message.clear();
        banks.publish(&app);
    }
    tauri::async_runtime::spawn(async move {
        let job_app = app.clone();
        let result = tauri::async_runtime::spawn_blocking(move || check(&job_app, startup)).await;
        let state = app.state::<BankState>();
        if let Ok(mut banks) = state.0.lock() {
            banks.status.busy = false;
            banks.status.message = match result {
                Ok(Ok(Some(reference))) => {
                    banks.activate(reference);
                    if startup { "" } else { "指纹库已更新" }.into()
                }
                Ok(Ok(None)) => if startup { "" } else { "指纹库已是最新" }.into(),
                Ok(Err(error)) => format!("{error}，继续使用当前库"),
                Err(_) => "指纹库更新未完成，继续使用当前库".into(),
            };
            banks.publish(&app);
        };
    });
    Ok(())
}

pub fn start(app: AppHandle) {
    let _ = schedule(app, true);
}

#[tauri::command]
pub fn probe_reference_status(state: tauri::State<'_, BankState>) -> Result<BankStatus, String> {
    Ok(state.0.lock().map_err(|_| UNAVAILABLE)?.status.clone())
}

#[tauri::command]
pub fn probe_reference_update(app: AppHandle) -> Result<(), String> {
    schedule(app, false)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Scoring {
    analysis: Analysis,
    reference_bank: BankInfo,
}

#[tauri::command]
pub async fn probe_reference_analyze(
    state: tauri::State<'_, BankState>,
    version: String,
    outputs: Vec<Output>,
) -> Result<Scoring, String> {
    let counts: HashSet<_> = outputs.iter().map(|sample| sample.expected_count).collect();
    if !(1..=3).contains(&outputs.len())
        || counts.len() != outputs.len()
        || outputs.iter().any(|sample| {
            !(292..=332).contains(&sample.expected_count) || sample.text.len() > 80_000
        })
    {
        return Err("探查样本无效".into());
    }
    let bank = state
        .0
        .lock()
        .map_err(|_| UNAVAILABLE)?
        .versions
        .iter()
        .find(|bank| bank.info.version == version)
        .cloned()
        .ok_or("此轮使用的指纹库版本已不可用")?;
    tauri::async_runtime::spawn_blocking(move || {
        let analysis = bank
            .detector
            .analyze(&outputs, false)
            .map_err(|_| "指纹分析失败")?;
        Ok(Scoring {
            analysis,
            reference_bank: bank.info.clone(),
        })
    })
    .await
    .map_err(|_| "指纹分析未完成".to_owned())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    fn fixture(date: &str) -> (Value, Vec<u8>) {
        let vector = |n, value| vec![value; n];
        let head = |sizes: &[usize]| {
            sizes
                .iter()
                .map(|&n| json!({"mean": vector(n, 0), "scale": vector(n, 1)}))
                .collect::<Vec<_>>()
        };
        let feature = |n| {
            json!({
                "feature_mean": vector(n, 0), "feature_scale": vector(n, 1),
                "centroids": [vector(n, 0)], "nuisance_basis": [],
                "environment_centroids": [[vector(n, 0)]]
            })
        };
        let header = serde_json::to_vec(&json!({
            "schema": "shared-detector-v2", "bank_built_at": date,
            "base_sha256": "base", "source_reference_sha256": "reference",
            "model_ids": ["fixture-model"], "response_counts": [36],
            "calibration": {"schema": "shared-confidence-v2", "method": "ranking-temperature",
                "tau": 1.0, "binding": {"base_sha256": "base", "reference_sha256": "reference",
                "model_ids": ["fixture-model"]}},
            "ranker": {
                "head_params": head(&[355, 74]), "full_params": head(&[355, 74]),
                "lda_weights": [vector(429, 0)], "lda_bias": [0],
                "references": [[vector(429, 0)]],
                "bank": {"hellinger": feature(355), "ordered_blocks": feature(74)},
                "blend": {"frequency": 0.5, "positional": 0.5},
                "positional": {"params": head(&[355, 74, 102, 220]), "weights": [vector(751, 0)], "bias": [0]}
            }
        })).unwrap();
        let mut binary = (header.len() as u32).to_le_bytes().to_vec();
        binary.extend(header);
        binary.resize(binary.len().next_multiple_of(4), 0);
        let part = ruzstd::encoding::compress_to_vec(
            binary.as_slice(),
            ruzstd::encoding::CompressionLevel::Fastest,
        );
        let manifest = json!({
            "schema": "lmfpd-data-v1", "detector_schema": "shared-detector-v2", "built_at": date,
            "models": [{"id": "fixture-model", "display_name": "Fixture", "family": "fixture",
                "family_name": "Fixture", "response_count": 36}],
            "files": {"shared_detector.bin": {"bytes": part.len(), "sha256": format!("{:x}", Sha256::digest(binary)),
                "parts": ["shared_detector.bin.fixture.0.zst"]}}
        });
        (manifest, part)
    }

    #[test]
    fn updates_and_restores_a_valid_bundle_without_replacing_it_on_failure() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("cache.bin");
        let builtin = BankState::default().0.into_inner().unwrap().status.bank;
        let (mut bundle, part) = fixture("2026-11-12T18:48:30Z");
        let fetch = |file: &str, _| {
            if file == "bundle.json" {
                Ok(serde_json::to_vec(&bundle).unwrap())
            } else {
                Ok(part.clone())
            }
        };
        let installed = download(&path, &builtin, fetch).unwrap().unwrap();
        assert_eq!(installed.info.model_count, 1);
        assert_eq!(
            read_cache(&path).unwrap().info.version,
            installed.info.version
        );
        let original = fs::read(&path).unwrap();
        assert!(download(&path, &installed.info, |file, _| {
            assert_eq!(file, "bundle.json");
            Ok(serde_json::to_vec(&bundle).unwrap())
        })
        .unwrap()
        .is_none());

        bundle["files"][DETECTOR]["sha256"] = json!("a".repeat(64));
        assert!(download(&path, &installed.info, |file, _| {
            if file == "bundle.json" {
                Ok(serde_json::to_vec(&bundle).unwrap())
            } else {
                Ok(part.clone())
            }
        })
        .is_err());
        assert_eq!(fs::read(&path).unwrap(), original);
        let mut damaged = original;
        *damaged.last_mut().unwrap() ^= 1;
        fs::write(&path, damaged).unwrap();
        assert!(read_cache(&path).is_none());
    }

    #[test]
    fn rejects_incompatible_or_older_manifest_before_fetching_chunks() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("cache.bin");
        let builtin = BankState::default().0.into_inner().unwrap().status.bank;
        let (mut bundle, _) = fixture("2026-10-01T00:00:00Z");
        for invalid in [None, Some(("detector_schema", "shared-detector-v3"))] {
            if let Some((field, value)) = invalid {
                bundle[field] = json!(value);
            }
            assert!(download(&path, &builtin, |file, _| {
                assert_eq!(file, "bundle.json");
                Ok(serde_json::to_vec(&bundle).unwrap())
            })
            .is_err());
        }
        assert!(!path.exists());
    }
}
