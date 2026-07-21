use crate::PackageManifest;
use crate::traits::HttpClient;
use crate::utils::{deserialize_json_slice, expect_object};
use crate::version::Version;
use crate::{VersionSelector, io};
use futures::prelude::*;
use indexmap::IndexMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::pin::pin;
use std::str::FromStr;
use url::Url;

type JsonMap = Map<String, Value>;

#[derive(Debug, Clone)]
pub struct RemoteRepository {
    actual: JsonMap,
    name: Option<Box<str>>,
    url: Option<Url>,
    id: Option<Box<str>>,
    packages: IndexMap<Box<str>, RemotePackages>,
}

impl RemoteRepository {
    pub fn parse(cache: JsonMap) -> io::Result<Self> {
        let mut actual = cache;
        let name = take_optional_string(&mut actual, "name")
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
        let url = take_optional_url(&mut actual, "url")
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
        let id = take_optional_string(&mut actual, "id")
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
        let packages = take_packages(&mut actual)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;

        Ok(Self {
            actual,
            name,
            url,
            id,
            packages,
        })
    }

    pub(crate) fn to_json_value(&self) -> Value {
        Value::Object(self.actual.clone())
    }

    pub async fn download(
        client: &impl HttpClient,
        url: &Url,
        headers: &IndexMap<Box<str>, Box<str>>,
    ) -> io::Result<(RemoteRepository, Option<Box<str>>)> {
        match Self::download_with_etag(client, url, headers, None).await {
            Ok(None) => unreachable!("downloading without etag should must return Ok(Some)"),
            Ok(Some(repo_and_etag)) => Ok(repo_and_etag),
            Err(err) => Err(err),
        }
    }

    pub async fn download_with_etag(
        client: &impl HttpClient,
        url: &Url,
        headers: &IndexMap<Box<str>, Box<str>>,
        current_etag: Option<&str>,
    ) -> io::Result<Option<(RemoteRepository, Option<Box<str>>)>> {
        let Some((stream, etag)) = client.get_with_etag(url, headers, current_etag).await? else {
            return Ok(None);
        };

        let mut bytes = Vec::new();
        pin!(stream).read_to_end(&mut bytes).await?;

        let no_bom = bytes
            .strip_prefix(b"\xEF\xBB\xBF")
            .unwrap_or(bytes.as_ref());
        let json = deserialize_json_slice(no_bom)?;

        let mut repo = RemoteRepository::parse(json)?;
        repo.set_url_if_none(|| url.clone());
        Ok(Some((repo, etag)))
    }

    pub(crate) fn set_id_if_none(&mut self, f: impl FnOnce() -> Box<str>) {
        if self.id.is_none() {
            let id = f();
            self.id = Some(id.clone());
            self.actual
                .insert("id".to_owned(), Value::String(id.into()));
        }
    }

    pub(crate) fn set_url_if_none(&mut self, f: impl FnOnce() -> Url) {
        if self.url.is_none() {
            let url = f();
            self.url = Some(url.clone());
            self.actual
                .insert("url".to_owned(), Value::String(url.to_string()));
        }
        if self.id.is_none() {
            let url = self.url.as_ref().unwrap().as_str().into();
            self.set_id_if_none(move || url);
        }
    }

    pub fn url(&self) -> Option<&Url> {
        self.url.as_ref()
    }

    pub(crate) fn set_url(&mut self, url: Url) {
        self.url = Some(url);
    }

    pub fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn get_versions_of(
        &self,
        package: &str,
    ) -> impl Iterator<Item = &'_ PackageManifest> + use<'_> {
        self.packages
            .get(package)
            .map(RemotePackages::all_versions)
            .into_iter()
            .flatten()
    }

    pub fn get_package(&self, package: &str) -> Option<&RemotePackages> {
        self.packages.get(package)
    }

    pub fn get_packages(&self) -> impl Iterator<Item = &'_ RemotePackages> {
        self.packages.values()
    }

    pub fn get_package_version(&self, name: &str, version: &Version) -> Option<&PackageManifest> {
        self.packages.get(name)?.versions.get(version)
    }
}

impl Serialize for RemoteRepository {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.actual.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for RemoteRepository {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::Error;
        let map = expect_object(Value::deserialize(deserializer)?).map_err(Error::custom)?;
        Self::parse(map).map_err(Error::custom)
    }
}

fn take_optional_string(object: &mut JsonMap, key: &str) -> Result<Option<Box<str>>, String> {
    match object.remove(key) {
        Some(value) => Ok(Some(crate::utils::deserialize_value::<Box<str>>(value)?)),
        None => Ok(None),
    }
}

fn take_optional_url(object: &mut JsonMap, key: &str) -> Result<Option<Url>, String> {
    match object.remove(key) {
        Some(Value::String(url)) => Url::parse(&url).map(Some).map_err(|err| err.to_string()),
        Some(_) => Err(format!("invalid {key}")),
        None => Ok(None),
    }
}

fn take_packages(object: &mut JsonMap) -> Result<IndexMap<Box<str>, RemotePackages>, String> {
    let Some(value) = object.remove("packages") else {
        return Ok(IndexMap::new());
    };
    let packages = expect_object(value)?;
    packages
        .into_iter()
        .map(|(name, value)| {
            parse_remote_package(&name, value).map(|pkg| (name.into_boxed_str(), pkg))
        })
        .collect()
}

fn parse_remote_package(name: &str, value: Value) -> Result<RemotePackages, String> {
    let mut object = expect_object(value)?;
    let versions = match object.remove("versions") {
        Some(value) => parse_versions(name, value)?,
        None => HashMap::new(),
    };
    Ok(RemotePackages { versions })
}

fn parse_versions(name: &str, value: Value) -> Result<HashMap<Version, PackageManifest>, String> {
    let versions = expect_object(value)?;
    let mut parsed = HashMap::new();
    for (version, value) in versions {
        let version =
            Version::from_str(&version).map_err(|_| format!("invalid version {version}"))?;
        match PackageManifest::from_json_value(value) {
            Ok(manifest) => {
                parsed.insert(version, manifest);
            }
            Err(err) => {
                log::warn!(
                    "Error deserializing package manifest for {}@{}: {err}",
                    name,
                    version
                );
            }
        }
    }
    Ok(parsed)
}

#[derive(Debug, Clone)]
pub struct RemotePackages {
    versions: HashMap<Version, PackageManifest>,
}

impl RemotePackages {
    pub fn all_versions(&self) -> impl Iterator<Item = &PackageManifest> {
        self.versions.values()
    }

    pub fn get_latest_may_yanked(&self, selector: VersionSelector) -> Option<&PackageManifest> {
        self.get_latest(selector).or_else(|| {
            self.versions
                .values()
                .filter(|json| selector.satisfies(json))
                .max_by_key(|json| json.version())
        })
    }

    pub fn get_latest(&self, selector: VersionSelector) -> Option<&PackageManifest> {
        if let Some(version) = selector.as_specific() {
            return self.versions.get(version);
        }

        self.versions
            .values()
            .filter(|json| selector.satisfies(json))
            .filter(|json| !json.is_yanked())
            .max_by_key(|json| json.version())
    }

    pub fn get_version(&self, version: &Version) -> Option<&PackageManifest> {
        self.versions.get(version)
    }
}
