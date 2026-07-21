use crate::UserRepoSetting;
use crate::environment::PackageCollection;
use crate::io;
use crate::io::DefaultEnvironmentIo;
use crate::utils::{deserialize_value, expect_object, save_json, try_load_json_value};
use serde_json::{Map, Number, Value};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
#[allow(clippy::struct_excessive_bools)]
struct AsJson {
    path_to_unity_exe: Box<str>,
    path_to_unity_hub: Box<str>,
    // The current VPM toolchain has two places of storing user projects: `settings.json` and `vcc.litedb`.
    // Currently, `settings.json` is the single source of truth, and VCC will always copy
    // information of `settings.json` to `vcc.litedb`.
    //
    // However, it's announced that future VCC will remove copying `settings.json` to `vcc.litedb`.
    // There's no detailed documentation on how `settings.json` would be when migration removal becomes true.
    // However, we can assume the `userProjects` key will be absent from `settings.json` and `vcc.litedb` become
    // the single source of truth (opposite to current `settings.json`).
    //
    // To support reading the settings.json for both versions and writing for both versions
    // 1) vrc-get will skip copying the data from 'userProjects' to vcc.litedb if 'userProjects' is absent,
    //      for future VCC compatibility
    // 2) vrc-get will always emit 'userProjects' key even if 'userProjects' is absent.
    //    The future VCC will just remove 'userProjects' so this should not cause a problem,
    //       and older VCC will become compatible since 'userProjects' can become single source of truth
    //
    // See https://github.com/vrchat-community/creator-companion/issues/400#issuecomment-1855484391
    // See https://vcc.docs.vrchat.com/news/release-2.2.0/#important-notes-for-tool-developers
    user_projects: Option<Vec<Box<str>>>,
    unity_editors: Vec<Box<str>>,
    preferred_unity_editors: JsonObject,
    // In the current VCC, this path will be reset to default if it's null
    // and vrc-get prefers another path the VCC's one so keep null if not set
    default_project_path: Option<Box<str>>,
    last_ui_state: i64,
    skip_unity_auto_find: bool,
    user_package_folders: Vec<PathBuf>,
    window_size_data: JsonObject,
    skip_requirements: bool,
    last_news_update: Box<str>,
    allow_pii: bool,
    // In the current VCC, this path will be reset to default if it's null
    // and vrc-get prefers another path the VCC's one so keep null if not set
    project_backup_path: Option<Box<str>>,
    show_prerelease_packages: bool,
    track_community_repos: bool,
    selected_providers: u64,
    last_selected_project: Box<str>,
    user_repos: Vec<UserRepoSetting>,
    rest: JsonObject,
}

type JsonObject = Map<String, Value>;

impl AsJson {
    fn from_json_value(value: Value) -> Result<Self, String> {
        let mut object = expect_object(value)?;
        Ok(Self {
            path_to_unity_exe: take_box_str(&mut object, "pathToUnityExe")?,
            path_to_unity_hub: take_box_str(&mut object, "pathToUnityHub")?,
            user_projects: take_optional_value(&mut object, "userProjects")?,
            unity_editors: take_vec(&mut object, "unityEditors")?,
            preferred_unity_editors: take_map(&mut object, "preferredUnityEditors")?,
            default_project_path: take_optional_value(&mut object, "defaultProjectPath")?,
            last_ui_state: take_value(&mut object, "lastUIState")?,
            skip_unity_auto_find: take_value(&mut object, "skipUnityAutoFind")?,
            user_package_folders: take_vec(&mut object, "userPackageFolders")?,
            window_size_data: take_map(&mut object, "windowSizeData")?,
            skip_requirements: take_value(&mut object, "skipRequirements")?,
            last_news_update: take_box_str(&mut object, "lastNewsUpdate")?,
            allow_pii: take_value(&mut object, "allowPii")?,
            project_backup_path: take_optional_value(&mut object, "projectBackupPath")?,
            show_prerelease_packages: take_value(&mut object, "showPrereleasePackages")?,
            track_community_repos: take_value(&mut object, "trackCommunityRepos")?,
            selected_providers: take_value(&mut object, "selectedProviders")?,
            last_selected_project: take_box_str(&mut object, "lastSelectedProject")?,
            user_repos: take_user_repos(&mut object, "userRepos")?,
            rest: object,
        })
    }

