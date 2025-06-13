use crate::analog3::Value;

pub struct ParseParamError {}

pub struct Spec {
    pub name: String,
    pub required: bool,
    pub parse: fn(&String) -> Result<Value, ParseParamError>,
}

impl Spec {
    pub fn u8(name: &str, required: bool) -> Self {
        Self {
            name: name.to_string(),
            required: required,
            parse: |src| {
                let trimmed = src.trim();
                let parse_u8 = || {
                    if trimmed.starts_with("0x") {
                        u8::from_str_radix(trimmed.trim_start_matches("0x"), 16)
                    } else {
                        u8::from_str_radix(trimmed, 10)
                    }
                };
                return match parse_u8() {
                    Ok(value) => Ok(Value::U8(value)),
                    Err(_) => Err(ParseParamError {}),
                };
            },
        }
    }

    #[allow(dead_code)]
    pub fn u16(name: &str, required: bool) -> Self {
        Self {
            name: name.to_string(),
            required: required,
            parse: |src| {
                let trimmed = src.trim();
                let parse_16 = || {
                    if trimmed.starts_with("0x") {
                        u16::from_str_radix(trimmed.trim_start_matches("0x"), 16)
                    } else {
                        u16::from_str_radix(trimmed, 10)
                    }
                };
                return match parse_16() {
                    Ok(value) => Ok(Value::U16(value)),
                    Err(_) => Err(ParseParamError {}),
                };
            },
        }
    }

    pub fn u32(name: &str, required: bool) -> Self {
        Self {
            name: name.to_string(),
            required: required,
            parse: |src| {
                let trimmed = src.trim();
                let parse_16 = || {
                    if trimmed.starts_with("0x") {
                        u32::from_str_radix(trimmed.trim_start_matches("0x"), 16)
                    } else {
                        u32::from_str_radix(trimmed, 10)
                    }
                };
                return match parse_16() {
                    Ok(value) => Ok(Value::U32(value)),
                    Err(_) => Err(ParseParamError {}),
                };
            },
        }
    }

    #[allow(dead_code)]
    pub fn str(name: String, required: bool) -> Self {
        Self {
            name: name,
            required: required,
            parse: |src| Ok(Value::Text(src.trim().to_string())),
        }
    }

    pub fn bool(name: &str, required: bool) -> Self {
        Self {
            name: name.to_string(),
            required: required,
            parse: |src| {
                return match src.trim().parse() {
                    Ok(value) => Ok(Value::Bool(value)),
                    Err(_) => Err(ParseParamError {}),
                };
            },
        }
    }
}
