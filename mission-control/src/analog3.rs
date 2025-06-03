use lazy_static::lazy_static;
use std::cmp::min;
use std::fmt;

// ID assignments /////////////////////////////////
pub const A3_ID_MIDI_TIMING_CLOCK: u32 = 0x100;
pub const A3_ID_MIDI_VOICE_BASE: u32 = 0x101;
pub const A3_ID_MIDI_REAL_TIME: u32 = 0x140;

pub const A3_ID_MISSION_CONTROL: u32 = 0x700;
pub const A3_ID_INDIVIDUAL_MODULE_BASE: u32 = 0x700;

// Message types //////////////////////////////////

/* MIDI channel voice messages */
pub const A3_VOICE_MSG_SET_NOTE: u8 = 0x07;
pub const A3_VOICE_MSG_GATE_OFF: u8 = 0x08;
pub const A3_VOICE_MSG_GATE_ON: u8 = 0x09;
pub const A3_VOICE_MSG_POLY_KEY_PRESSURE: u8 = 0x0A;

/* MIDI channel messages */
pub const A3_VOICE_MSG_CONTROL_CHANGE: u8 = 0x0B;
pub const A3_VOICE_MSG_PROGRAM_CHANGE: u8 = 0x0C;
pub const A3_VOICE_MSG_CHANNEL_PRESSURE: u8 = 0x0D;
pub const A3_VOICE_MSG_PITCH_BEND: u8 = 0x0E;

/* Module administration opcodes */
pub const A3_ADMIN_SIGN_IN: u8 = 0x01;
pub const A3_ADMIN_NOTIFY_ID: u8 = 0x02;
pub const A3_ADMIN_REQ_UID_CANCEL: u8 = 0x03;

/* Mission control opcodes */
pub const A3_MC_SIGN_IN: u8 = 0x01;
pub const A3_MC_ASSIGN_MODULE_ID: u8 = 0x02;
pub const A3_MC_PING: u8 = 0x03;
pub const A3_MC_REQUEST_NAME: u8 = 0x04;
pub const A3_MC_CONTINUE_NAME: u8 = 0x05;

/* Individual module opcodes */
pub const A3_IM_REPLY_PING: u8 = 0x01;
pub const A3_IM_REPLY_NAME: u8 = 0x02;

pub const A3_DATA_LENGTH: u8 = 8;

#[derive(Debug)]
pub enum Value {
    U8(u8),
    U16(u16),
    U32(u32),
    Text(String),
    Bool(bool),
}

#[derive(Debug, Clone)]
pub enum ValueType {
    U8,
    U16,
    U32,
    Text,
    Bool,
}

#[derive(Debug, Clone)]
pub struct Attribute {
    pub name: String,
    pub kind: ValueType,
}

lazy_static! {
    static ref ATTRIBUTES: [Attribute; 256] = {
        let mut l = core::array::from_fn(|_| Attribute {
            name: String::from(""),
            kind: ValueType::U8,
        });

        l[1] = Attribute {
            name: String::from("name"),
            kind: ValueType::Text,
        };

        l
    };
}

#[derive(Debug, Clone)]
pub struct DataField {
    pub id: u8,
    pub length: u8,
    pub data: Vec<u8>,
}

impl DataField {
    pub fn get_attribute(&self) -> Option<&Attribute> {
        let attr = &ATTRIBUTES[self.id as usize];
        if !attr.name.is_empty() {
            Some(attr)
        } else {
            None
        }
    }

    pub fn get_value(&self) -> Option<Value> {
        let attr = self.get_attribute()?;
        Some(self.get_value_with_type(&attr.kind))
    }

