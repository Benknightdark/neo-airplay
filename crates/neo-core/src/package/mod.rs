use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use zip::ZipArchive;

use crate::{AppLogger, LogLevel, Platform};

const MAX_ARCHIVE_BYTES: u64 = 1_500_000_000;
const MAX_EXTRACTED_BYTES: u64 = 1_500_000_000;
const MAX_MANIFEST_BYTES: u64 = 2_000_000;
const MAX_ARCHIVE_ENTRIES: usize = 20_000;
const MANIFEST_NAME: &str = "engine-package-manifest.json";
const ENGINE_REPOSITORY: &str = "Benknightdark/neo-airplay";

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub struct EnginePackageConfig {
    pub schema_version: u32,
    pub version: String,
    pub release_tag: String,
    pub manifest_name: String,
    pub targets: BTreeMap<String, EnginePackageTarget>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub struct EnginePackageTarget {
    pub asset_name: String,
    pub sha256: String,
    pub air_play_files: Vec<String>,
    pub android_files: Vec<String>,
    pub shared_files: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EngineProvisioningResult {
    pub is_ready: bool,
    pub message: String,
}

#[derive(Debug, Error)]
pub enum EnginePackageError {
    #[error("engine package configuration is invalid: {0}")]
    InvalidConfiguration(String),
    #[error("engine package is invalid: {0}")]
    InvalidArchive(String),
    #[error("engine package I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("engine package JSON is invalid: {0}")]
    Json(#[from] serde_json::Error),
    #[error("engine package download failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("engine package archive is invalid: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("engine package worker failed: {0}")]
    Worker(#[from] tokio::task::JoinError),
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
struct PackageManifest {
    version: String,
    air_play_files: Vec<String>,
    android_files: Vec<String>,
    shared_files: Vec<String>,
    files: Vec<PackageManifestFile>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
struct PackageManifestFile {
    path: String,
    length: u64,
    sha256: String,
}

pub struct EnginePackageBootstrapper {
    config: EnginePackageConfig,
    target_key: String,
    target: EnginePackageTarget,
    bundled_engine_directory: Option<PathBuf>,
    engine_directory: PathBuf,
    package_cache_directory: PathBuf,
    client: Client,
    logger: Arc<dyn AppLogger>,
}

impl EnginePackageConfig {
    pub fn parse(value: &str) -> Result<Self, EnginePackageError> {
        let config: Self = serde_json::from_str(value)?;
        config.validate()?;
        Ok(config)
    }

    pub fn target_key(platform: Platform, architecture: &str) -> String {
        let platform = match platform {
            Platform::Windows => "windows",
            Platform::Macos => "macos",
            Platform::Linux => "linux",
        };
        let architecture = match architecture {
            "aarch64" | "arm64" => "aarch64",
            "x86_64" | "x64" => "x86_64",
            other => other,
        };
        format!("{platform}-{architecture}")
    }

    fn validate(&self) -> Result<(), EnginePackageError> {
        if self.schema_version != 2
            || !valid_version(&self.version)
            || self.release_tag != format!("v{}", self.version)
            || self.manifest_name != MANIFEST_NAME
            || self.targets.is_empty()
        {
            return Err(EnginePackageError::InvalidConfiguration(
                "schema version, version, release tag, manifest name, or targets are invalid"
                    .to_owned(),
            ));
        }
        for (target_key, target) in &self.targets {
            if !matches!(
                target_key.as_str(),
                "windows-x86_64" | "macos-aarch64" | "macos-x86_64" | "linux-x86_64"
            ) {
                return Err(EnginePackageError::InvalidConfiguration(format!(
                    "unsupported runtime target: {target_key}"
                )));
            }
            target.validate(target_key)?;
        }
        Ok(())
    }
}

impl EnginePackageTarget {
    fn validate(&self, target_key: &str) -> Result<(), EnginePackageError> {
        let expected_asset = format!("neo-airplay-engine-{target_key}.zip");
        if self.asset_name != expected_asset || !valid_sha256(&self.sha256) {
            return Err(EnginePackageError::InvalidConfiguration(format!(
                "target {target_key} has an invalid asset name or SHA-256"
            )));
        }
        let groups = [
            &self.air_play_files,
            &self.android_files,
            &self.shared_files,
        ];
        if groups.iter().all(|group| group.is_empty()) {
            return Err(EnginePackageError::InvalidConfiguration(format!(
                "target {target_key} has no runtime files"
            )));
        }
        let mut paths = HashSet::new();
        for path in groups.into_iter().flatten() {
            validate_engine_path(path).map_err(|_| {
                EnginePackageError::InvalidConfiguration(format!(
                    "target {target_key} has an unsafe file path: {path}"
                ))
            })?;
            if !paths.insert(path.to_lowercase()) {
                return Err(EnginePackageError::InvalidConfiguration(format!(
                    "target {target_key} repeats a file path: {path}"
                )));
            }
        }
        Ok(())
    }

    fn all_files(&self) -> Vec<String> {
        self.air_play_files
            .iter()
            .chain(&self.android_files)
            .chain(&self.shared_files)
            .cloned()
            .collect()
    }
}

impl EnginePackageBootstrapper {
    pub fn new(
        config: EnginePackageConfig,
        target_key: impl Into<String>,
        bundled_engine_directory: Option<PathBuf>,
        engine_directory: impl Into<PathBuf>,
        package_cache_directory: impl Into<PathBuf>,
        client: Client,
        logger: Arc<dyn AppLogger>,
    ) -> Result<Self, EnginePackageError> {
        config.validate()?;
        let target_key = target_key.into();
        let target = config.targets.get(&target_key).cloned().ok_or_else(|| {
            EnginePackageError::InvalidConfiguration(format!(
                "no engine package is configured for target {target_key}"
            ))
        })?;
        Ok(Self {
            config,
            target_key,
            target,
            bundled_engine_directory,
            engine_directory: engine_directory.into(),
            package_cache_directory: package_cache_directory.into(),
            client,
            logger,
        })
    }

    pub fn is_runtime_complete(&self) -> bool {
        is_runtime_complete_at(&self.engine_directory, &self.target)
            || self
                .bundled_engine_directory
                .as_deref()
                .is_some_and(|directory| is_runtime_complete_at(directory, &self.target))
    }

    pub async fn prepare(&self) -> Result<EngineProvisioningResult, EnginePackageError> {
        if self.is_runtime_complete() {
            return Ok(EngineProvisioningResult {
                is_ready: true,
                message: "引擎檔案已齊全。".to_owned(),
            });
        }
        let missing = missing_groups(
            &self.engine_directory,
            self.bundled_engine_directory.as_deref(),
            &self.target,
        );
        if missing.is_empty() {
            return Ok(EngineProvisioningResult {
                is_ready: true,
                message: "引擎檔案已齊全。".to_owned(),
            });
        }

        tokio::fs::create_dir_all(&self.package_cache_directory).await?;
        let archive_path = self.package_cache_directory.join(format!(
            "{}-{}",
            self.config.version, self.target.asset_name
        ));
        if !archive_matches_hash(&archive_path, &self.target.sha256).await? {
            if archive_path.exists() {
                tokio::fs::remove_file(&archive_path).await?;
            }
            self.logger
                .write(
                    LogLevel::Information,
                    "正在背景下載投影引擎套件。".to_owned(),
                )
                .await;
            self.download_archive(&archive_path).await?;
        }

        let engine_directory = self.engine_directory.clone();
        let bundled_engine_directory = self.bundled_engine_directory.clone();
        let target = self.target.clone();
        let manifest_name = self.config.manifest_name.clone();
        let version = self.config.version.clone();
        let archive_path_for_worker = archive_path.clone();
        tokio::task::spawn_blocking(move || {
            install_archive(
                &archive_path_for_worker,
                &engine_directory,
                bundled_engine_directory.as_deref(),
                &target,
                &manifest_name,
                &version,
                &missing,
            )
        })
        .await??;

        self.logger
            .write(
                LogLevel::Information,
                format!(
                    "已準備 {} 引擎套件 {}。",
                    self.target_key, self.config.version
                ),
            )
            .await;
        Ok(EngineProvisioningResult {
            is_ready: self.is_runtime_complete(),
            message: "引擎檔案已準備完成。".to_owned(),
        })
    }

    async fn download_archive(&self, archive_path: &Path) -> Result<(), EnginePackageError> {
        let url = format!(
            "https://github.com/{ENGINE_REPOSITORY}/releases/download/{}/{}",
            self.config.release_tag, self.target.asset_name
        );
        let mut response = self.client.get(url).send().await?.error_for_status()?;
        if response
            .content_length()
            .is_some_and(|length| length > MAX_ARCHIVE_BYTES)
        {
            return Err(EnginePackageError::InvalidArchive(
                "engine package exceeds the download size limit".to_owned(),
            ));
        }

        let temporary_path = archive_path.with_extension("download");
        let result = async {
            let mut output = tokio::fs::File::create(&temporary_path).await?;
            let mut hasher = Sha256::new();
            let mut total = 0_u64;
            while let Some(chunk) = response.chunk().await? {
                total = total.saturating_add(chunk.len() as u64);
                if total > MAX_ARCHIVE_BYTES {
                    return Err(EnginePackageError::InvalidArchive(
                        "engine package exceeds the download size limit".to_owned(),
                    ));
                }
                hasher.update(&chunk);
                output.write_all(&chunk).await?;
            }
            output.flush().await?;
            output.sync_all().await?;
            if !digest_hex(hasher.finalize()).eq_ignore_ascii_case(&self.target.sha256) {
                return Err(EnginePackageError::InvalidArchive(
                    "engine package SHA-256 verification failed".to_owned(),
                ));
            }
            drop(output);
            tokio::fs::rename(&temporary_path, archive_path).await?;
            Ok::<(), EnginePackageError>(())
        }
        .await;
        if result.is_err() {
            let _ = tokio::fs::remove_file(&temporary_path).await;
        }
        result
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RuntimeGroup {
    AirPlay,
    Android,
    Shared,
}

fn missing_groups(
    engine_directory: &Path,
    bundled_engine_directory: Option<&Path>,
    target: &EnginePackageTarget,
) -> Vec<RuntimeGroup> {
    let mut missing = Vec::new();
    if !runtime_group_available(
        engine_directory,
        bundled_engine_directory,
        &target.air_play_files,
    ) {
        missing.push(RuntimeGroup::AirPlay);
    }
    if !runtime_group_available(
        engine_directory,
        bundled_engine_directory,
        &target.android_files,
    ) {
        missing.push(RuntimeGroup::Android);
    }
    if !runtime_group_available(
        engine_directory,
        bundled_engine_directory,
        &target.shared_files,
    ) {
        missing.push(RuntimeGroup::Shared);
    }
    missing
}

fn runtime_group_available(
    engine_directory: &Path,
    bundled_engine_directory: Option<&Path>,
    files: &[String],
) -> bool {
    files.is_empty()
        || files
            .iter()
            .all(|path| engine_directory.join(engine_relative(path)).is_file())
        || bundled_engine_directory.is_some_and(|directory| {
            files
                .iter()
                .all(|path| directory.join(engine_relative(path)).is_file())
        })
}

fn is_runtime_complete_at(directory: &Path, target: &EnginePackageTarget) -> bool {
    target
        .all_files()
        .iter()
        .all(|path| directory.join(engine_relative(path)).is_file())
}

fn install_archive(
    archive_path: &Path,
    engine_directory: &Path,
    bundled_engine_directory: Option<&Path>,
    target: &EnginePackageTarget,
    manifest_name: &str,
    version: &str,
    missing: &[RuntimeGroup],
) -> Result<(), EnginePackageError> {
    if !archive_matches_hash_sync(archive_path, &target.sha256)? {
        return Err(EnginePackageError::InvalidArchive(
            "cached engine package SHA-256 verification failed".to_owned(),
        ));
    }

    let staging = tempfile::tempdir()?;
    let manifest =
        extract_and_validate(archive_path, staging.path(), target, manifest_name, version)?;
    let mut files_to_install = HashSet::new();
    if missing.contains(&RuntimeGroup::AirPlay) {
        files_to_install.extend(target.air_play_files.iter().cloned());
    }
    if missing.contains(&RuntimeGroup::Android) {
        files_to_install.extend(target.android_files.iter().cloned());
    }
    if missing.contains(&RuntimeGroup::Shared) {
        files_to_install.extend(target.shared_files.iter().cloned());
    }
    for file in files_to_install {
        let source = staging.path().join(&file);
        let destination = engine_directory.join(engine_relative(&file));
        atomic_install_file(&source, &destination)?;
    }

    if !missing_groups(engine_directory, bundled_engine_directory, target).is_empty()
        || manifest.version != version
    {
        return Err(EnginePackageError::InvalidArchive(
            "one or more required runtime groups remain incomplete after installation".to_owned(),
        ));
    }
    Ok(())
}

fn extract_and_validate(
    archive_path: &Path,
    staging_directory: &Path,
    target: &EnginePackageTarget,
    manifest_name: &str,
    version: &str,
) -> Result<PackageManifest, EnginePackageError> {
    fs::create_dir_all(staging_directory)?;
    let file = File::open(archive_path)?;
    let mut archive = ZipArchive::new(file)?;
    if archive.len() > MAX_ARCHIVE_ENTRIES {
        return Err(EnginePackageError::InvalidArchive(
            "engine package contains too many entries".to_owned(),
        ));
    }

    let mut entry_names = HashSet::new();
    let mut extracted_files = HashSet::new();
    let mut total_size = 0_u64;
    for index in 0..archive.len() {
        let entry = archive.by_index(index)?;
        let name = entry.name().to_owned();
        if name.is_empty() {
            return Err(EnginePackageError::InvalidArchive(
                "engine package contains an empty path".to_owned(),
            ));
        }
        let is_directory = name.ends_with('/');
        let trimmed_name = name.trim_end_matches('/');
        let relative_path = if is_directory {
            validate_directory_path(trimmed_name)?
        } else {
            validate_archive_path(trimmed_name, manifest_name)?
        };
        if !entry_names.insert(trimmed_name.to_lowercase()) {
            return Err(EnginePackageError::InvalidArchive(format!(
                "engine package contains a duplicate path: {name}"
            )));
        }

        let mode = entry.unix_mode().unwrap_or_default();
        if is_symlink_mode(mode) {
            return Err(EnginePackageError::InvalidArchive(format!(
                "engine package contains a symbolic link: {name}"
            )));
        }
        if is_directory {
            fs::create_dir_all(staging_directory.join(relative_path))?;
            continue;
        }
        extracted_files.insert(trimmed_name.to_lowercase());
        let file_type = mode & 0xF000;
        if file_type != 0 && file_type != 0x8000 {
            return Err(EnginePackageError::InvalidArchive(format!(
                "engine package contains a non-regular file: {name}"
            )));
        }
        total_size = total_size.saturating_add(entry.size());
        if total_size > MAX_EXTRACTED_BYTES {
            return Err(EnginePackageError::InvalidArchive(
                "engine package exceeds the extracted size limit".to_owned(),
            ));
        }
        if trimmed_name.eq_ignore_ascii_case(manifest_name) && entry.size() > MAX_MANIFEST_BYTES {
            return Err(EnginePackageError::InvalidArchive(
                "engine package manifest exceeds the size limit".to_owned(),
            ));
        }

        let destination = staging_directory.join(relative_path);
        let parent = destination.parent().ok_or_else(|| {
            EnginePackageError::InvalidArchive(format!("invalid archive path: {name}"))
        })?;
        fs::create_dir_all(parent)?;
        let mut output = File::options()
            .write(true)
            .create_new(true)
            .open(&destination)?;
        let expected_size = entry.size();
        let copied = io::copy(
            &mut entry.take(expected_size.saturating_add(1)),
            &mut output,
        )?;
        if copied != expected_size {
            return Err(EnginePackageError::InvalidArchive(format!(
                "engine package entry size does not match its header: {name}"
            )));
        }
        output.sync_all()?;
        set_extracted_permissions(&destination, mode)?;
    }

    let manifest_path = staging_directory.join(manifest_name);
    let manifest_bytes = fs::read(&manifest_path).map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            EnginePackageError::InvalidArchive(format!("engine package is missing {manifest_name}"))
        } else {
            EnginePackageError::Io(error)
        }
    })?;
    if manifest_bytes.len() as u64 > MAX_MANIFEST_BYTES {
        return Err(EnginePackageError::InvalidArchive(
            "engine package manifest exceeds the size limit".to_owned(),
        ));
    }
    let manifest: PackageManifest = serde_json::from_slice(&manifest_bytes)?;
    if manifest.version != version
        || !same_paths(&manifest.air_play_files, &target.air_play_files)
        || !same_paths(&manifest.android_files, &target.android_files)
        || !same_paths(&manifest.shared_files, &target.shared_files)
    {
        return Err(EnginePackageError::InvalidArchive(
            "engine package manifest does not match the configured version and file groups"
                .to_owned(),
        ));
    }

    let mut manifest_files = HashMap::new();
    for file in &manifest.files {
        validate_engine_path(&file.path).map_err(|_| {
            EnginePackageError::InvalidArchive(format!(
                "engine package manifest contains an unsafe path: {}",
                file.path
            ))
        })?;
        if !valid_sha256(&file.sha256)
            || manifest_files
                .insert(file.path.to_lowercase(), file)
                .is_some()
        {
            return Err(EnginePackageError::InvalidArchive(format!(
                "engine package manifest contains an invalid or duplicate file: {}",
                file.path
            )));
        }
        let path = staging_directory.join(&file.path);
        if !path.is_file() || fs::metadata(&path)?.len() != file.length {
            return Err(EnginePackageError::InvalidArchive(format!(
                "engine package file is missing or has a different size: {}",
                file.path
            )));
        }
        if !sha256_file(&path)?.eq_ignore_ascii_case(&file.sha256) {
            return Err(EnginePackageError::InvalidArchive(format!(
                "engine package file SHA-256 does not match: {}",
                file.path
            )));
        }
    }

    let expected_files = target
        .all_files()
        .into_iter()
        .map(|path| path.to_lowercase())
        .collect::<HashSet<_>>();
    if expected_files
        .iter()
        .any(|path| !manifest_files.contains_key(path))
    {
        return Err(EnginePackageError::InvalidArchive(
            "engine package manifest omits a required runtime file".to_owned(),
        ));
    }
    let actual_files = extracted_files
        .into_iter()
        .filter(|path| path != &manifest_name.to_lowercase())
        .collect::<HashSet<_>>();
    if actual_files != manifest_files.keys().cloned().collect() {
        return Err(EnginePackageError::InvalidArchive(
            "engine package manifest file set does not match ZIP contents".to_owned(),
        ));
    }
    Ok(manifest)
}

fn validate_archive_path(path: &str, manifest_name: &str) -> Result<PathBuf, EnginePackageError> {
    if path.eq_ignore_ascii_case(manifest_name) {
        return Ok(PathBuf::from(manifest_name));
    }
    validate_engine_path(path).map_err(|_| {
        EnginePackageError::InvalidArchive(format!(
            "engine package contains an unsafe path: {path}"
        ))
    })?;
    Ok(path.split('/').collect())
}

fn validate_directory_path(path: &str) -> Result<PathBuf, EnginePackageError> {
    let lower = path.to_lowercase();
    if path.contains('\\')
        || path.contains(':')
        || path.starts_with('/')
        || path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
        || Path::new(path)
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
        || !(lower == "engine" || lower.starts_with("engine/"))
        || lower == "engine/bonjour"
        || lower.starts_with("engine/bonjour/")
    {
        return Err(EnginePackageError::InvalidArchive(format!(
            "engine package contains an unsafe directory: {path}"
        )));
    }
    Ok(path.split('/').collect())
}

fn validate_engine_path(path: &str) -> Result<(), ()> {
    let lower = path.to_lowercase();
    if !lower.starts_with("engine/")
        || lower == "engine/bonjour"
        || lower.starts_with("engine/bonjour/")
        || path.contains('\\')
        || path.contains(':')
        || path.starts_with('/')
        || path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
        || Path::new(path)
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(());
    }
    Ok(())
}

fn engine_relative(path: &str) -> &str {
    &path["engine/".len()..]
}

fn same_paths(left: &[String], right: &[String]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .map(|path| path.to_lowercase())
            .collect::<HashSet<_>>()
            == right
                .iter()
                .map(|path| path.to_lowercase())
                .collect::<HashSet<_>>()
}

fn valid_version(version: &str) -> bool {
    let core = version.split(['-', '+']).next().unwrap_or_default();
    let parts = core.split('.').collect::<Vec<_>>();
    if parts.len() != 3
        || parts.iter().any(|part| {
            part.is_empty()
                || !part.bytes().all(|byte| byte.is_ascii_digit())
                || (part.len() > 1 && part.starts_with('0'))
        })
    {
        return false;
    }
    version.strip_prefix(core).is_some_and(|suffix| {
        suffix.is_empty()
            || (suffix.len() > 1
                && matches!(suffix.as_bytes()[0], b'-' | b'+')
                && suffix[1..]
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-')))
    })
}

fn valid_sha256(hash: &str) -> bool {
    hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn is_symlink_mode(mode: u32) -> bool {
    mode & 0xF000 == 0xA000
}

async fn archive_matches_hash(
    path: &Path,
    expected_hash: &str,
) -> Result<bool, EnginePackageError> {
    let metadata = match tokio::fs::metadata(path).await {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    if metadata.len() == 0 || metadata.len() > MAX_ARCHIVE_BYTES {
        return Ok(false);
    }
    let mut file = tokio::fs::File::open(path).await?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 128 * 1024];
    loop {
        let read = file.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(digest_hex(hasher.finalize()).eq_ignore_ascii_case(expected_hash))
}

fn archive_matches_hash_sync(path: &Path, expected_hash: &str) -> Result<bool, EnginePackageError> {
    let metadata = fs::metadata(path)?;
    if metadata.len() == 0 || metadata.len() > MAX_ARCHIVE_BYTES {
        return Ok(false);
    }
    Ok(sha256_file(path)?.eq_ignore_ascii_case(expected_hash))
}

fn sha256_file(path: &Path) -> Result<String, EnginePackageError> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 128 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(digest_hex(hasher.finalize()))
}

fn digest_hex(digest: impl AsRef<[u8]>) -> String {
    digest
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn atomic_install_file(source: &Path, destination: &Path) -> Result<(), EnginePackageError> {
    let parent = destination.parent().ok_or_else(|| {
        EnginePackageError::InvalidArchive("engine destination has no parent directory".to_owned())
    })?;
    fs::create_dir_all(parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    let mut source_file = File::open(source)?;
    io::copy(&mut source_file, &mut temporary)?;
    temporary.as_file().sync_all()?;
    set_install_permissions(source, temporary.path())?;
    temporary
        .persist(destination)
        .map_err(|error| error.error)?;
    Ok(())
}

fn set_extracted_permissions(path: &Path, mode: u32) -> Result<(), EnginePackageError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(
            path,
            fs::Permissions::from_mode(if mode & 0o111 != 0 { 0o755 } else { 0o644 }),
        )?;
    }
    #[cfg(not(unix))]
    let _ = (path, mode);
    Ok(())
}

fn set_install_permissions(source: &Path, destination: &Path) -> Result<(), EnginePackageError> {
    #[cfg(unix)]
    {
        fs::set_permissions(destination, fs::metadata(source)?.permissions())?;
    }
    #[cfg(not(unix))]
    let _ = (source, destination);
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs::File;
    use std::io::Write;

    use sha2::{Digest, Sha256};
    use tempfile::tempdir;
    use zip::write::SimpleFileOptions;
    use zip::{CompressionMethod, ZipWriter};

    use super::{
        EnginePackageConfig, EnginePackageError, EnginePackageTarget, MANIFEST_NAME,
        PackageManifest, PackageManifestFile, RuntimeGroup, extract_and_validate, install_archive,
        is_symlink_mode, sha256_file, validate_directory_path, validate_engine_path,
    };

    fn target() -> EnginePackageTarget {
        EnginePackageTarget {
            asset_name: "neo-airplay-engine-linux-x86_64.zip".to_owned(),
            sha256: "a".repeat(64),
            air_play_files: Vec::new(),
            android_files: vec!["engine/android/scrcpy".to_owned()],
            shared_files: Vec::new(),
        }
    }

    fn config() -> EnginePackageConfig {
        EnginePackageConfig {
            schema_version: 2,
            version: "1.2.3".to_owned(),
            release_tag: "v1.2.3".to_owned(),
            manifest_name: "engine-package-manifest.json".to_owned(),
            targets: BTreeMap::from([("linux-x86_64".to_owned(), target())]),
        }
    }

    fn runtime_bytes() -> Vec<u8> {
        b"scrcpy runtime fixture".to_vec()
    }

    fn manifest_bytes(target: &EnginePackageTarget, content: &[u8]) -> Vec<u8> {
        let files = target
            .all_files()
            .into_iter()
            .map(|path| PackageManifestFile {
                path,
                length: content.len() as u64,
                sha256: hex_digest(content),
            })
            .collect();
        serde_json::to_vec(&PackageManifest {
            version: "1.2.3".to_owned(),
            air_play_files: target.air_play_files.clone(),
            android_files: target.android_files.clone(),
            shared_files: target.shared_files.clone(),
            files,
        })
        .expect("manifest should serialize")
    }

    fn hex_digest(content: &[u8]) -> String {
        Sha256::digest(content)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }

    fn write_archive(path: &std::path::Path, entries: &[(&str, &[u8], u32)]) {
        let file = File::create(path).expect("archive should be created");
        let mut archive = ZipWriter::new(file);
        for (name, content, mode) in entries {
            archive
                .start_file(
                    *name,
                    SimpleFileOptions::default()
                        .compression_method(CompressionMethod::Stored)
                        .unix_permissions(*mode),
                )
                .expect("archive entry should start");
            archive
                .write_all(content)
                .expect("archive entry should be written");
        }
        archive.finish().expect("archive should finish");
    }

    fn mark_first_entry_as_symlink(path: &std::path::Path) {
        let mut bytes = std::fs::read(path).expect("archive should be readable");
        let central_header = bytes
            .windows(4)
            .position(|window| window == b"PK\x01\x02")
            .expect("archive should contain a central directory entry");
        let symlink_mode = ((0xA000_u32 | 0o777) << 16).to_le_bytes();
        bytes[central_header + 38..central_header + 42].copy_from_slice(&symlink_mode);
        std::fs::write(path, bytes).expect("archive should be updated");
    }

    #[test]
    fn accepts_empty_airplay_group_for_system_provided_uxplay() {
        assert!(config().validate().is_ok());
    }

    #[test]
    fn rejects_unsafe_duplicate_and_unpinned_runtime_paths() {
        for path in [
            "../engine/android/scrcpy",
            "/engine/android/scrcpy",
            "engine\\android\\scrcpy",
            "engine/C:/scrcpy",
            "engine/./scrcpy",
            "engine/android/../scrcpy",
            "engine//scrcpy",
            "engine/bonjour",
            "engine/bonjour/dnssd.dll",
        ] {
            assert!(validate_engine_path(path).is_err(), "accepted {path}");
        }
        assert!(validate_directory_path("engine/bonjour/").is_err());
        assert!(is_symlink_mode(0xA000 | 0o777));
        assert!(!is_symlink_mode(0x8000 | 0o755));
        let mut invalid = config();
        invalid
            .targets
            .get_mut("linux-x86_64")
            .expect("target exists")
            .shared_files
            .push("engine/android/scrcpy".to_owned());
        assert!(matches!(
            invalid.validate(),
            Err(EnginePackageError::InvalidConfiguration(_))
        ));
    }

    #[test]
    fn maps_architecture_aliases_to_manifest_target_names() {
        assert_eq!(
            EnginePackageConfig::target_key(crate::Platform::Macos, "arm64"),
            "macos-aarch64"
        );
        assert_eq!(
            EnginePackageConfig::target_key(crate::Platform::Windows, "x64"),
            "windows-x86_64"
        );
    }

    #[test]
    fn rejects_unknown_targets_and_non_semver_versions() {
        let mut invalid_target = config();
        let target = invalid_target.targets.remove("linux-x86_64").unwrap();
        invalid_target
            .targets
            .insert("linux-aarch64".to_owned(), target);
        assert!(matches!(
            invalid_target.validate(),
            Err(EnginePackageError::InvalidConfiguration(_))
        ));

        let mut invalid_version = config();
        invalid_version.version = "01.2.3".to_owned();
        invalid_version.release_tag = "v01.2.3".to_owned();
        assert!(invalid_version.validate().is_err());
    }

    #[test]
    fn validates_and_installs_a_runtime_archive_with_executable_permissions() {
        let temp = tempdir().expect("temporary directory should be created");
        let archive_path = temp.path().join("engine.zip");
        let content = runtime_bytes();
        let target = target();
        let manifest = manifest_bytes(&target, &content);
        write_archive(
            &archive_path,
            &[
                ("engine/android/scrcpy", &content, 0o755),
                (MANIFEST_NAME, &manifest, 0o644),
            ],
        );

        let mut install_target = target;
        install_target.sha256 = sha256_file(&archive_path).expect("archive hash should be read");
        let install_directory = temp.path().join("installed");
        install_archive(
            &archive_path,
            &install_directory,
            None,
            &install_target,
            MANIFEST_NAME,
            "1.2.3",
            &[RuntimeGroup::Android],
        )
        .expect("valid archive should install");

        let installed = install_directory.join("android/scrcpy");
        assert_eq!(
            std::fs::read(&installed).expect("runtime should exist"),
            content
        );
        assert!(super::is_runtime_complete_at(
            &install_directory,
            &install_target
        ));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(installed)
                    .expect("installed runtime metadata should exist")
                    .permissions()
                    .mode()
                    & 0o777,
                0o755
            );
        }
    }

    #[test]
    fn rejects_archive_traversal_before_writing_outside_staging() {
        let temp = tempdir().expect("temporary directory should be created");
        let archive_path = temp.path().join("traversal.zip");
        let staging = temp.path().join("staging");
        write_archive(&archive_path, &[("../escape", b"bad", 0o644)]);

        let error =
            extract_and_validate(&archive_path, &staging, &target(), MANIFEST_NAME, "1.2.3")
                .expect_err("traversal path should be rejected");

        assert!(matches!(error, EnginePackageError::InvalidArchive(_)));
        assert!(!temp.path().join("escape").exists());
    }

    #[test]
    fn rejects_unsafe_zip_entry_path_forms() {
        let temp = tempdir().expect("temporary directory should be created");
        let staging = temp.path().join("staging");
        for (index, path) in [
            "../escape",
            "/engine/android/scrcpy",
            "engine\\android\\scrcpy",
            "engine/C:/scrcpy",
            "engine/./scrcpy",
            "engine/android/../scrcpy",
            "engine//scrcpy",
            "engine/bonjour/dnssd.dll",
            "engine/bonjour",
        ]
        .into_iter()
        .enumerate()
        {
            let archive_path = temp.path().join(format!("unsafe-{index}.zip"));
            write_archive(&archive_path, &[(path, b"bad", 0o644)]);
            let error =
                extract_and_validate(&archive_path, &staging, &target(), MANIFEST_NAME, "1.2.3")
                    .expect_err("unsafe ZIP path should be rejected");
            assert!(
                matches!(error, EnginePackageError::InvalidArchive(_)),
                "did not reject {path}"
            );
        }
    }

    #[test]
    fn rejects_case_insensitive_duplicate_archive_entries() {
        let temp = tempdir().expect("temporary directory should be created");
        let archive_path = temp.path().join("duplicate.zip");
        let staging = temp.path().join("staging");
        write_archive(
            &archive_path,
            &[
                ("engine/android/scrcpy", b"first", 0o644),
                ("ENGINE/ANDROID/SCRCPY", b"second", 0o644),
            ],
        );

        let error =
            extract_and_validate(&archive_path, &staging, &target(), MANIFEST_NAME, "1.2.3")
                .expect_err("duplicate paths should be rejected");

        assert!(matches!(error, EnginePackageError::InvalidArchive(_)));
    }

    #[test]
    fn rejects_a_symbolic_link_zip_entry() {
        let temp = tempdir().expect("temporary directory should be created");
        let archive_path = temp.path().join("symlink.zip");
        let staging = temp.path().join("staging");
        write_archive(
            &archive_path,
            &[("engine/android/scrcpy", b"target", 0o777)],
        );
        mark_first_entry_as_symlink(&archive_path);

        let error =
            extract_and_validate(&archive_path, &staging, &target(), MANIFEST_NAME, "1.2.3")
                .expect_err("symbolic link should be rejected");

        assert!(matches!(error, EnginePackageError::InvalidArchive(_)));
        assert!(!staging.join("engine/android/scrcpy").exists());
    }

    #[test]
    fn rejects_manifest_file_hash_mismatch() {
        let temp = tempdir().expect("temporary directory should be created");
        let archive_path = temp.path().join("bad-hash.zip");
        let staging = temp.path().join("staging");
        let content = runtime_bytes();
        let target = target();
        let mut manifest: PackageManifest =
            serde_json::from_slice(&manifest_bytes(&target, &content)).expect("manifest is valid");
        manifest.files[0].sha256 = "0".repeat(64);
        let manifest = serde_json::to_vec(&manifest).expect("manifest should serialize");
        write_archive(
            &archive_path,
            &[
                ("engine/android/scrcpy", &content, 0o755),
                (MANIFEST_NAME, &manifest, 0o644),
            ],
        );

        let error = extract_and_validate(&archive_path, &staging, &target, MANIFEST_NAME, "1.2.3")
            .expect_err("manifest hash mismatch should be rejected");

        assert!(matches!(error, EnginePackageError::InvalidArchive(_)));
    }
}
