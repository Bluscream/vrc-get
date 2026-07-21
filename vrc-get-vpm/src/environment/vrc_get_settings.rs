use crate::io;
use crate::io::{DefaultEnvironmentIo, IoTrait};
use crate::utils::{deserialize_value, parse_json_file_as_value, read_to_end};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};

/// since this file is vrc-get specific, additional keys can be removed
#[derive(Debug, Default, Clone)]
struct AsJson {
    ignore_official_repository: bool,
    ignore_curated_repository: bool,
}

impl<'de> Deserialize<'de> for AsJson {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let Value::Object(mut object) = Value::deserialize(deserializer)? else {
            return Err(serde::de::Error::custom("expected object"));
        };
        Ok(Self {
            ignore_official_repository: object
                .remove("ignoreOfficialRepository")
                .map(deserialize_value::<bool>)
                .transpose()
                .map_err(serde::de::Error::custom)?
                .unwrap_or(false),
            ignore_curated_repository: object
                .remove("ignoreCuratedRepository")
                .map(deserialize_value::<bool>)
                .transpose()
                .map_err(serde::de::Error::custom)?
                .unwrap_or(false),
        })
    }
}

impl Serialize for AsJson {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut object = Map::new();
        object.insert(
            "ignoreOfficialRepository".to_owned(),
            Value::Bool(self.ignore_official_repository),
        );
        object.insert(
            "ignoreCuratedRepository".to_owned(),
            Value::Bool(self.ignore_curated_repository),
        );
        Value::Object(object).serialize(serializer)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct VrcGetSettings {
    parsed: AsJson,
}

const JSON_PATH: &str = "vrc-get/settings.json";

impl VrcGetSettings {
    pub async fn load(io: &DefaultEnvironmentIo) -> io::Result<Self> {
        let parsed = match io.open(JSON_PATH.as_ref()).await {
            Ok(file) => match read_to_end(file).await? {
                vec if vec.is_empty() => Default::default(),
                vec => {
                    log::warn!("vrc-get specific settings file is experimental feature!");
                    let value = parse_json_file_as_value(&vec, JSON_PATH.as_ref())?;
                    deserialize_value(value).map_err(|err| {
                        io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!("syntax error loading {}: {err}", JSON_PATH),
                        )
                    })?
                }
            },
            Err(ref e) if e.kind() == io::ErrorKind::NotFound => Default::default(),
            Err(e) => return Err(e),
        };

        Ok(Self { parsed })
    }

    pub fn ignore_official_repository(&self) -> bool {
        self.parsed.ignore_official_repository
    }

    pub fn ignore_curated_repository(&self) -> bool {
        self.parsed.ignore_curated_repository
    }
}