    pub fn get_value_with_type(&self, kind: &ValueType) -> Value {
        let value = match kind {
            ValueType::U8 => Value::U8(self.data[0]),
            ValueType::U16 => Value::U16(((self.data[0] as u16) << 8) + self.data[1] as u16),
            ValueType::U32 => Value::U32(
                ((self.data[0] as u32) << 24)
                    + ((self.data[1] as u32) << 16)
                    + ((self.data[2] as u32) << 8)
                    + self.data[3] as u32,
            ),
            ValueType::Text => Value::Text(String::from_utf8(self.data.clone()).unwrap()),
            ValueType::Bool => Value::Bool(self.data[0] != 0),
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
            return Err(DataParsingError {
                message: String::from("Building is done already"),
            });
        }
        let Some(acc_data) = &mut self.data.as_mut() else {
            return Err(DataParsingError {
                message: String::from("Data field is not set"),
            });
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

    pub fn build(&mut self) -> Result<DataField> {
        if self.data_pos < self.length as usize {
            return Err(DataParsingError {
                message: String::from("Not yet ready to build"),
            });
        }
        if let Some(data) = self.data.take() {
            return Ok(DataField {
                id: self.id,
                length: self.length,
                data,
            });
        }
        return Err(DataParsingError {
            message: String::from("Data field is not set"),
        });
    }
}

#[derive(Debug, Clone)]
pub struct ChunkBuilder {
    chunk: Option<Vec<DataField>>,
    target_num_fields: usize,
    num_fields_read: bool,
    field_builder: DataFieldBuilder,
}

impl ChunkBuilder {
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
            return Err(DataParsingError {
                message: String::from("Chunk field is not set"),
            });
        };
        if self.num_fields_read && chunk.len() == self.target_num_fields {
            return Err(DataParsingError {
                message: String::from("Building is done already"),
            });
        }

        let mut index = 0;
        if !self.num_fields_read {
            self.target_num_fields = data[index] as usize;
            self.num_fields_read = true;
            index += 1;
        }
        while index < data_length {
            let (field_done, next_index) = self.field_builder.data(data, data_length, index)?;
            index = next_index;
            if field_done {
                chunk.push(self.field_builder.build().unwrap());
                self.field_builder = DataFieldBuilder::new();
            }
        }
        return Ok(chunk.len() == self.target_num_fields);
    }

    pub fn build(&mut self) -> Result<Vec<DataField>> {
        let Some(chunk) = self.chunk.as_ref() else {
            return Err(DataParsingError {
                message: String::from("Chunk field is not set"),
            });
        };
        if !self.num_fields_read || chunk.len() < self.target_num_fields {
            return Err(DataParsingError {
                message: String::from("Not yet ready to build"),
            });
        }
        return Ok(self.chunk.take().unwrap());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_single_field_segment() {
        let data = b"\x01\x05hello";
        let mut builder = DataFieldBuilder::new();
        let result = builder.data(data, 7, 0).unwrap();
        assert_eq!(result, (true, 7));
        let data_field = builder.build().unwrap();
        assert_eq!(data_field.id, 1);
        assert_eq!(data_field.data.as_slice(), b"hello");
        assert_eq!(data_field.get_attribute().unwrap().name, "name".to_string());
        let Value::Text(value) = data_field.get_value().unwrap() else {
            panic!("unexpected value type");
        };
        assert_eq!(value, "hello".to_string());
    }

    #[test]
    fn test_parse_two_field_segments() {
        let data1 = b"\x01\x02hi\x02\x05h";
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
        let data1 = b"\x02\x01\x02hi\x02\x05";
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
        let data = b"\x01\x05hello";
        let mut builder = ChunkBuilder::for_single_field();
        let result = builder.data(data, 7).unwrap();
        assert!(result);
        let data_fields = builder.build().unwrap();
        assert_eq!(data_fields.len(), 1);
        let data_field = &data_fields[0];
        assert_eq!(data_field.id, 1);
        assert_eq!(data_field.data.as_slice(), b"hello");
        assert_eq!(data_field.get_attribute().unwrap().name, "name".to_string());
        let Value::Text(value) = data_field.get_value().unwrap() else {
            panic!("unexpected value type");
        };
        assert_eq!(value, "hello".to_string());
    }
}