    fn to_json_value(&self) -> Value {
        let mut object = self.rest.clone();
        object.insert(
            "pathToUnityExe".to_owned(),
            Value::String(self.path_to_unity_exe.to_string()),
        );
        object.insert(
            "pathToUnityHub".to_owned(),
            Value::String(self.path_to_unity_hub.to_string()),
        );
        if let Some(user_projects) = &self.user_projects {
            object.insert(
                "userProjects".to_owned(),
                Value::Array(user_projects.iter().map(|s| Value::String(s.to_string())).collect()),
            );
        } else {
            object.remove("userProjects");
        }
        object.insert(
            "unityEditors".to_owned(),
            Value::Array(self.unity_editors.iter().map(|s| Value::String(s.to_string())).collect()),
        );
        object.insert(
            "preferredUnityEditors".to_owned(),
            Value::Object(self.preferred_unity_editors.clone()),
        );
        object.insert(
            "defaultProjectPath".to_owned(),
            self.default_project_path
                .as_ref()
                .map(|x| Value::String(x.to_string()))
                .unwrap_or(Value::Null),
        );
        object.insert(
            "lastUIState".to_owned(),
            Value::Number(Number::from(self.last_ui_state)),
        );
        object.insert(
            "skipUnityAutoFind".to_owned(),
            Value::Bool(self.skip_unity_auto_find),
        );
        object.insert(
            "userPackageFolders".to_owned(),
            Value::Array(
                self.user_package_folders
                    .iter()
                    .map(|p| Value::String(p.display().to_string()))
                    .collect(),
            ),
        );
        object.insert(
            "windowSizeData".to_owned(),
            Value::Object(self.window_size_data.clone()),
        );
        object.insert(
            "skipRequirements".to_owned(),
            Value::Bool(self.skip_requirements),
        );
        object.insert(
            "lastNewsUpdate".to_owned(),
            Value::String(self.last_news_update.to_string()),
        );
        object.insert("allowPii".to_owned(), Value::Bool(self.allow_pii));
        object.insert(
            "projectBackupPath".to_owned(),
            self.project_backup_path
                .as_ref()
                .map(|x| Value::String(x.to_string()))
                .unwrap_or(Value::Null),
        );
        object.insert(
            "showPrereleasePackages".to_owned(),
            Value::Bool(self.show_prerelease_packages),
        );
        object.insert(
            "trackCommunityRepos".to_owned(),
            Value::Bool(self.track_community_repos),
        );
        object.insert(
            "selectedProviders".to_owned(),
            Value::Number(Number::from(self.selected_providers)),
        );
        object.insert(
            "lastSelectedProject".to_owned(),
            Value::String(self.last_selected_project.to_string()),
        );
        object.insert(
            "userRepos".to_owned(),
            Value::Array(self.user_repos.iter().map(|r| r.to_json_value()).collect()),
        );
        Value::Object(object)
    }
}

fn take_box_str(object: &mut JsonObject, key: &str) -> Result<Box<str>, String> {
    Ok(match object.remove(key) {
        Some(value) => deserialize_value(value)?,
        None => "".into(),
    })
}

fn take_optional_value<T: serde::de::DeserializeOwned>(
    object: &mut JsonObject,
    key: &str,
) -> Result<Option<T>, String> {
    match object.remove(key) {
        Some(value) => deserialize_value(value),
        None => Ok(None),
    }
}

fn take_value<T: serde::de::DeserializeOwned + Default>(
    object: &mut JsonObject,
    key: &str,
) -> Result<T, String> {
    match object.remove(key) {
        Some(value) => deserialize_value(value),
        None => Ok(T::default()),
    }
}

fn take_vec<T: serde::de::DeserializeOwned>(
    object: &mut JsonObject,
    key: &str,
) -> Result<Vec<T>, String> {
    Ok(match object.remove(key) {
        Some(value) => deserialize_value(value)?,
        None => Vec::new(),
    })
}

fn take_user_repos(object: &mut JsonObject, key: &str) -> Result<Vec<UserRepoSetting>, String> {
    match object.remove(key) {
        Some(Value::Array(arr)) => arr.into_iter().map(UserRepoSetting::from_json_value).collect(),
        Some(_) => Err(format!("invalid {key}")),
        None => Ok(Vec::new()),
    }
}

fn take_map(object: &mut JsonObject, key: &str) -> Result<JsonObject, String> {
    Ok(match object.remove(key) {
        Some(value) => expect_object(value)?,
        None => Map::new(),
    })
}

#[derive(Default, Debug, Clone)]
pub(crate) struct VpmSettings {
    parsed: AsJson,
}

const JSON_PATH: &str = "settings.json";
const ALT_JSON_PATH: &str = "vrc-get/vcc-settings-backup.json";

impl VpmSettings {
    pub async fn load(io: &DefaultEnvironmentIo) -> io::Result<Option<Self>> {
        Self::load_inner(io, JSON_PATH).await
    }

    pub async fn load_alt(io: &DefaultEnvironmentIo) -> io::Result<Option<Self>> {
        let mut settings = Self::load_inner(io, ALT_JSON_PATH).await?;

        // We use data from vcc.litedb for the source of the projecs list since it's much reliable source.
        if let Some(ref mut settings) = settings {
            settings.parsed.user_projects = None;
        }

        Ok(settings)
    }

