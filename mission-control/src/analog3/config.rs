use std::cmp::min;
use std::collections::BTreeMap;
use std::fmt;

use crate::analog3::{A3_PROP_MODULE_TYPE, schema::COMMON_MODULE_DEF};

use super::schema::{ATTRIBUTES, MODULES_SCHEMA, ModuleDef, PropertyDef, ValueType};

#[derive(Debug, Clone)]
pub struct TypeError {}

impl fmt::Display for TypeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "invalid value type")
    }
}

impl std::error::Error for TypeError {}

// Configuration data ///////////////////////////////////////////////////////

#[derive(Debug)]
pub enum Value {
    U8(u8),
    U16(u16),
    U32(u32),
    Text(String),
    Boolean(bool),
    VectorU8(Vec<u8>),
}

impl Value {
    pub fn as_u8(&self) -> std::result::Result<u8, TypeError> {
        let Value::U8(value) = self else {
            return Err(TypeError {});
        };
        return Ok(*value);
    }

    pub fn as_u16(&self) -> std::result::Result<u16, TypeError> {
        let Value::U16(value) = self else {
            return Err(TypeError {});
        };
        return Ok(*value);
    }

    pub fn as_u32(&self) -> std::result::Result<u32, TypeError> {
        let Value::U32(value) = self else {
            return Err(TypeError {});
        };
        return Ok(*value);
    }

    pub fn as_bool(&self) -> std::result::Result<bool, TypeError> {
        let Value::Boolean(value) = self else {
            return Err(TypeError {});
        };
        return Ok(*value);
    }

    pub fn as_text(&self) -> std::result::Result<String, TypeError> {
        let Value::Text(value) = self else {
            return Err(TypeError {});
        };
        return Ok(value.clone());
    }

    pub fn as_vec_u8(&self) -> std::result::Result<Vec<u8>, TypeError> {
        let Value::VectorU8(value) = self else {
            return Err(TypeError {});
        };
        return Ok(value.clone());
    }
}

#[derive(Debug, Clone)]
pub struct Property {
    pub id: u8,
    pub length: u8,
    pub data: Vec<u8>,
}

impl Property {
    pub fn get_attribute(&self) -> Option<&PropertyDef> {
        let attr = &ATTRIBUTES[self.id as usize];
        if !attr.name.is_empty() {
            Some(attr)
        } else {
            None
        }
    }

    pub fn get_value(&self) -> Option<Value> {
        let attr = self.get_attribute()?;
        Some(self.get_value_with_type(&attr.value_type))
    }

    pub fn get_value_with_type(&self, value_type: &ValueType) -> Value {
        let value = match value_type {
            ValueType::U8 => Value::U8(self.data[0]),
            ValueType::U16 => Value::U16(((self.data[0] as u16) << 8) + self.data[1] as u16),
            ValueType::U32 => Value::U32(
                ((self.data[0] as u32) << 24)
                    + ((self.data[1] as u32) << 16)
                    + ((self.data[2] as u32) << 8)
                    + self.data[3] as u32,
            ),
            ValueType::Text => Value::Text(String::from_utf8(self.data.clone()).unwrap()),
            ValueType::Boolean => Value::Boolean(self.data[0] != 0),
            ValueType::VectorU8 => Value::VectorU8(self.data.clone()),
        };
        value
    }

    pub fn get_value_as_string(&self) -> Result<String> {
        let Value::Text(value) = self.get_value_with_type(&ValueType::Text) else {
            return Err(DataParsingError {
                message: "Internal Server Error".to_string(),
            });
        };
        Ok(value)
    }
}

/// Module properties with schema
pub struct Configuration<'a> {
    module_def: &'a ModuleDef,
    properties: Vec<Property>,

    pub module_type: u16,
    pub module_type_name: &'a String,
}

impl<'a> Configuration<'a> {
    pub fn new(properties: Vec<Property>) -> Self {
        Self::with_schema(properties, &MODULES_SCHEMA)
    }

    pub fn with_schema(properties: Vec<Property>, schema: &'a BTreeMap<u16, ModuleDef>) -> Self {
        let mut module_type = 0xffff;
        for property in &properties {
            if property.id == A3_PROP_MODULE_TYPE {
                if let Ok(value) = property.get_value_with_type(&ValueType::U16).as_u16() {
                    module_type = value;
                } else {
                    log::error!("Failed to read module type: Invalid data type");
                };
                break;
            }
        }
        let module_def = match schema.get(&module_type) {
            Some(prop_def) => prop_def,
            None => &COMMON_MODULE_DEF,
        };

        Self {
            module_def,
            properties,

            module_type: module_type,
            module_type_name: &module_def.module_type_name,
        }
    }

    pub fn len(&self) -> usize {
        self.properties.len()
    }

    pub fn prop_name(&self, index: usize) -> String {
        let property = &self.properties[index];
        match self.module_def.properties.get(&property.id) {
            Some(prop_def) => prop_def.name.clone(),
            None => "unknown property".to_string(),
        }
    }

