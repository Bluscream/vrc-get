use crate::PackageManifest;
use crate::repository::{RemotePackages, RemoteRepository};
use crate::utils::{deserialize_value, expect_object, take_default_with};
use indexmap::IndexMap;
use serde_json::{Map, Value};
use url::Url;

impl LocalCachedRepository {
    pub(crate) fn from_json_value(value: Value) -> Result<Self, String> {
        let mut object = expect_object(value)?;
        let repo = RemoteRepository::parse(expect_object(
            object
                .remove("repo")
                .ok_or_else(|| "missing repo".to_owned())?,
        )?)
        .map_err(|err| err.to_string())?;
        let headers = take_default_with(&mut object, "headers", |value| {
            expect_object(value)?
                .into_iter()
                .map(|(key, value)| {
                    deserialize_value::<Box<str>>(value).map(|value| (key.into_boxed_str(), value))
                })
                .collect::<Result<IndexMap<_, _>, _>>()
        })?;
        let vrc_get = match object.remove("vrc-get") {
            Some(value) => Some(VrcGetMeta::from_json_value(value)?),
            None => None,
        };
        Ok(Self {
            repo,
            headers,
            vrc_get,
        })
    }

    pub(crate) fn to_json_value(&self) -> Value {
        let mut object = Map::new();
        object.insert("repo".to_owned(), self.repo.to_json_value());
        if !self.headers.is_empty() {
            object.insert(
                "headers".to_owned(),
                Value::Object(
                    self.headers
                        .iter()
                        .map(|(k, v)| (k.to_string(), Value::String(v.to_string())))
                        .collect(),
                ),
            );
        }
        if let Some(vrc_get) = &self.vrc_get
            && let Value::Object(value) = vrc_get.to_json_value()
            && !value.is_empty()
        {
            object.insert("vrc-get".to_owned(), Value::Object(value));
        }
        Value::Object(object)
    }

    pub fn new(repo: RemoteRepository, headers: IndexMap<Box<str>, Box<str>>) -> Self {
        Self {
            repo,
            headers,
            vrc_get: None,
        }
    }

    pub fn headers(&self) -> &IndexMap<Box<str>, Box<str>> {
        &self.headers
    }

    pub fn repo(&self) -> &RemoteRepository {
        &self.repo
    }

    pub(crate) fn set_repo(&mut self, mut repo: RemoteRepository) {
        if let Some(id) = self.id() {
            repo.set_id_if_none(|| id.into());
        }
        if let Some(url) = self.url() {
            repo.set_url_if_none(|| url.to_owned());
        }
        self.repo = repo;
    }

    pub(crate) fn set_etag(&mut self, etag: Option<Box<str>>) {
        if let Some(etag) = etag {
            self.vrc_get.get_or_insert_with(Default::default).etag = etag;
        } else if let Some(x) = self.vrc_get.as_mut() {
            x.etag = "".into();
        }
    }

    pub fn url(&self) -> Option<&Url> {
        self.repo().url()
    }

    pub fn set_url(&mut self, url: Url) {
        self.repo.set_url(url);
    }

    pub fn id(&self) -> Option<&str> {
        self.repo().id()
    }

    pub fn name(&self) -> Option<&str> {
        self.repo().name()
    }

    pub fn get_versions_of(
        &self,
        package: &str,
    ) -> impl Iterator<Item = &'_ PackageManifest> + use<'_> {
        self.repo().get_versions_of(package)
    }

    pub fn get_packages(&self) -> impl Iterator<Item = &'_ RemotePackages> {
        self.repo().get_packages()
    }
}

#[derive(Debug, Clone, Default)]
pub struct VrcGetMeta {
    pub etag: Box<str>,
}

impl VrcGetMeta {
    fn from_json_value(value: Value) -> Result<Self, String> {
        let mut object = expect_object(value)?;
        Ok(Self {
            etag: match object.remove("etag") {
                Some(value) => deserialize_value::<Box<str>>(value)?,
                None => "".into(),
            },
        })
    }

    fn to_json_value(&self) -> Value {
        let mut object = Map::new();
        if !self.etag.is_empty() {
            object.insert("etag".to_owned(), Value::String(self.etag.to_string()));
        }
        Value::Object(object)
    }
}