    async fn load_inner(io: &DefaultEnvironmentIo, path: &str) -> io::Result<Option<Self>> {
        let Some(value) = try_load_json_value(io, path.as_ref()).await? else {
            log::debug!("VpmSettings Configuration file not found at {path}");
            return Ok(None);
        };
        let parsed = AsJson::from_json_value(value).map_err(|err| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("syntax error loading {path}: {err}"),
            )
        })?;

        log::debug!("Parsed VpmSettings at {path}");

        Ok(Some(Self { parsed }))
    }

    pub(crate) fn user_repos(&self) -> &[UserRepoSetting] {
        &self.parsed.user_repos
    }

    pub(crate) fn user_package_folders(&self) -> &[PathBuf] {
        &self.parsed.user_package_folders
    }

    pub fn remove_user_package_folder(&mut self, path: &Path) {
        self.parsed.user_package_folders.retain(|x| x != path);
    }

    pub(crate) fn add_user_package_folder(&mut self, path: PathBuf) {
        self.parsed.user_package_folders.push(path);
    }

    pub(crate) fn update_id(&mut self, collection: &PackageCollection) -> bool {
        let json = &mut self.parsed;
        let mut changed = false;

        for repo in &mut json.user_repos {
            if let Some(cache) = collection.repositories.get_by_path(repo.local_path())
                && cache.repo.id() != repo.id()
            {
                repo.id = cache.repo.id().map(|x| x.into());
                changed = true;
            }
        }

        changed
    }

    pub fn retain_user_repos(
        &mut self,
        mut f: impl FnMut(&UserRepoSetting) -> bool,
    ) -> Vec<UserRepoSetting> {
        self.parsed
            .user_repos
            .extract_if(.., |r| !f(r))
            .collect::<Vec<_>>()
    }

    pub fn remove_user_repo_at_index(&mut self, index: usize) -> Option<UserRepoSetting> {
        let repos = &mut self.parsed.user_repos;
        if index < repos.len() {
            Some(repos.remove(index))
        } else {
            None
        }
    }

    pub fn reorder_user_repos_by_indices(&mut self, indices: &[usize]) {
        let mut pool: Vec<Option<UserRepoSetting>> = std::mem::take(&mut self.parsed.user_repos)
            .into_iter()
            .map(Some)
            .collect();
        let mut result = Vec::with_capacity(pool.len());
        for &idx in indices {
            if let Some(slot) = pool.get_mut(idx)
                && let Some(repo) = slot.take()
            {
                result.push(repo);
            }
        }
        result.extend(pool.into_iter().flatten());
        self.parsed.user_repos = result;
    }

    pub(crate) fn add_user_repo(&mut self, repo: UserRepoSetting) {
        self.parsed.user_repos.push(repo);
    }

    pub(crate) fn show_prerelease_packages(&self) -> bool {
        self.parsed.show_prerelease_packages
    }

    pub(crate) fn set_show_prerelease_packages(&mut self, value: bool) {
        self.parsed.show_prerelease_packages = value;
    }

    pub(crate) fn default_project_path(&self) -> Option<&str> {
        self.parsed.default_project_path.as_deref()
    }

    pub(crate) fn set_default_project_path(&mut self, value: &str) {
        self.parsed.default_project_path = Some(value.into());
    }

    pub(crate) fn project_backup_path(&self) -> Option<&str> {
        self.parsed.project_backup_path.as_deref()
    }

    pub(crate) fn set_project_backup_path(&mut self, value: &str) {
        self.parsed.project_backup_path = Some(value.into());
    }

    pub(crate) fn unity_hub(&self) -> &str {
        &self.parsed.path_to_unity_hub
    }

    pub(crate) fn set_unity_hub(&mut self, path: &str) {
        self.parsed.path_to_unity_hub = path.into();
    }

    pub async fn save(&self, io: &DefaultEnvironmentIo) -> io::Result<()> {
        let value = self.parsed.to_json_value();
        save_json(io, JSON_PATH.as_ref(), &value).await?;
        save_json(io, ALT_JSON_PATH.as_ref(), &value).await
    }
}

#[cfg(feature = "experimental-project-management")]
impl VpmSettings {
    pub(crate) fn user_projects(&self) -> Option<&[Box<str>]> {
        self.parsed.user_projects.as_deref()
    }

    pub(crate) fn retain_user_projects(
        &mut self,
        mut f: impl FnMut(&str) -> bool,
    ) -> Option<Vec<Box<str>>> {
        Some(
            (self.parsed.user_projects.as_mut())?
                .extract_if(.., |x| !f(x))
                .collect(),
        )
    }

    pub(crate) fn remove_user_project(&mut self, path: &str) {
        if let Some(x) = self.parsed.user_projects.as_mut() {
            x.retain(|x| x.as_ref() != path)
        }
    }

    pub(crate) fn add_user_project(&mut self, path: &str) {
        self.parsed
            .user_projects
            .get_or_insert_default()
            .insert(0, path.into());
    }
}
