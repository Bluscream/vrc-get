mod partial_unity_version;
mod yank_state;

use crate::utils::{
    deserialize_value, expect_object, take_default_with, take_optional, take_optional_with,
    take_required, value_to_index_map,
};
use crate::version::{Version, VersionRange};
use indexmap::IndexMap;
use serde::{Deserialize, Deserializer};
use serde_json::Value;
use std::collections::HashMap;
use std::str::FromStr;
use url::Url;

use crate::package_manifest::yank_state::YankState;
pub use partial_unity_version::PartialUnityVersion;

#[derive(Debug, Clone)]
pub struct PackageManifest {
    name: Box<str>,
    version: Version,
    display_name: Option<Box<str>>,
    description: Option<Box<str>>,
    unity: Option<PartialUnityVersion>,
    url: Option<Url>,
    zip_sha_256: Option<Box<str>>,
    vpm_dependencies: IndexMap<Box<str>, VersionRange>,
    legacy_folders: HashMap<Box<str>, Option<Box<str>>>,
    legacy_files: HashMap<Box<str>, Option<Box<str>>>,
    legacy_packages: Vec<Box<str>>,
    headers: IndexMap<Box<str>, Box<str>>,
    changelog_url: Option<Url>,
    documentation_url: Option<Url>,
    keywords: Vec<Box<str>>,
    vrc_get: VrcGetMeta,
}

#[derive(Debug, Clone, Default)]
pub(super) struct VrcGetMeta {
    yanked: YankState,
    aliases: Vec<Box<str>>,
}

impl<'de> Deserialize<'de> for PackageManifest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        Self::from_json_value(value).map_err(serde::de::Error::custom)
    }
}

fn parse_optional_url(value: Value) -> Result<Option<Url>, String> {
    let Some(url) = deserialize_value::<Option<String>>(value)? else {
        return Ok(None);
    };
    if url.trim().is_empty() {
        return Ok(None);
    }
    Url::parse(&url).map(Some).map_err(|err| err.to_string())
}

fn parse_default_index_map<T: serde::de::DeserializeOwned>(
    value: Value,
) -> Result<IndexMap<Box<str>, T>, String> {
    if value.is_null() {
        Ok(IndexMap::new())
    } else {
        value_to_index_map(value)
    }
}

fn parse_default_hash_map<T: serde::de::DeserializeOwned + Default>(
    value: Value,
) -> Result<HashMap<Box<str>, T>, String> {
    if value.is_null() {
        Ok(HashMap::new())
    } else {
        deserialize_value(value)
    }
}

fn parse_default_vec<T: serde::de::DeserializeOwned>(value: Value) -> Result<Vec<T>, String> {
    if value.is_null() {
        Ok(Vec::new())
    } else {
        deserialize_value(value)
    }
}

fn parse_vrc_get_meta(value: Value) -> Result<VrcGetMeta, String> {
    let mut object = expect_object(value)?;
    Ok(VrcGetMeta {
        yanked: take_default_with(&mut object, "yanked", deserialize_value::<YankState>)?,
        aliases: take_default_with(&mut object, "aliases", parse_default_vec::<Box<str>>)?,
    })
}

impl PackageManifest {
    pub(crate) fn from_json_value(value: Value) -> Result<Self, String> {
        let mut object = expect_object(value)?;
        Ok(Self {
            name: take_required(&mut object, "name")?,
            version: take_required(&mut object, "version")?,
            display_name: take_optional(&mut object, "displayName")?,
            description: take_optional(&mut object, "description")?,
            unity: take_optional(&mut object, "unity")?,
            url: take_optional_with(&mut object, "url", parse_optional_url)?.flatten(),
            zip_sha_256: take_optional(&mut object, "zipSHA256")?,
            vpm_dependencies: take_default_with(
                &mut object,
                "vpmDependencies",
                parse_default_index_map::<VersionRange>,
            )?,
            legacy_folders: take_default_with(
                &mut object,
                "legacyFolders",
                parse_default_hash_map::<Option<Box<str>>>,
            )?,
            legacy_files: take_default_with(
                &mut object,
                "legacyFiles",
                parse_default_hash_map::<Option<Box<str>>>,
            )?,
            legacy_packages: take_default_with(
                &mut object,
                "legacyPackages",
                parse_default_vec::<Box<str>>,
            )?,
            headers: take_default_with(
                &mut object,
                "headers",
                parse_default_index_map::<Box<str>>,
            )?,
            changelog_url: take_optional_with(&mut object, "changelogUrl", parse_optional_url)?
                .flatten(),
            documentation_url: take_optional_with(
                &mut object,
                "documentationUrl",
                parse_optional_url,
            )?
            .flatten(),
            keywords: take_default_with(&mut object, "keywords", parse_default_vec::<Box<str>>)?,
            vrc_get: take_default_with(&mut object, "vrc-get", parse_vrc_get_meta)?,
        })
    }

