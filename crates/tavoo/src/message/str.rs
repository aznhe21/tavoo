use super::bin::Binary;

#[derive(Debug, Clone, serde::Serialize)]
pub struct AribString(Binary);

impl From<&isdb::eight::str::AribStr> for AribString {
    #[inline]
    fn from(s: &isdb::eight::str::AribStr) -> AribString {
        AribString(Binary(s.as_bytes().to_vec()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arib_string() {
        assert_eq!(
            serde_json::to_value(&AribString(Binary(vec![0, 1, 2, 3]))).unwrap(),
            serde_json::json!("AAECAw=="),
        );
    }
}
