use std::collections::BTreeMap;
use std::fmt;
use std::{cmp::min, num::ParseIntError};

use crate::error::{AppError, ErrorType};

use super::{
    PropertyId,
    schema::{COMMON_MODULE_DEF, MODULES_SCHEMA, ModuleDef, ValueType},
};

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
    VectorU16(Vec<u16>),
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

    pub fn as_vec_u16(&self) -> std::result::Result<Vec<u16>, TypeError> {
        let Value::VectorU16(value) = self else {
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
    pub fn u8(id: u8, value: u8) -> Self {
        Self {
            id,
            length: 1,
            data: vec![value],
        }
    }

    pub fn u16(id: u8, value: u16) -> Self {
        Self {
            id,
            length: 2,
            data: vec![(value >> 8) as u8, (value & 0xff) as u8],
        }
    }

    pub fn u32(id: u8, value: u32) -> Self {
        Self {
            id,
            length: 4,
            data: vec![
                ((value >> 24) & 0xff) as u8,
                ((value >> 16) & 0xff) as u8,
                ((value >> 8) & 0xff) as u8,
                (value & 0xff) as u8,
            ],
        }
    }

    pub fn text(id: u8, value: &String) -> Self {
        Self {
            id,
            length: value.len() as u8,
            data: value.as_bytes().to_vec(),
        }
    }

    pub fn vector_u8(id: u8, value: &Vec<u8>) -> Self {
        Self {
            id,
            length: value.len() as u8,
            data: value.clone(),
        }
    }

    pub fn vector_u16(id: u8, value: &Vec<u16>) -> Self {
        let data = value.into_iter().flat_map(|v| v.to_be_bytes()).collect();
        Self {
            id,
            length: (value.len() * 2) as u8,
            data,
        }
    }

    pub fn boolean(id: u8, value: bool) -> Self {
        Self {
            id,
            length: 1,
            data: vec![if value { 1 } else { 0 }],
        }
    }

    pub fn from_string(
        id: u8,
        src: &String,
        value_type: &ValueType,
    ) -> std::result::Result<Self, AppError> {
        match value_type {
            ValueType::U8 => {
                let Ok(value) = parse_u8(src) else {
                    return Self::make_error();
                };
                Ok(Self::u8(id, value))
            }
            ValueType::U16 => {
                let Ok(value) = parse_u16(src) else {
                    return Self::make_error();
                };
                Ok(Self::u16(id, value))
            }
            ValueType::U32 => {
                let Ok(value) = parse_u32(src) else {
                    return Self::make_error();
                };
                Ok(Self::u32(id, value))
            }
            ValueType::Text => Ok(Self::text(id, src)),
            ValueType::Boolean => {
                let Ok(value) = src.parse() else {
                    return Self::make_error();
                };
                Ok(Self::boolean(id, value))
            }
            ValueType::VectorU8 => {
                let src_array = Self::split(src);
                let mut out: Vec<u8> = Vec::new();
                for element in src_array {
                    let Ok(value) = parse_u8(&element) else {
                        return Self::make_error();
                    };
                    out.push(value);
                }
                Ok(Self::vector_u8(id, &out))
            }
            ValueType::VectorU16 => {
                let src_array = Self::split(src);
                let mut out: Vec<u16> = Vec::new();
                for element in src_array {
                    let Ok(value) = parse_u16(&element) else {
                        return Self::make_error();
                    };
                    out.push(value);
                }
                Ok(Self::vector_u16(id, &out))
            }
        }
    }

    fn make_error() -> std::result::Result<Self, AppError> {
        Err(AppError::new(
            ErrorType::A3InvalidValue,
            "Invalid value string".to_string(),
        ))
    }

    fn split(src: &String) -> Vec<String> {
        src.split(",").map(|s| s.trim().to_string()).collect()
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
            ValueType::Text => {
                let value = match String::from_utf8(self.data.clone()) {
                    Ok(v) => v,
                    Err(e) => {
                        log::warn!("Utf parsing error: {:?}", e);
                        let mut v = String::new();
                        for byte in &self.data {
                            v.push_str(format!("\\x{:02x}", byte).as_str());
                        }
                        v
                    }
                };
                Value::Text(value)
            }
            ValueType::Boolean => Value::Boolean(self.data[0] != 0),
            ValueType::VectorU8 => Value::VectorU8(self.data.clone()),
            ValueType::VectorU16 => {
                let length = self.data.len() & !1;
                let value = self.data[..length]
                    .chunks(2)
                    .map(|b| u16::from_be_bytes([b[0], b[1]]))
                    .collect();
                Value::VectorU16(value)
            }
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
            if property.id == PropertyId::ModuleType as u8 {
                if let Ok(value) = property.get_value_with_type(&ValueType::U16).as_u16() {
                    module_type = value;
                } else {
                    log::error!("Failed to read module type: Invalid data type");
                };
                break;
            }
        }
        let module_def = match schema.get(&module_type) {
            Some(def) => def,
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
            Some(prop_def) => format!("({:3}) {}", property.id, prop_def.name),
            None => format!("({:3}) unknown", property.id),
        }
    }

    pub fn prop_value_as_string(&self, index: usize) -> String {
        let property = &self.properties[index];
        match self.module_def.properties.get(&property.id) {
            Some(prop_def) => {
                let value_type = &prop_def.value_type;
                let value = property.get_value_with_type(value_type);

                match &prop_def.enum_names {
                    Some(enum_names) => match value_type {
                        ValueType::U8 => {
                            let enum_index = value.as_u8().unwrap() as usize;
                            if enum_index < enum_names.len() {
                                format!("{} ({})", enum_names[enum_index], enum_index)
                            } else {
                                "VALUE_OUT_OF_ENUM_RANGE".to_string()
                            }
                        }
                        ValueType::VectorU8 => value
                            .as_vec_u8()
                            .unwrap()
                            .iter()
                            .map(|value| {
                                let index = value.clone() as usize;
                                if index < enum_names.len() {
                                    format!("{} ({})", enum_names[index].clone(), index)
                                } else {
                                    "VALUE_OUT_OF_ENUM_RANGE".to_string()
                                }
                            })
                            .collect::<Vec<_>>()
                            .join(", "),
                        _ => "INVALID_ENUM_TYPE".to_string(),
                    },
                    None => value_type.to_hex(&value),
                }
            }
            None => hex::encode(&property.data),
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
pub struct DataFieldParser {
    id: u8,
    length: u8,
    data: Option<Vec<u8>>,
    id_read: bool,
    length_read: bool,
    data_pos: usize,
}

impl DataFieldParser {
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
            return error("DataFieldParser: Data overflow");
        }
        let Some(acc_data) = &mut self.data.as_mut() else {
            return error("DataFieldParser: Committed already. The build cannot be used twice.");
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

    pub fn commit(&mut self) -> Result<Property> {
        if self.data_pos < self.length as usize {
            return error("DataFieldParser: The parser is not ready to build yet");
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
pub struct ChunkParser {
    chunk: Option<Vec<Property>>,
    target_num_fields: usize,
    num_fields_read: bool,
    field_parser: DataFieldParser,
}

impl ChunkParser {
    pub fn new() -> Self {
        return Self {
            chunk: Some(Vec::new()),
            target_num_fields: 0,
            num_fields_read: false,
            field_parser: DataFieldParser::new(),
        };
    }

    pub fn for_single_field() -> Self {
        let mut parser = Self::new();
        let header: [u8; 1] = [1; 1];
        parser.data(&header, 1).unwrap();
        return parser;
    }

    pub fn data(&mut self, data: &[u8], data_length: usize) -> Result<bool> {
        let Some(chunk) = self.chunk.as_mut() else {
            return error("ChunkParser: Committed already. The parser cannot be used twice.");
        };
        if self.num_fields_read && chunk.len() == self.target_num_fields {
            return error("ChunkParser: Data overflow.");
        }

        let mut index = 0;
        if !self.num_fields_read {
            self.target_num_fields = data[index] as usize;
            self.num_fields_read = true;
            index += 1;
        }
        while index < data_length && chunk.len() < self.target_num_fields {
            let (field_done, next_index) = self.field_parser.data(data, data_length, index)?;
            index = next_index;
            if field_done {
                chunk.push(self.field_parser.commit().unwrap());
                self.field_parser = DataFieldParser::new();
            }
        }
        return Ok(chunk.len() == self.target_num_fields);
    }

    pub fn commit(&mut self) -> Result<Vec<Property>> {
        let Some(chunk) = self.chunk.as_ref() else {
            return error("ChunkParser: build() method cannot be called twice.");
        };
        if !self.num_fields_read || chunk.len() < self.target_num_fields {
            return error("ChunkParser: The parser is not ready for generating the chunk.");
        }
        return Ok(self.chunk.take().unwrap());
    }
}

// Property encoder ///////////////////////////////////////////////////////////////

pub struct PropertyEncoder<'a> {
    props: &'a Vec<Property>,
    num_props_sent: bool,
    prop_id_sent: bool,
    value_length_sent: bool,
    pos_value: usize,
    pos_prop: usize,
}

impl<'a> PropertyEncoder<'a> {
    pub fn new(props: &'a Vec<Property>) -> Self {
        Self {
            props,
            num_props_sent: false,
            prop_id_sent: false,
            value_length_sent: false,
            pos_value: 0,
            pos_prop: 0,
        }
    }

    /// Flushes next piece of property data into the specified byte array.
    ///
    /// # Arguments
    ///
    /// - `data` (`&mut [u8]`) - The data array where the data is flushed into.
    ///
    /// # Returns
    ///
    /// - `usize` - Number of bytes that were flushed.
    pub fn flush(&mut self, out_data: &mut [u8]) -> usize {
        let out_data_len = out_data.len();
        if out_data_len < 1 || self.is_done() {
            // nothing can be done
            return 0;
        }

        let mut data_index = 0;
        if !self.num_props_sent {
            out_data[data_index] = self.props.len() as u8;
            data_index += 1;
            self.num_props_sent = true;
        }

        while data_index < out_data_len {
            let prop = &self.props[self.pos_prop];

            if !self.prop_id_sent {
                out_data[data_index] = prop.id;
                data_index += 1;
                self.prop_id_sent = true;
                if data_index >= out_data_len {
                    return data_index;
                }
            }

            if !self.value_length_sent {
                out_data[data_index] = prop.data.len() as u8;
                data_index += 1;
                self.value_length_sent = true;
                if data_index >= out_data_len {
                    return data_index;
                }
            }

            let bytes_to_send = min(out_data_len - data_index, prop.data.len() - self.pos_value);
            let dest = &mut out_data[data_index..data_index + bytes_to_send];
            let src = &prop.data[self.pos_value..self.pos_value + bytes_to_send];
            dest.copy_from_slice(src);
            data_index += bytes_to_send;
            if self.update_positions(bytes_to_send) {
                break;
            }
        }

        return data_index;
    }

    /// Proceed the value and the prop positions by delta.
    ///
    /// # Arguments
    ///
    /// - `delta` (`usize`) - Steps to proceed.
    ///
    /// # Returns
    ///
    /// - `bool` - True if the position reaches the end.
    fn update_positions(&mut self, delta: usize) -> bool {
        let prop = &self.props[self.pos_prop];
        self.pos_value += delta;
        if self.pos_value >= prop.data.len() {
            self.pos_value = 0;
            self.prop_id_sent = false;
            self.value_length_sent = false;
            self.pos_prop += 1;
        }
        return self.is_done();
    }

    pub fn is_done(&self) -> bool {
        self.pos_prop == self.props.len()
    }
}

#[cfg(test)]
mod tests {
    use super::super::PropertyId;
    use super::super::schema::load_schema;
    use super::*;

    #[test]
    fn test_make_property_u8() {
        let property = Property::u8(1, 234);
        assert_eq!(property.id, 1);
        let Ok(value) = property.get_value_with_type(&ValueType::U8).as_u8() else {
            panic!();
        };
        assert_eq!(value, 234u8);
    }

    #[test]
    fn test_make_property_u16() {
        let property = Property::u16(2, 0xba11);
        assert_eq!(property.id, 2);
        let Ok(value) = property.get_value_with_type(&ValueType::U16).as_u16() else {
            panic!();
        };
        assert_eq!(value, 0xba11u16);
    }

    #[test]
    fn test_make_property_u32() {
        let property = Property::u32(3, 0xba5eba11);
        assert_eq!(property.id, 3);
        let Ok(value) = property.get_value_with_type(&ValueType::U32).as_u32() else {
            panic!();
        };
        assert_eq!(value, 0xba5eba11u32);
    }

    #[test]
    fn test_make_property_text() {
        let property = Property::text(4, &"hello world".to_string());
        assert_eq!(property.id, 4);
        let Ok(value) = property.get_value_with_type(&ValueType::Text).as_text() else {
            panic!();
        };
        assert_eq!(value, "hello world");
    }

    #[test]
    fn test_make_property_vector_u8() {
        let property = Property::vector_u8(5, &vec![0xca, 0xfe]);
        assert_eq!(property.id, 5);
        let Ok(value) = property
            .get_value_with_type(&ValueType::VectorU8)
            .as_vec_u8()
        else {
            panic!();
        };
        assert_eq!(value, vec![0xca, 0xfe]);
    }

    #[test]
    fn test_make_property_vector_u16() {
        let property = Property::vector_u16(7, &vec![0xba5e, 0xba11]);
        assert_eq!(property.id, 7);
        let Ok(value) = property
            .get_value_with_type(&ValueType::VectorU16)
            .as_vec_u16()
        else {
            panic!();
        };
        assert_eq!(value, vec![0xba5e, 0xba11]);
    }

    #[test]
    fn test_make_property_boolean() {
        let property = Property::boolean(6, true);
        assert_eq!(property.id, 6);
        let Ok(value) = property.get_value_with_type(&ValueType::Boolean).as_bool() else {
            panic!();
        };
        assert!(value);
    }

    #[test]
    fn test_parse_single_field_segment() {
        let data = b"\x02\x05hello";
        let mut parser = DataFieldParser::new();
        let result = parser.data(data, 7, 0).unwrap();
        assert_eq!(result, (true, 7));
        let data_field = parser.commit().unwrap();
        assert_eq!(data_field.id, 2);
        assert_eq!(data_field.data.as_slice(), b"hello");
        let Value::Text(value) = data_field.get_value_with_type(&ValueType::Text) else {
            panic!("unexpected value type");
        };
        assert_eq!(value, "hello".to_string());
    }

    #[test]
    fn test_parse_two_field_segments() {
        let data1 = b"\x02\x02hi\x03\x05h";
        let data2 = b"ello\0\0\0";
        let mut parser1 = DataFieldParser::new();
        let result = parser1.data(data1, 7, 0).unwrap();
        assert_eq!(result, (true, 4));
        let mut parser2 = DataFieldParser::new();
        let result2 = parser2.data(data1, 7, 4).unwrap();
        assert_eq!(result2, (false, 7));
        let result3 = parser2.data(data2, 4, 0).unwrap();
        assert_eq!(result3, (true, 4));
        let data_field1 = parser1.commit().unwrap();
        assert_eq!(data_field1.data.as_slice(), b"hi");

        let Value::Text(value1) = data_field1.get_value_with_type(&ValueType::Text) else {
            panic!("unexpected value type");
        };
        assert_eq!(value1, "hi".to_string());

        let data_field2 = parser2.commit().unwrap();
        assert_eq!(data_field2.data.as_slice(), b"hello");
        let Value::Text(value2) = data_field2.get_value_with_type(&ValueType::Text) else {
            panic!("unexpected value type");
        };
        assert_eq!(value2, "hello".to_string());
    }

    #[test]
    fn test_parse_chunk() {
        let data1 = b"\x02\x02\x02hi\x03\x05";
        let data2 = b"hello\0\0";
        let mut parser = ChunkParser::new();
        assert!(!parser.data(data1, 7).unwrap());
        assert!(parser.data(data2, 5).unwrap());
        let chunk = parser.commit().unwrap();
        assert_eq!(chunk[0].data.as_slice(), b"hi");
        assert_eq!(chunk[1].data.as_slice(), b"hello");

        let Value::Text(value0) = chunk[0].get_value_with_type(&ValueType::Text) else {
            panic!("unexpected value type");
        };
        assert_eq!(value0, "hi".to_string());

        let Value::Text(value1) = chunk[1].get_value_with_type(&ValueType::Text) else {
            panic!("unexpected value type");
        };
        assert_eq!(value1, "hello".to_string());
    }

    #[test]
    fn test_parse_single_field_using_chunk_parser() {
        let data = b"\x02\x05hello";
        let mut parser = ChunkParser::for_single_field();
        let result = parser.data(data, 7).unwrap();
        assert!(result);
        let data_fields = parser.commit().unwrap();
        assert_eq!(data_fields.len(), 1);
        let data_field = &data_fields[0];
        assert_eq!(data_field.id, 2);
        assert_eq!(data_field.data.as_slice(), b"hello");
        let Value::Text(value) = data_field.get_value_with_type(&ValueType::Text) else {
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

        let mut parser = ChunkParser::new();
        let mut index = 0;
        loop {
            match parser.data(data[index], 7) {
                Ok(is_done) => {
                    if is_done {
                        let properties = parser.commit().unwrap();
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

    #[test]
    fn test_encode_string() {
        let prop = Property::text(
            PropertyId::Name as u8,
            &"Analog3 mission control".to_string(),
        );
        let props = vec![prop];
        let mut encoder = PropertyEncoder::new(&props);

        let mut data: [u8; 8] = [0; 8];

        assert_eq!(encoder.flush(&mut data), 8);
        assert!(!encoder.is_done());
        assert_eq!(&data.as_slice(), b"\x01\x02\x17Analo");

        assert_eq!(encoder.flush(&mut data), 8);
        assert!(!encoder.is_done());
        assert_eq!(&data.as_slice(), b"g3 missi");

        assert_eq!(encoder.flush(&mut data), 8);
        assert!(!encoder.is_done());
        assert_eq!(&data.as_slice(), b"on contr");

        assert_eq!(encoder.flush(&mut data), 2);
        assert!(encoder.is_done());
        assert_eq!(&data.as_slice()[0..2], b"ol");

        assert_eq!(encoder.flush(&mut data), 0);
    }

    #[test]
    fn test_two_props() {
        let prop1 = Property::text(PropertyId::Name as u8, &"Analog3".to_string());
        let prop2 = Property::u32(3, 0xbd093ca7);
        let props = vec![prop1, prop2];
        let mut encoder = PropertyEncoder::new(&props);

        let mut data: [u8; 8] = [0; 8];

        assert_eq!(encoder.flush(&mut data), 8);
        assert!(!encoder.is_done());
        assert_eq!(&data.as_slice(), b"\x02\x02\x07Analo");

        assert_eq!(encoder.flush(&mut data), 8);
        assert!(encoder.is_done());
        assert_eq!(&data.as_slice(), b"g3\x03\x04\xbd\x09\x3c\xa7");

        assert_eq!(encoder.flush(&mut data), 0);
    }
}

// primitive parsers ///////////////////////////////////////////

fn parse_uint<T>(
    src: &str,
    from_str_radix: fn(&str, u32) -> core::result::Result<T, ParseIntError>,
) -> core::result::Result<T, ParseIntError> {
    let s = src.trim();
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        from_str_radix(hex, 16)
    } else {
        from_str_radix(s, 10)
    }
}

pub fn parse_u8(src: &str) -> core::result::Result<u8, ParseIntError> {
    parse_uint(src, u8::from_str_radix)
}

pub fn parse_u16(src: &str) -> core::result::Result<u16, ParseIntError> {
    parse_uint(src, u16::from_str_radix)
}

pub fn parse_u32(src: &str) -> core::result::Result<u32, ParseIntError> {
    parse_uint(src, u32::from_str_radix)
}
