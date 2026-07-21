use crate::io;
use crate::io::{DefaultEnvironmentIo, IoTrait};
use crate::utils::{deserialize_value, parse_json_file_as_value, read_to_end};
use serde_json::Value;

/// since this file is vrc-get specific, additional keys can be removed
#[derive(Debug, Default, Clone)]
struct AsJson {
    ignore_official_repository: bool,
    ignore_curated_repository: bool,
}

impl AsJson {
    fn from_json_value(value: Value) -> Result<Self, String> {
        let Value::Object(mut object) = value else {
            return Err("expected object".into());
        };
        Ok(Self {
            ignore_official_repository: object
                .remove("ignoreOfficialRepository")
                .map(deserialize_value::<bool>)
                .transpose()?
                .unwrap_or(false),
            ignore_curated_repository: object
                .remove("ignoreCuratedRepository")
                .map(deserialize_value::<bool>)
                .transpose()?
                .unwrap_or(false),
        })
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
                    AsJson::from_json_value(value).map_err(|err| {
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
