use crate::analog3::config::{Value, parse_u8, parse_u16, parse_u32};

#[derive(Debug)]
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
            parse: |src| match parse_u8(src) {
                Ok(value) => Ok(Value::U8(value)),
                Err(_) => Err(ParseParamError {}),
            },
        }
    }

    #[allow(dead_code)]
    pub fn u16(name: &str, required: bool) -> Self {
        Self {
            name: name.to_string(),
            required: required,
            parse: |src| match parse_u16(src) {
                Ok(value) => Ok(Value::U16(value)),
                Err(_) => Err(ParseParamError {}),
            },
        }
    }

    pub fn u32(name: &str, required: bool) -> Self {
        Self {
            name: name.to_string(),
            required: required,
            parse: |src| match parse_u32(src) {
                Ok(value) => Ok(Value::U32(value)),
                Err(_) => Err(ParseParamError {}),
            },
        }
    }

    pub fn str(name: &str, required: bool) -> Self {
        Self {
            name: name.to_string(),
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
                    Ok(value) => Ok(Value::Boolean(value)),
                    Err(_) => Err(ParseParamError {}),
                };
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_u8() {
        let spec = Spec::u8("velocity", true);
        assert_eq!(spec.name, "velocity");
        let Ok(out) = (spec.parse)(&"123".to_string()) else {
            panic!();
        };
        assert_eq!(out.as_u8().unwrap(), 123u8);

        // retrieve the value as wrong types
        assert!(out.as_u16().is_err());
        assert!(out.as_u32().is_err());
        assert!(out.as_text().is_err());
        assert!(out.as_bool().is_err());

        let Ok(out2) = (spec.parse)(&"0x71".to_string()) else {
            panic!();
        };
        assert_eq!(out2.as_u8().unwrap(), 0x71u8);

        let Ok(out3) = (spec.parse)(&" 213 ".to_string()) else {
            panic!();
        };
        assert_eq!(out3.as_u8().unwrap(), 213);

        let Ok(out4) = (spec.parse)(&" 0xca ".to_string()) else {
            panic!();
        };
        assert_eq!(out4.as_u8().unwrap(), 0xcau8);

        // parse errors
        assert!((spec.parse)(&"no".to_string()).is_err());
        assert!((spec.parse)(&"321".to_string()).is_err());
        assert!((spec.parse)(&"0xbad".to_string()).is_err());
    }

    #[test]
    fn test_u16() {
        let spec = Spec::u16("sixteen", true);
        assert_eq!(spec.name, "sixteen");
        let Ok(out) = (spec.parse)(&"65432".to_string()) else {
            panic!();
        };
        assert_eq!(out.as_u16().unwrap(), 65432u16);

        // retrieve the value as wrong types
        assert!(out.as_u8().is_err());
        assert!(out.as_u32().is_err());
        assert!(out.as_text().is_err());
        assert!(out.as_bool().is_err());

        let Ok(out2) = (spec.parse)(&"0xcafe".to_string()) else {
            panic!();
        };
        assert_eq!(out2.as_u16().unwrap(), 0xcafeu16);

        let Ok(out3) = (spec.parse)(&" 13245 ".to_string()) else {
            panic!();
        };
        assert_eq!(out3.as_u16().unwrap(), 13245u16);

        let Ok(out4) = (spec.parse)(&" 0xbad ".to_string()) else {
            panic!();
        };
        assert_eq!(out4.as_u16().unwrap(), 0xbadu16);

        // parse errors
        assert!((spec.parse)(&"bad".to_string()).is_err());
        assert!((spec.parse)(&"76543".to_string()).is_err());
        assert!((spec.parse)(&"0xbaddata".to_string()).is_err());
    }

    #[test]
    fn test_u32() {
        let spec = Spec::u32("thirty-two", true);
        assert_eq!(spec.name, "thirty-two");
        let Ok(out) = (spec.parse)(&"12345678".to_string()) else {
            panic!();
        };
        assert_eq!(out.as_u32().unwrap(), 12345678u32);

        // retrieve the value as wrong types
        assert!(out.as_u8().is_err());
        assert!(out.as_u16().is_err());
        assert!(out.as_text().is_err());
        assert!(out.as_bool().is_err());

        let Ok(out2) = (spec.parse)(&"0xba5eba11".to_string()) else {
            panic!();
        };
        assert_eq!(out2.as_u32().unwrap(), 0xba5eba11u32);

        let Ok(out3) = (spec.parse)(&" 87654321 ".to_string()) else {
            panic!();
        };
        assert_eq!(out3.as_u32().unwrap(), 87654321u32);

        let Ok(out4) = (spec.parse)(&" 0xdeadbeef ".to_string()) else {
            panic!();
        };
        assert_eq!(out4.as_u32().unwrap(), 0xdeadbeefu32);

        // parse errors
        assert!((spec.parse)(&"bad".to_string()).is_err());
        assert!((spec.parse)(&"99999999999999999".to_string()).is_err());
        assert!((spec.parse)(&"0xbadbadbeef".to_string()).is_err());
    }

    #[test]
    fn test_str() {
        let spec = Spec::str("nickname", true);
        assert_eq!(spec.name, "nickname");
        let Ok(out) = (spec.parse)(&"hello".to_string()) else {
            panic!();
        };
        assert_eq!(out.as_text().unwrap(), "hello");

        // retrieve the value as wrong types
        assert!(out.as_u8().is_err());
        assert!(out.as_u16().is_err());
        assert!(out.as_u32().is_err());
        assert!(out.as_bool().is_err());

        let Ok(out2) = (spec.parse)(&" world ".to_string()) else {
            panic!();
        };
        assert_eq!(out2.as_text().unwrap(), "world");
    }

    #[test]
    fn test_bool() {
        let spec = Spec::bool("yes_or_no", true);
        assert_eq!(spec.name, "yes_or_no");
        let Ok(out) = (spec.parse)(&"true".to_string()) else {
            panic!();
        };
        assert!(out.as_bool().unwrap());

        // retrieve the value as wrong types
        assert!(out.as_u8().is_err());
        assert!(out.as_u16().is_err());
        assert!(out.as_u32().is_err());
        assert!(out.as_text().is_err());

        let Ok(out2) = (spec.parse)(&"false".to_string()) else {
            panic!();
        };
        assert!(!out2.as_bool().unwrap());

        // parse errors
        assert!((spec.parse)(&"bad".to_string()).is_err());
        assert!((spec.parse)(&"no".to_string()).is_err());
        assert!((spec.parse)(&"TRUE".to_string()).is_err()); // must be lower case
    }
}
