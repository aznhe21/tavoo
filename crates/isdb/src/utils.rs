/// `data`からビッグエンディアンで16ビット符号無し整数を読み込む。
///
/// 事前に`data`の長さが2以上あると分かるようなコードであれば最適化が期待できる。
#[inline]
pub fn read_be_16(data: &[u8]) -> u16 {
    u16::from_be_bytes(data[..2].try_into().unwrap())
}

/// `data`からビッグエンディアンで32ビット符号無し整数を読み込む。
///
/// 事前に`data`の長さが4以上あると分かるようなコードであれば最適化が期待できる。
#[inline]
pub fn read_be_32(data: &[u8]) -> u32 {
    u32::from_be_bytes(data[..4].try_into().unwrap())
}

/// 条件が常に一致しているものとして事前条件を示す。
///
/// 後続する処理ではこの条件が満たされることを前提とした最適化が行われる可能性がある。
///
/// # Safety
///
/// この条件が満たされない場合の動作は未定義である。
macro_rules! assume {
    ($cond:expr) => {{
        if cfg!(debug_assertions) {
            assert!($cond);
        } else if !($cond) {
            std::hint::unreachable_unchecked();
        }
    }};
}

// マクロはpub useできない
pub(crate) use assume;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_be_u16() {
        assert_eq!(read_be_16(b"\x12\x34\x56\x78"), 0x1234);
    }

    #[test]
    fn test_read_be_u32() {
        assert_eq!(read_be_32(b"\x12\x34\x56\x78\x9A\xBC\xDE"), 0x12345678);
    }
}