    pub fn prop_value_as_string(&self, index: usize) -> String {
        let property = &self.properties[index];
        match self.module_def.properties.get(&property.id) {
            Some(prop_def) => {
                let value_type = &prop_def.value_type;
                let value = property.get_value_with_type(value_type);

                match &prop_def.enum_names {
                    Some(enum_names) => {
                        let enum_index = value.as_u8().unwrap() as usize;
                        if enum_index < enum_names.len() {
                            enum_names[enum_index].clone()
                        } else {
                            "VALUE_OUT_OF_ENUM_RANGE".to_string()
                        }
                    }
                    None => value_type.to_hex(&value),
                }
            }
            None => "unknown property".to_string(),
        }
    }
}

// Config parser ///////////////////////////////////////////////////////////////

#[derive(Debug)]
pub struct DataParsingError {
    pub message: String,
}

impl fmt::Display for DataParsingError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for DataParsingError {}

type Result<T> = std::result::Result<T, DataParsingError>;

fn error<T>(message: &str) -> Result<T> {
    return Err(DataParsingError {
        message: String::from(message),
    });
}

#[derive(Debug, Clone)]
pub struct DataFieldBuilder {
    // data_field: Option<DataField>,
    id: u8,
    length: u8,
    data: Option<Vec<u8>>,
    id_read: bool,
    length_read: bool,
    data_pos: usize,
}

impl DataFieldBuilder {
    pub fn new() -> Self {
        Self {
            id: 0,
            length: 0,
            data: Some(Vec::new()),
            id_read: false,
            length_read: false,
            data_pos: 0,
        }
    }

    pub fn data(&mut self, data: &[u8], length: usize, offset: usize) -> Result<(bool, usize)> {
        if self.length_read && self.data_pos == self.length as usize {
            return error("DataFieldBuilder: Data overflow");
        }
        let Some(acc_data) = &mut self.data.as_mut() else {
            return error("DataFieldBuilder: Built already. The build cannot be used twice.");
        };
        let mut index = offset;
        if !self.id_read {
            self.id = data[index];
            index += 1;
            self.id_read = true;
        }
        if index >= length {
            return Ok((false, index));
        }
        if !self.length_read {
            self.length = data[index];
            index += 1;
            self.length_read = true;
        }
        let bytes_left = length - index;
        if bytes_left <= 0 {
            return Ok((false, index));
        }
        let to_read = min(self.length as usize - self.data_pos, bytes_left);
        acc_data.extend_from_slice(&data[index..index + to_read]);
        self.data_pos += to_read;
        let is_ready = self.data_pos == self.length as usize;
        return Ok((is_ready, index + to_read));
    }

    pub fn build(&mut self) -> Result<Property> {
        if self.data_pos < self.length as usize {
            return error("DataFieldBuilder: The builder is not ready to build yet");
        }
        if let Some(data) = self.data.take() {
            return Ok(Property {
                id: self.id,
                length: self.length,
                data,
            });
        }
        return error("Data field is not set");
    }
}

#[derive(Debug, Clone)]
pub struct ChunkBuilder {
    chunk: Option<Vec<Property>>,
    target_num_fields: usize,
    num_fields_read: bool,
    field_builder: DataFieldBuilder,
}

impl<'a> ChunkBuilder {
    pub fn new() -> Self {
        return Self {
            chunk: Some(Vec::new()),
            target_num_fields: 0,
            num_fields_read: false,
            field_builder: DataFieldBuilder::new(),
        };
    }

    pub fn for_single_field() -> Self {
        let mut builder = Self::new();
        let header: [u8; 1] = [1; 1];
        builder.data(&header, 1).unwrap();
        return builder;
    }

    pub fn data(&mut self, data: &[u8], data_length: usize) -> Result<bool> {
        let Some(chunk) = self.chunk.as_mut() else {
            return error("ChunkBuilder: Built already. The builder cannot be used twice.");
        };
        if self.num_fields_read && chunk.len() == self.target_num_fields {
            return error("ChunkBuilder: Data overflow.");
        }

        let mut index = 0;
        if !self.num_fields_read {
            self.target_num_fields = data[index] as usize;
            self.num_fields_read = true;
            index += 1;
        }
        while index < data_length && chunk.len() < self.target_num_fields {
            let (field_done, next_index) = self.field_builder.data(data, data_length, index)?;
            index = next_index;
            if field_done {
                chunk.push(self.field_builder.build().unwrap());
                self.field_builder = DataFieldBuilder::new();
            }
        }
        return Ok(chunk.len() == self.target_num_fields);
    }

    pub fn build(&mut self) -> Result<Vec<Property>> {
        let Some(chunk) = self.chunk.as_ref() else {
            return error("ChunkBuilder: build() method cannot be called twice.");
        };
        if !self.num_fields_read || chunk.len() < self.target_num_fields {
            return error("ChunkBuilder: The builder is not ready for generating the chunk.");
        }
        return Ok(self.chunk.take().unwrap());
    }
}

#[cfg(test)]
mod tests {
    use super::super::schema::load_schema;
    use super::*;

