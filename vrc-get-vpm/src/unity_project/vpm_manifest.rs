use crate::io;
use crate::io::DefaultProjectIo;
use crate::unity_project::LockedDependencyInfo;
use crate::utils::{
    SaveController, deserialize_value, expect_object, load_json_or_default, save_json,
};
use crate::version::{DependencyRange, Version, VersionRange};
use indexmap::IndexMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};

const MANIFEST_PATH: &str = "Packages/vpm-manifest.json";

#[derive(Debug, Default)]
struct AsJson {
    dependencies: IndexMap<Box<str>, VpmDependency>,
    locked: IndexMap<Box<str>, VpmLockedDependency>,
}

#[derive(Debug, Clone)]
struct VpmDependency {
    pub version: DependencyRange,
}

#[derive(Debug, Clone)]
struct VpmLockedDependency {
    pub version: Version,
    pub dependencies: Option<IndexMap<Box<str>, VersionRange>>,
}

impl<'de> Deserialize<'de> for AsJson {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let mut object =
            expect_object(Value::deserialize(deserializer)?).map_err(serde::de::Error::custom)?;
        Ok(Self {
            dependencies: take_dependency_map(&mut object, "dependencies")
                .map_err(serde::de::Error::custom)?,
            locked: take_locked_map(&mut object, "locked").map_err(serde::de::Error::custom)?,
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
            "dependencies".to_owned(),
            Value::Object(
                self.dependencies
                    .iter()
                    .map(|(name, dep)| (name.to_string(), dep.to_json_value()))
                    .collect(),
            ),
        );
        object.insert(
            "locked".to_owned(),
            Value::Object(
                self.locked
                    .iter()
                    .map(|(name, dep)| (name.to_string(), dep.to_json_value()))
                    .collect(),
            ),
        );
        Value::Object(object).serialize(serializer)
    }
}

fn take_dependency_map(
    object: &mut Map<String, Value>,
    key: &str,
) -> Result<IndexMap<Box<str>, VpmDependency>, String> {
    let Some(value) = object.remove(key) else {
        return Ok(IndexMap::new());
    };
    expect_object(value)?
        .into_iter()
        .map(|(name, value)| {
            VpmDependency::from_json_value(value).map(|dep| (name.into_boxed_str(), dep))
        })
        .collect()
}

fn take_locked_map(
    object: &mut Map<String, Value>,
    key: &str,
) -> Result<IndexMap<Box<str>, VpmLockedDependency>, String> {
    let Some(value) = object.remove(key) else {
        return Ok(IndexMap::new());
    };
    expect_object(value)?
        .into_iter()
        .map(|(name, value)| {
            VpmLockedDependency::from_json_value(value).map(|dep| (name.into_boxed_str(), dep))
        })
        .collect()
}

impl VpmDependency {
    fn from_json_value(value: Value) -> Result<Self, String> {
        let mut object = expect_object(value)?;
        Ok(Self {
            version: deserialize_value(
                object
                    .remove("version")
                    .ok_or_else(|| "missing version".to_owned())?,
            )?,
        })
    }

    fn to_json_value(&self) -> Value {
        let mut object = Map::new();
        object.insert(
            "version".to_owned(),
            serde_json::to_value(&self.version).unwrap(),
        );
        Value::Object(object)
    }
}

impl VpmLockedDependency {
    fn from_json_value(value: Value) -> Result<Self, String> {
        let mut object = expect_object(value)?;
        Ok(Self {
            version: deserialize_value(
                object
                    .remove("version")
                    .ok_or_else(|| "missing version".to_owned())?,
            )?,
            dependencies: match object.remove("dependencies") {
                Some(value) => Some(
                    expect_object(value)?
                        .into_iter()
                        .map(|(name, value)| {
                            deserialize_value::<VersionRange>(value)
                                .map(|value| (name.into_boxed_str(), value))
                        })
                        .collect::<Result<IndexMap<_, _>, _>>()?,
                ),
                None => None,
            },
        })
    }

    fn to_json_value(&self) -> Value {
        let mut object = Map::new();
        object.insert(
            "version".to_owned(),
            serde_json::to_value(&self.version).unwrap(),
        );
        if let Some(dependencies) = &self.dependencies {
            object.insert(
                "dependencies".to_owned(),
                Value::Object(
                    dependencies
                        .iter()
                        .map(|(name, value)| {
                            (name.to_string(), serde_json::to_value(value).unwrap())
                        })
                        .collect(),
                ),
            );
        }
        Value::Object(object)
    }
}

#[derive(Debug)]
pub(super) struct VpmManifest {
    controller: SaveController<AsJson>,
}

impl VpmManifest {
    pub(super) async fn load(io: &DefaultProjectIo) -> io::Result<Self> {
        Ok(Self {
            controller: SaveController::new(
                load_json_or_default(io, MANIFEST_PATH.as_ref()).await?,
            ),
        })
    }

    pub(super) fn dependencies(&self) -> impl Iterator<Item = (&str, &DependencyRange)> {
        self.controller
            .dependencies
            .iter()
            .map(|(name, dep)| (name.as_ref(), &dep.version))
    }

    pub(super) fn get_dependency(&self, package: &str) -> Option<&DependencyRange> {
        self.controller
            .dependencies
            .get(package)
            .map(|x| &x.version)
    }

    pub(super) fn all_locked(&self) -> impl Iterator<Item = LockedDependencyInfo<'_>> {
        self.controller.locked.iter().map(|(name, dep)| {
            LockedDependencyInfo::new(name.as_ref(), &dep.version, dep.dependencies.as_ref())
        })
    }

    pub(super) fn get_locked(&self, package: &str) -> Option<LockedDependencyInfo<'_>> {
        self.controller
            .locked
            .get_key_value(package)
            .map(|(package, x)| {
                LockedDependencyInfo::new(package, &x.version, x.dependencies.as_ref())
            })
    }

    pub(super) fn add_dependency(&mut self, name: &str, version: DependencyRange) {
        self.controller
            .as_mut()
            .dependencies
            .insert(name.into(), VpmDependency { version });
    }

    pub(super) fn add_locked(
        &mut self,
        name: &str,
        version: Version,
        dependencies: IndexMap<Box<str>, VersionRange>,
    ) {
        self.controller.as_mut().locked.insert(
            name.into(),
            VpmLockedDependency {
                version,
                dependencies: Some(dependencies),
            },
        );
    }

    pub(crate) fn remove_packages<'a>(&mut self, names: impl Iterator<Item = &'a str>) {
        for name in names {
            self.controller.as_mut().locked.shift_remove(name);
            self.controller.as_mut().dependencies.shift_remove(name);
        }
    }

    pub(crate) fn has_any(&self) -> bool {
        !self.controller.locked.is_empty() || !self.controller.dependencies.is_empty()
    }

    pub(super) async fn save(&mut self, io: &DefaultProjectIo) -> io::Result<()> {
        self.controller
            .save(|json| save_json(io, MANIFEST_PATH.as_ref(), json))
            .await
    }

    pub(super) fn to_json(&self) -> io::Result<Vec<u8>> {
        crate::utils::to_vec_pretty_os_eol(&*self.controller)
    }
}
