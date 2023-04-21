use std::fmt::{self, Debug};

use base64::engine::{general_purpose::STANDARD, Engine};

/// バイナリデータをBase64でシリアライズ・デシリアライズするためのオブジェクト。
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Binary(pub Vec<u8>);

impl From<Vec<u8>> for Binary {
    #[inline]
    fn from(value: Vec<u8>) -> Binary {
        Binary(value)
    }
}

impl fmt::Display for Binary {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl fmt::Debug for Binary {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl serde::Serialize for Binary {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_str(&base64::display::Base64Display::new(&*self.0, &STANDARD))
    }
}

impl<'de> serde::Deserialize<'de> for Binary {
    fn deserialize<D>(deserializer: D) -> Result<Binary, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_str(Visitor)
    }
}

struct Visitor;

impl<'de> serde::de::Visitor<'de> for Visitor {
    type Value = Binary;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("base64 string")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        match STANDARD.decode(v) {
            Ok(v) => Ok(Binary(v)),
            Err(e) => Err(E::custom(e)),
        }
    }

    fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(Binary(v.to_vec()))
    }

    fn visit_byte_buf<E>(self, v: Vec<u8>) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(Binary(v))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_binary() {
        assert_eq!(
            serde_json::to_value(&Binary(vec![0, 1, 2, 3])).unwrap(),
            serde_json::json!("AAECAw=="),
        );

        assert_eq!(
            serde_json::from_value::<Binary>(serde_json::json!("AAECAw==")).unwrap(),
            Binary(vec![0, 1, 2, 3]),
        );
    }
}