    #[test]
    fn test_parse_single_field_segment() {
        let data = b"\x02\x05hello";
        let mut builder = DataFieldBuilder::new();
        let result = builder.data(data, 7, 0).unwrap();
        assert_eq!(result, (true, 7));
        let data_field = builder.build().unwrap();
        assert_eq!(data_field.id, 2);
        assert_eq!(data_field.data.as_slice(), b"hello");
        assert_eq!(data_field.get_attribute().unwrap().name, "name".to_string());
        let Value::Text(value) = data_field.get_value().unwrap() else {
            panic!("unexpected value type");
        };
        assert_eq!(value, "hello".to_string());
    }

    #[test]
    fn test_parse_two_field_segments() {
        let data1 = b"\x02\x02hi\x03\x05h";
        let data2 = b"ello\0\0\0";
        let mut builder1 = DataFieldBuilder::new();
        let result = builder1.data(data1, 7, 0).unwrap();
        assert_eq!(result, (true, 4));
        let mut builder2 = DataFieldBuilder::new();
        let result2 = builder2.data(data1, 7, 4).unwrap();
        assert_eq!(result2, (false, 7));
        let result3 = builder2.data(data2, 4, 0).unwrap();
        assert_eq!(result3, (true, 4));
        let data_field1 = builder1.build().unwrap();
        assert_eq!(data_field1.data.as_slice(), b"hi");

        let Value::Text(value1) = data_field1.get_value().unwrap() else {
            panic!("unexpected value type");
        };
        assert_eq!(value1, "hi".to_string());

        let data_field2 = builder2.build().unwrap();
        assert_eq!(data_field2.data.as_slice(), b"hello");
        assert!(data_field2.get_value().is_none());
        let Value::Text(value2) = data_field2.get_value_with_type(&ValueType::Text) else {
            panic!("unexpected value type");
        };
        assert_eq!(value2, "hello".to_string());
    }

    #[test]
    fn test_parse_chunk() {
        let data1 = b"\x02\x02\x02hi\x03\x05";
        let data2 = b"hello\0\0";
        let mut builder = ChunkBuilder::new();
        assert!(!builder.data(data1, 7).unwrap());
        assert!(builder.data(data2, 5).unwrap());
        let chunk = builder.build().unwrap();
        assert_eq!(chunk[0].data.as_slice(), b"hi");
        assert_eq!(chunk[1].data.as_slice(), b"hello");

        let Value::Text(value0) = chunk[0].get_value().unwrap() else {
            panic!("unexpected value type");
        };
        assert_eq!(value0, "hi".to_string());

        assert!(chunk[1].get_value().is_none());
        let Value::Text(value1) = chunk[1].get_value_with_type(&ValueType::Text) else {
            panic!("unexpected value type");
        };
        assert_eq!(value1, "hello".to_string());
    }

    #[test]
    fn test_parse_single_field_using_chunk_builder() {
        let data = b"\x02\x05hello";
        let mut builder = ChunkBuilder::for_single_field();
        let result = builder.data(data, 7).unwrap();
        assert!(result);
        let data_fields = builder.build().unwrap();
        assert_eq!(data_fields.len(), 1);
        let data_field = &data_fields[0];
        assert_eq!(data_field.id, 2);
        assert_eq!(data_field.data.as_slice(), b"hello");
        assert_eq!(data_field.get_attribute().unwrap().name, "name".to_string());
        let Value::Text(value) = data_field.get_value().unwrap() else {
            panic!("unexpected value type");
        };
        assert_eq!(value, "hello".to_string());
    }

    #[test]
    fn test_load_config() {
        let data = [
            b"\x05\x00\x04\x1a\xce\xbe\xef",
            b"\x01\x02\x23\x45\x02\x06m",
            b"odule\x03\x01",
            b"\x02\x04\x01\x01\0\0\0",
        ];

        let schema = load_schema("test-schema");

        let mut builder = ChunkBuilder::new();
        let mut index = 0;
        loop {
            match builder.data(data[index], 7) {
                Ok(is_done) => {
                    if is_done {
                        let properties = builder.build().unwrap();
                        let config = Configuration::with_schema(properties, &schema);
                        assert_eq!(config.module_type, 0x2345);
                        assert_eq!(config.module_type_name.as_str(), "test-module");
                        assert_eq!(config.len(), 5);
                        assert_eq!(config.prop_name(0), "module_uid");
                        assert_eq!(config.prop_value_as_string(0), "1acebeef");
                        assert_eq!(config.prop_name(1), "module_type");
                        assert_eq!(config.prop_value_as_string(1), "2345");
                        assert_eq!(config.prop_name(2), "name");
                        assert_eq!(config.prop_value_as_string(2), "module");
                        assert_eq!(config.prop_name(3), "num_voices");
                        assert_eq!(config.prop_value_as_string(3), "02");
                        assert_eq!(config.prop_name(4), "key_assign_mode");
                        assert_eq!(config.prop_value_as_string(4), "UNISON");
                        break;
                    }
                }
                Err(e) => {
                    panic!("Data parsing failed: {:?}", e);
                }
            }
            index += 1;
        }
    }
}
