use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use lazy_static::lazy_static;
use serde;
use serde::Deserialize;
use walkdir::WalkDir;

use super::config::Value;

#[cfg(not(test))]
use log::error;

/// Analog3 module configuration value types
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValueType {
    U8,
    U16,
    U32,
    Text,
    Boolean,
    VectorU8,
    VectorU16,
}

impl ValueType {
    pub fn to_string(&self, value: &Value) -> String {
        return match self {
            ValueType::U8 => value.as_u8().unwrap().to_string(),
            ValueType::U16 => value.as_u16().unwrap().to_string(),
            ValueType::U32 => value.as_u32().unwrap().to_string(),
            ValueType::Text => value.as_text().unwrap(),
            ValueType::Boolean => value.as_bool().unwrap().to_string(),
            ValueType::VectorU8 => value
                .as_vec_u8()
                .unwrap()
                .iter()
                .map(|val| val.to_string())
                .collect::<Vec<_>>()
                .join(", "),
            ValueType::VectorU16 => value
                .as_vec_u16()
                .unwrap()
                .iter()
                .map(|val| val.to_string())
                .collect::<Vec<_>>()
                .join(", "),
        };
    }

    pub fn to_hex(&self, value: &Value) -> String {
        return match self {
            ValueType::U8 => format!("{:02x}", value.as_u8().unwrap()),
            ValueType::U16 => format!("{:04x}", value.as_u16().unwrap()),
            ValueType::U32 => format!("{:08x}", value.as_u32().unwrap()),
            ValueType::Text => value.as_text().unwrap(),
            ValueType::Boolean => value.as_bool().unwrap().to_string(),
            ValueType::VectorU8 => value
                .as_vec_u8()
                .unwrap()
                .iter()
                .map(|val| format!("{:02x}", val))
                .collect::<Vec<_>>()
                .join(", "),
            ValueType::VectorU16 => value
                .as_vec_u16()
                .unwrap()
                .iter()
                .map(|val| format!("{:04x}", val))
                .collect::<Vec<_>>()
                .join(", "),
        };
    }
}

/// Property schema
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PropertyDef {
    pub id: u8,
    pub name: String,
    pub value_type: ValueType,
    #[serde(rename = "enum")]
    pub enum_names: Option<Vec<String>>,
    pub read_only: Option<bool>,
}

/// Module description that is used tentatively during schema loading.
/// Module schema yaml files use this schema
#[derive(Debug, Clone, Deserialize)]
struct ModuleDesc {
    pub module_type: u16,
    pub module_type_name: String,
    pub properties: Vec<Option<PropertyDef>>,
}

/// Module definition used internally to handle config data
#[derive(Debug, Clone)]
pub struct ModuleDef {
    pub module_type: u16,
    pub module_type_name: String,
    pub properties: BTreeMap<u8, PropertyDef>,
}

impl ModuleDef {
    fn from_desc(module_desc: ModuleDesc) -> Self {
        let mut def = Self {
            module_type: module_desc.module_type,
            module_type_name: module_desc.module_type_name,
            properties: BTreeMap::new(),
        };

        for mut property_or_none in module_desc.properties {
            if let Some(property) = property_or_none.take() {
                def.properties.insert(property.id, property);
            }
        }

        for (id, prop_def) in &COMMON_MODULE_DEF.properties {
            def.properties.insert(*id, prop_def.clone());
        }

        def
    }

    pub fn get_property_def_by_name(&self, name: &String) -> Option<&PropertyDef> {
        match self.properties.iter().find(|entry| entry.1.name == *name) {
            Some(entry) => Some(entry.1),
            None => None,
        }
    }

    pub fn get_property_by_id(&self, id: u8) -> Option<&PropertyDef> {
        self.properties.get(&id)
    }
}