    pub(crate) fn from_loose_json_value(value: Value) -> Result<Self, String> {
        fn soft_value<T>(value: Value, f: impl FnOnce(Value) -> Result<T, String>) -> T
        where
            T: Default,
        {
            f(value).unwrap_or_default()
        }

        let mut object = expect_object(value)?;
        Ok(Self {
            name: take_required(&mut object, "name")?,
            version: take_required(&mut object, "version")?,
            display_name: object
                .remove("displayName")
                .map(|value| soft_value(value, deserialize_value::<Option<Box<str>>>))
                .flatten(),
            description: object
                .remove("description")
                .map(|value| soft_value(value, deserialize_value::<Option<Box<str>>>))
                .flatten(),
            unity: object
                .remove("unity")
                .map(|value| soft_value(value, deserialize_value::<Option<PartialUnityVersion>>))
                .flatten(),
            url: object
                .remove("url")
                .map(|value| soft_value(value, parse_optional_url))
                .flatten(),
            zip_sha_256: object
                .remove("zipSHA256")
                .map(|value| soft_value(value, deserialize_value::<Option<Box<str>>>))
                .flatten(),
            vpm_dependencies: object
                .remove("vpmDependencies")
                .map(|value| soft_value(value, parse_default_index_map::<VersionRange>))
                .unwrap_or_default(),
            legacy_folders: object
                .remove("legacyFolders")
                .map(|value| soft_value(value, parse_default_hash_map::<Option<Box<str>>>))
                .unwrap_or_default(),
            legacy_files: object
                .remove("legacyFiles")
                .map(|value| soft_value(value, parse_default_hash_map::<Option<Box<str>>>))
                .unwrap_or_default(),
            legacy_packages: object
                .remove("legacyPackages")
                .map(|value| soft_value(value, parse_default_vec::<Box<str>>))
                .unwrap_or_default(),
            headers: object
                .remove("headers")
                .map(|value| soft_value(value, parse_default_index_map::<Box<str>>))
                .unwrap_or_default(),
            changelog_url: object
                .remove("changelogUrl")
                .map(|value| soft_value(value, parse_optional_url))
                .flatten(),
            documentation_url: object
                .remove("documentationUrl")
                .map(|value| soft_value(value, parse_optional_url))
                .flatten(),
            keywords: object
                .remove("keywords")
                .map(|value| soft_value(value, parse_default_vec::<Box<str>>))
                .unwrap_or_default(),
            vrc_get: object
                .remove("vrc-get")
                .map(|value| soft_value(value, parse_vrc_get_meta))
                .unwrap_or_default(),
        })
    }

    #[cfg(test)]
    fn from_loose_str(json: &str) -> Result<Self, String> {
        let value = serde_json::from_str(json).map_err(|err| err.to_string())?;
        Self::from_loose_json_value(value)
    }

    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn version(&self) -> &Version {
        &self.version
    }
    pub fn vpm_dependencies(&self) -> &IndexMap<Box<str>, VersionRange> {
        &self.vpm_dependencies
    }
    pub fn legacy_folders(&self) -> &HashMap<Box<str>, Option<Box<str>>> {
        &self.legacy_folders
    }

    pub fn legacy_files(&self) -> &HashMap<Box<str>, Option<Box<str>>> {
        &self.legacy_files
    }
    pub fn headers(&self) -> &IndexMap<Box<str>, Box<str>> {
        &self.headers
    }

    pub fn legacy_packages(&self) -> &[Box<str>] {
        self.legacy_packages.as_slice()
    }
    pub fn display_name(&self) -> Option<&str> {
        self.display_name.as_deref()
    }
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }
    pub fn url(&self) -> Option<&Url> {
        self.url.as_ref()
    }
    pub fn zip_sha_256(&self) -> Option<&str> {
        self.zip_sha_256.as_deref()
    }
    pub fn changelog_url(&self) -> Option<&Url> {
        self.changelog_url.as_ref()
    }
    pub fn documentation_url(&self) -> Option<&Url> {
        self.documentation_url.as_ref()
    }
    pub fn unity(&self) -> Option<&PartialUnityVersion> {
        self.unity.as_ref()
    }
    pub fn is_yanked(&self) -> bool {
        self.vrc_get.yanked.is_yanked()
    }
    // TODO: deprecate aliases on next minor release
    pub fn aliases(&self) -> &[Box<str>] {
        self.vrc_get.aliases.as_slice()
    }
    pub fn keywords(&self) -> &[Box<str>] {
        self.keywords.as_slice()
    }
}

