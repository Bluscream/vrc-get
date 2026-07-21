pub mod setting {
    use crate::environment::RepoSource;
    use crate::utils::{deserialize_value, expect_object, take_default_with, take_optional};
    use indexmap::IndexMap;
    use serde_json::{Map, Value};
    use std::path::{Path, PathBuf};
    use url::Url;

    #[derive(Debug, Clone)]
    pub struct UserRepoSetting {
        local_path: Box<Path>,
        name: Option<Box<str>>,
        // must be non-relative url.
        url: Option<Url>,
        pub(crate) id: Option<Box<str>>,
        pub(crate) headers: IndexMap<Box<str>, Box<str>>,
    }

    fn parse_optional_url(value: Value) -> Result<Option<Url>, String> {
        let Some(url) = deserialize_value::<Option<String>>(value)? else {
            return Ok(None);
        };
        Url::parse(&url).map(Some).map_err(|err| err.to_string())
    }

    impl UserRepoSetting {
        pub(crate) fn from_json_value(value: Value) -> Result<Self, String> {
            let mut object = expect_object(value)?;
            let local_path = deserialize_value::<Box<str>>(
                object
                    .remove("localPath")
                    .ok_or_else(|| "missing localPath".to_owned())?,
            )?;
            Ok(Self {
                local_path: PathBuf::from(local_path.as_ref()).into_boxed_path(),
                name: take_optional(&mut object, "name")?,
                url: match object.remove("url") {
                    Some(value) => parse_optional_url(value)?,
                    None => None,
                },
                id: take_optional(&mut object, "id")?,
                headers: take_default_with(&mut object, "headers", |value| {
                    let object = expect_object(value)?;
                    object
                        .into_iter()
                        .map(|(key, value)| {
                            deserialize_value::<Box<str>>(value)
                                .map(|value| (key.into_boxed_str(), value))
                        })
                        .collect::<Result<IndexMap<_, _>, _>>()
                })?,
            })
        }

        pub(crate) fn to_json_value(&self) -> Value {
            let mut object = Map::new();
            object.insert(
                "localPath".to_owned(),
                Value::String(self.local_path.display().to_string()),
            );
            if let Some(name) = &self.name {
                object.insert("name".to_owned(), Value::String(name.to_string()));
            }
            if let Some(url) = &self.url {
                object.insert("url".to_owned(), Value::String(url.to_string()));
            }
            if let Some(id) = &self.id {
                object.insert("id".to_owned(), Value::String(id.to_string()));
            }
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
            Value::Object(object)
        }

        pub fn new(
            local_path: Box<Path>,
            name: Option<Box<str>>,
            url: Option<Url>,
            id: Option<Box<str>>,
        ) -> Self {
            Self {
                local_path,
                name,
                id: id.or(url.as_ref().map(Url::to_string).map(Into::into)),
                url,
                headers: IndexMap::new(),
            }
        }

        pub fn local_path(&self) -> &Path {
            &self.local_path
        }

        pub fn name(&self) -> Option<&str> {
            self.name.as_deref()
        }

        pub fn url(&self) -> Option<&Url> {
            self.url.as_ref()
        }

        pub fn id(&self) -> Option<&str> {
            self.id.as_deref()
        }

        pub fn headers(&self) -> &IndexMap<Box<str>, Box<str>> {
            &self.headers
        }

        pub(crate) fn to_source(&self) -> RepoSource<'_> {
            RepoSource::new(&self.local_path, &self.headers, self.url.as_ref())
        }
    }
}