lazy_static! {
    pub static ref COMMON_MODULE_DEF: ModuleDef = {
        let mut def = ModuleDef {
            module_type: 0xffff,
            module_type_name: "unknown".to_string(),
            properties: BTreeMap::new(),
        };

        def.properties.insert(
            0,
            PropertyDef {
                id: 0,
                name: String::from("module_uid"),
                value_type: ValueType::U32,
                enum_names: None,
                read_only: Some(true),
            },
        );
        def.properties.insert(
            1,
            PropertyDef {
                id: 1,
                name: String::from("module_type"),
                value_type: ValueType::U16,
                enum_names: None,
                read_only: Some(true),
            },
        );
        def.properties.insert(
            2,
            PropertyDef {
                id: 2,
                name: String::from("name"),
                value_type: ValueType::Text,
                enum_names: None,
                read_only: None,
            },
        );

        def
    };
    pub static ref MODULES_SCHEMA: BTreeMap<u16, ModuleDef> = load_schema("schema");
}

/// schema loader
pub fn load_schema<P: AsRef<Path>>(directory: P) -> BTreeMap<u16, ModuleDef> {
    let mut schema = BTreeMap::new();

    for entry in WalkDir::new(directory) {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        if entry.file_type().is_file() {
            let path = entry.path();
            if let Some(ext) = path.extension() {
                if ext == "yaml" || ext == "yml" {
                    match fs::read_to_string(path) {
                        Ok(content) => match serde_yaml::from_str::<ModuleDesc>(&content) {
                            Ok(desc) => {
                                schema.insert(desc.module_type, ModuleDef::from_desc(desc));
                            }
                            Err(e) => error!("YAML parse error in {:?}: {}", path, e),
                        },
                        Err(e) => error!("File read error in {:?}: {}", path, e),
                    }
                }
            }
        }
    }

    schema.insert(COMMON_MODULE_DEF.module_type, COMMON_MODULE_DEF.clone());
    return schema;
}

#[cfg(test)]
use std::eprintln as error;

#[cfg(test)]

mod tests {
    use super::*;

    #[test]
    fn test_schema_loading() {
        let schema = load_schema("schema");
        assert!(schema.len() > 0);
        let Some(entry) = schema.get(&1) else {
            panic!("the entry must be found");
        };
        assert_eq!(entry.module_type, 1);
        assert_eq!(entry.module_type_name, "cv-depot".to_string());
        assert_eq!(entry.properties.len(), 13);

        let Some(uid) = entry.properties.get(&0) else {
            panic!("UID entry not found");
        };
        assert_eq!(uid.id, 0);
        assert_eq!(uid.name, "module_uid");
        assert_eq!(uid.value_type, ValueType::U32);

        let Some(module_type) = entry.properties.get(&1) else {
            panic!("module_type entry not found");
        };
        assert_eq!(module_type.id, 1);
        assert_eq!(module_type.name, "module_type");
        assert_eq!(module_type.value_type, ValueType::U16);

        let Some(module_type) = entry.properties.get(&1) else {
            panic!("module_type entry not found");
        };
        assert_eq!(module_type.id, 1);
        assert_eq!(module_type.name, "module_type");
        assert_eq!(module_type.value_type, ValueType::U16);

        let Some(module_name) = entry.properties.get(&2) else {
            panic!("name entry not found");
        };
        assert_eq!(module_name.id, 2);
        assert_eq!(module_name.name, "name");
        assert_eq!(module_name.value_type, ValueType::Text);

        let Some(num_voices) = entry.properties.get(&3) else {
            panic!("num_voices entry not found");
        };
        assert_eq!(num_voices.id, 3);
        assert_eq!(num_voices.name, "num_voices");
        assert_eq!(num_voices.value_type, ValueType::U8);
        assert!(num_voices.enum_names.is_none());

        let Some(key_assign_mode) = entry.properties.get(&4) else {
            panic!("key_assign_mode entry not found");
        };
        assert_eq!(key_assign_mode.id, 4);
        assert_eq!(key_assign_mode.name, "key_assign_mode");
        assert_eq!(key_assign_mode.value_type, ValueType::U8);
        let Some(ref enum_names) = key_assign_mode.enum_names else {
            panic!("key_assign_mode must have enums");
        };
        assert_eq!(enum_names.len(), 3);
        assert_eq!(enum_names[0], "DUOPHONIC".to_string());
    }
}