/// Constructing PackageJson. Especially for testing.
impl PackageManifest {
    pub fn new(name: impl Into<Box<str>>, version: Version) -> Self {
        Self {
            name: name.into(),
            version,
            display_name: None,
            description: None,
            vpm_dependencies: IndexMap::new(),
            url: None,
            unity: None,
            legacy_folders: HashMap::new(),
            legacy_files: HashMap::new(),
            legacy_packages: Vec::new(),
            headers: IndexMap::new(),
            vrc_get: VrcGetMeta::default(),
            zip_sha_256: None,
            changelog_url: None,
            documentation_url: None,
            keywords: Vec::new(),
        }
    }

    pub fn add_vpm_dependency(mut self, name: impl Into<Box<str>>, range: &str) -> Self {
        self.vpm_dependencies
            .insert(name.into(), VersionRange::from_str(range).unwrap());
        self
    }

    pub fn add_legacy_package(mut self, name: impl Into<Box<str>>) -> Self {
        self.legacy_packages.push(name.into());
        self
    }

    pub fn add_legacy_folder(
        mut self,
        path: impl Into<Box<str>>,
        guid: impl Into<Box<str>>,
    ) -> Self {
        self.legacy_folders.insert(path.into(), Some(guid.into()));
        self
    }

    pub fn add_legacy_file(mut self, path: impl Into<Box<str>>, guid: impl Into<Box<str>>) -> Self {
        self.legacy_files.insert(path.into(), Some(guid.into()));
        self
    }
}

#[test]
fn deserialize_partially_bad() {
    let json = r#"{
        "name": "vrc-get-vpm",
        "version": "0.1.0",
        "vpmDependencies": {
            "vrc-get": ">=0.1.0"
        },
        "comment": "Thre following is duplicated key url",
        "legacyPackages": ["vrc-get"],
        "legacyPackages": ["vrc-2"],
        "comment": "Thre following is invalid url",
        "changelog_url": "",
        "url": "",
        "vrc-get": {
            "yanked": false,
            "aliases": ["vpm"]
        }
    }"#;
    let package_json = PackageManifest::from_loose_str(json).unwrap();
    assert_eq!(package_json.name(), "vrc-get-vpm");
    assert_eq!(package_json.version(), &Version::new(0, 1, 0));
    assert_eq!(package_json.vpm_dependencies(), &{
        let mut map = IndexMap::new();
        map.insert(
            "vrc-get".into(),
            VersionRange::same_or_later(Version::new(0, 1, 0)),
        );
        map
    });
    assert_eq!(package_json.legacy_packages(), &["vrc-2".into()]);
    assert!(!package_json.is_yanked());
    assert_eq!(package_json.aliases(), &["vpm".into()]);
    assert_eq!(package_json.changelog_url(), None);
}

#[test]
fn deserialize_null_on_dependencies() {
    let json = r##"{
      "name": "com.kibalab.materialmerger",
      "displayName": "Material Merger",
      "description": "Unity Editor tool that merges multiple materials/textures into an atlas-based workflow. specifically designed for VRChat world/avatar optimization.",
      "version": "0.1.0",
      "unity": "2022.3",
      "url": "https://github.com/kibalab/material-merger/releases/download/0.1.0/com.kibalab.materialmerger-0.1.0.zip",
      "author": {
        "name": "KIBA",
        "email": "root@kiba.red",
        "url": "https://vpm.kiba.red"
      },
      "dependencies": null,
      "vpmDependencies": null,
      "samples": null,
      "zipSHA256": "0e201b9a1ed9f0e3a9c16b8f765605e8aa0c9aebf9a315c04bc67f6ebe2485f8"
    }"##;
    let package_json: PackageManifest = serde_json::from_str(json).unwrap();
    assert_eq!(package_json.name(), "com.kibalab.materialmerger");
    assert_eq!(package_json.version(), &Version::new(0, 1, 0));
    assert!(package_json.vpm_dependencies().is_empty());
}

#[test]
fn deserialize_empty_documentation() {
    let json = r##"{
      "name": "net.yarukizero.vrchat.shizuku",
      "displayName": "Shizuku",
      "version": "0.0.0",
      "unity": "2022.3",
      "description": "スクリプトでいい感じに定義したい",
      "vpmDependencies": {
        "nadena.dev.modular-avatar": ">=1.9.10"
      },
      "changelogUrl": " ",
      "author": {
        "name": "azumyar",
        "url": "https://github.com/azumyar"
      },
      "documentationUrl": "",
      "license": "MIT",
      "zipSHA256": "22a143ed75c429a471ffd784102d2fb577c56b010b49439b5930cbb2df820f8b",
      "url": "https://github.com/azumyar/vrchat-shizuku/releases/download/0.0.0/net.yarukizero.vrchat.shizuku-0.0.0.zip"
    }"##;
    let package_json: PackageManifest = serde_json::from_str(json).unwrap();
    assert_eq!(package_json.name(), "net.yarukizero.vrchat.shizuku");
    assert_eq!(package_json.version(), &Version::new(0, 0, 0));
    assert_eq!(package_json.documentation_url(), None);
    assert_eq!(package_json.changelog_url(), None);
}
