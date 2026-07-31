use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const SCHEMA_VERSION: u32 = 1;
const PROFILES_FILE: &str = "profiles.json";
const ACTIVE_FILE: &str = "active-profile";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TunnelSettings {
    pub provider: String,
    pub executable: String,
    pub extra_args: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Profile {
    pub id: String,
    pub display_name: String,
    pub roots: Vec<String>,
    pub port: u16,
    pub host: String,
    pub max_concurrency: usize,
    pub max_io_concurrency: usize,
    pub read_only: bool,
    pub follow_links: bool,
    pub max_read_mb: usize,
    pub max_write_mb: usize,
    pub tree_max_depth: usize,
    pub tree_max_entries: usize,
    pub tree_max_warnings: usize,
    pub patch_max_kb: usize,
    pub patch_preview_kb: usize,
    pub max_search_results: usize,
    pub search_max_concurrency: usize,
    pub search_worker_threads: usize,
    pub regex_cache_capacity: usize,
    pub include_hidden: bool,
    pub respect_gitignore: bool,
    pub terminal_enabled: bool,
    pub terminal_max_concurrency: usize,
    pub terminal_default_timeout_ms: u64,
    pub terminal_max_timeout_ms: u64,
    pub terminal_max_output_mb: usize,
    pub terminal_max_read_kb: usize,
    pub terminal_max_wait_ms: u64,
    pub terminal_session_retention_ms: u64,
    pub oauth_enabled: bool,
    pub oauth_require_auth: bool,
    pub oauth_issuer: Option<String>,
    pub log_tools: bool,
    pub tunnel: Option<TunnelSettings>,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            id: new_id(),
            display_name: "My project".to_owned(),
            roots: Vec::new(),
            port: 8000,
            host: "127.0.0.1".to_owned(),
            max_concurrency: 32,
            max_io_concurrency: 16,
            read_only: true,
            follow_links: false,
            max_read_mb: 8,
            max_write_mb: 8,
            tree_max_depth: 8,
            tree_max_entries: 1000,
            tree_max_warnings: 32,
            patch_max_kb: 1024,
            patch_preview_kb: 16,
            max_search_results: 1000,
            search_max_concurrency: 4,
            search_worker_threads: 4,
            regex_cache_capacity: 64,
            include_hidden: false,
            respect_gitignore: true,
            terminal_enabled: false,
            terminal_max_concurrency: 2,
            terminal_default_timeout_ms: 30_000,
            terminal_max_timeout_ms: 300_000,
            terminal_max_output_mb: 4,
            terminal_max_read_kb: 256,
            terminal_max_wait_ms: 30_000,
            terminal_session_retention_ms: 300_000,
            oauth_enabled: true,
            oauth_require_auth: false,
            oauth_issuer: None,
            log_tools: true,
            tunnel: Some(TunnelSettings {
                provider: "cloudflared".to_owned(),
                executable: String::new(),
                extra_args: String::new(),
            }),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfileDocument {
    schema_version: u32,
    profiles: Vec<Profile>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProfileState {
    pub schema_version: u32,
    pub active_profile_id: String,
    pub profiles: Vec<Profile>,
}

pub struct ProfileStore {
    directory: PathBuf,
}

impl ProfileStore {
    pub fn new(directory: PathBuf) -> Self {
        Self { directory }
    }

    pub fn load_or_initialize(&self) -> Result<ProfileState> {
        fs::create_dir_all(&self.directory).with_context(|| {
            format!(
                "cannot create app data directory {}",
                self.directory.display()
            )
        })?;
        if !self.profiles_path().exists() {
            let profile = Profile::default();
            let state = ProfileState {
                schema_version: SCHEMA_VERSION,
                active_profile_id: profile.id.clone(),
                profiles: vec![profile],
            };
            self.write_state(&state)?;
            return Ok(state);
        }
        let text = fs::read_to_string(self.profiles_path()).context("cannot read profiles.json")?;
        let document: ProfileDocument =
            serde_json::from_str(&text).context("profiles.json is corrupted")?;
        let document = migrate(document)?;
        if document.profiles.is_empty() {
            bail!("profiles.json contains no profiles");
        }
        let active = fs::read_to_string(self.active_path())
            .ok()
            .map(|value| value.trim().to_owned())
            .filter(|id| document.profiles.iter().any(|profile| profile.id == *id))
            .unwrap_or_else(|| document.profiles[0].id.clone());
        Ok(ProfileState {
            schema_version: document.schema_version,
            active_profile_id: active,
            profiles: document.profiles,
        })
    }

    pub fn reset(&self) -> Result<ProfileState> {
        let profile = Profile::default();
        let state = ProfileState {
            schema_version: SCHEMA_VERSION,
            active_profile_id: profile.id.clone(),
            profiles: vec![profile],
        };
        self.write_state(&state)?;
        Ok(state)
    }

    pub fn save_profile(&self, profile: Profile) -> Result<ProfileState> {
        validate_identity(&profile)?;
        let mut state = self.load_or_initialize()?;
        let existing = state
            .profiles
            .iter_mut()
            .find(|item| item.id == profile.id)
            .context("profile does not exist")?;
        *existing = profile;
        self.write_state(&state)?;
        Ok(state)
    }

    pub fn create_profile(&self, name: String, duplicate_id: Option<&str>) -> Result<ProfileState> {
        let name = required_name(&name)?;
        let mut state = self.load_or_initialize()?;
        let mut profile = match duplicate_id {
            Some(id) => state
                .profiles
                .iter()
                .find(|profile| profile.id == id)
                .cloned()
                .context("source profile does not exist")?,
            None => Profile::default(),
        };
        profile.id = new_id();
        profile.display_name = name;
        state.active_profile_id = profile.id.clone();
        state.profiles.push(profile);
        self.write_state(&state)?;
        Ok(state)
    }

    pub fn rename_profile(&self, id: &str, name: String) -> Result<ProfileState> {
        let name = required_name(&name)?;
        let mut state = self.load_or_initialize()?;
        state
            .profiles
            .iter_mut()
            .find(|profile| profile.id == id)
            .context("profile does not exist")?
            .display_name = name;
        self.write_state(&state)?;
        Ok(state)
    }

    pub fn delete_profile(&self, id: &str) -> Result<ProfileState> {
        let mut state = self.load_or_initialize()?;
        if state.profiles.len() == 1 {
            bail!("the last profile cannot be deleted");
        }
        let before = state.profiles.len();
        state.profiles.retain(|profile| profile.id != id);
        if state.profiles.len() == before {
            bail!("profile does not exist");
        }
        if state.active_profile_id == id {
            state.active_profile_id = state.profiles[0].id.clone();
        }
        self.write_state(&state)?;
        Ok(state)
    }

    pub fn set_active(&self, id: &str) -> Result<ProfileState> {
        let mut state = self.load_or_initialize()?;
        if !state.profiles.iter().any(|profile| profile.id == id) {
            bail!("profile does not exist");
        }
        state.active_profile_id = id.to_owned();
        self.write_active(id)?;
        Ok(state)
    }

    pub fn config_path(&self, id: &str) -> PathBuf {
        self.directory.join(format!("profile-{id}.toml"))
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    fn profiles_path(&self) -> PathBuf {
        self.directory.join(PROFILES_FILE)
    }

    fn active_path(&self) -> PathBuf {
        self.directory.join(ACTIVE_FILE)
    }

    fn write_state(&self, state: &ProfileState) -> Result<()> {
        fs::create_dir_all(&self.directory)?;
        let document = ProfileDocument {
            schema_version: SCHEMA_VERSION,
            profiles: state.profiles.clone(),
        };
        atomic_write(
            &self.profiles_path(),
            serde_json::to_string_pretty(&document)?.as_bytes(),
        )?;
        self.write_active(&state.active_profile_id)
    }

    fn write_active(&self, id: &str) -> Result<()> {
        atomic_write(&self.active_path(), id.as_bytes())
    }
}

fn migrate(document: ProfileDocument) -> Result<ProfileDocument> {
    match document.schema_version {
        SCHEMA_VERSION => Ok(document),
        0 => Ok(ProfileDocument {
            schema_version: SCHEMA_VERSION,
            profiles: document.profiles,
        }),
        version => bail!(
            "unsupported profile schema version {version}; this app supports version {SCHEMA_VERSION}"
        ),
    }
}

fn validate_identity(profile: &Profile) -> Result<()> {
    if profile.id.trim().is_empty() {
        bail!("profile id is required");
    }
    required_name(&profile.display_name)?;
    Ok(())
}

fn required_name(name: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty() {
        bail!("profile name is required");
    }
    Ok(name.to_owned())
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let temporary = path.with_extension("tmp");
    fs::write(&temporary, bytes)
        .with_context(|| format!("cannot write {}", temporary.display()))?;
    if path.exists() {
        fs::remove_file(path).with_context(|| format!("cannot replace {}", path.display()))?;
    }
    fs::rename(&temporary, path).with_context(|| format!("cannot save {}", path.display()))
}

fn new_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("p-{nanos}-{}", std::process::id())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persists_and_restores_active_profile() {
        let temp = tempfile::tempdir().unwrap();
        let store = ProfileStore::new(temp.path().to_owned());
        let initial = store.load_or_initialize().unwrap();
        let state = store.create_profile("Second".to_owned(), None).unwrap();
        assert_eq!(state.profiles.len(), 2);
        assert_ne!(initial.active_profile_id, state.active_profile_id);
        assert_eq!(
            store.load_or_initialize().unwrap().active_profile_id,
            state.active_profile_id
        );
    }

    #[test]
    fn supports_crud_and_protects_last_profile() {
        let temp = tempfile::tempdir().unwrap();
        let store = ProfileStore::new(temp.path().to_owned());
        let initial = store.load_or_initialize().unwrap();
        assert!(store.delete_profile(&initial.active_profile_id).is_err());
        let created = store
            .create_profile("Copy".to_owned(), Some(&initial.active_profile_id))
            .unwrap();
        let id = created.active_profile_id.clone();
        let renamed = store.rename_profile(&id, "Renamed".to_owned()).unwrap();
        assert_eq!(renamed.profiles.last().unwrap().display_name, "Renamed");
        assert_eq!(store.delete_profile(&id).unwrap().profiles.len(), 1);
    }

    #[test]
    fn reports_corruption_and_unknown_versions() {
        let temp = tempfile::tempdir().unwrap();
        let store = ProfileStore::new(temp.path().to_owned());
        fs::write(store.profiles_path(), "not json").unwrap();
        assert!(
            store
                .load_or_initialize()
                .unwrap_err()
                .to_string()
                .contains("corrupted")
        );
        fs::write(
            store.profiles_path(),
            r#"{"schema_version":99,"profiles":[]}"#,
        )
        .unwrap();
        assert!(
            store
                .load_or_initialize()
                .unwrap_err()
                .to_string()
                .contains("unsupported")
        );
    }

    #[test]
    fn migrates_version_zero_document() {
        let document = ProfileDocument {
            schema_version: 0,
            profiles: vec![Profile::default()],
        };
        let migrated = migrate(document).unwrap();
        assert_eq!(migrated.schema_version, SCHEMA_VERSION);
        assert_eq!(migrated.profiles.len(), 1);
    }

    #[test]
    fn profile_json_round_trip_preserves_configuration() {
        let profile = Profile::default();
        let json = serde_json::to_string(&profile).unwrap();
        assert_eq!(serde_json::from_str::<Profile>(&json).unwrap(), profile);
    }
}
