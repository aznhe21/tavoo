use std::fmt;
use std::ops;

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

/// `b`からBCDを読み取る。
#[inline]
pub fn read_bcd_digit(b: u8) -> u8 {
    ((b >> 4) * 10) + (b & 0x0F)
}

/// `data`から`digits`桁のBCDで数値を読み込む。
pub fn read_bcd<T>(data: &[u8], digits: usize) -> T
where
    T: Default
        + From<u8>
        + ops::Add<T, Output = T>
        + ops::Mul<T, Output = T>
        + ops::AddAssign<T>
        + ops::MulAssign<T>,
{
    let mut value = data[..digits / 2].iter().fold(T::default(), |value, &v| {
        value * 100.into() + read_bcd_digit(v).into()
    });

    if (digits & 1) != 0 {
        value *= 10.into();
        value += (data[digits / 2] >> 4).into();
    }

    value
}

/// 要素数`N`のヒープに確保される配列を、`f`を呼び出した戻り値で生成する。
pub fn boxed_array<T, const N: usize, F>(f: F) -> Box<[T; N]>
where
    F: FnMut(usize) -> T,
{
    let slice = (0..N).map(f).collect::<Vec<T>>().into_boxed_slice();

    // Safety: 要素数の分かっている`Box<[T]>`から`Box<[T; N]>`への変換でしかない
    unsafe { Box::from_raw(Box::into_raw(slice) as *mut [T; N]) }
}

/// 内包する値を`fmt::UpperHex`で出力する。
pub struct UpperHex<T>(pub T);

impl<T: fmt::UpperHex> fmt::Debug for UpperHex<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::UpperHex::fmt(&self.0, f)
    }
}

/// スライス型用拡張トレイト。
pub trait SliceExt {
    /// スライスの要素型。
    type Item;

    /// スライスを`mid`の位置で分割する。
    ///
    /// `mid`が要素数より大きい場合は`None`を返す。
    fn split_at_checked(&self, mid: usize) -> Option<(&[Self::Item], &[Self::Item])>;
}

impl<T> SliceExt for [T] {
    type Item = T;

    fn split_at_checked(&self, mid: usize) -> Option<(&[T], &[T])> {
        if mid > self.len() {
            None
        } else {
            // Safety: `mid`は配列中のインデックスでありアクセス可能
            unsafe {
                Some((
                    std::slice::from_raw_parts(self.as_ptr(), mid),
                    std::slice::from_raw_parts(self.as_ptr().add(mid), self.len() - mid),
                ))
            }
        }
    }
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

    #[test]
    fn test_read_bcd() {
        assert_eq!(read_bcd_digit(0x12), 12);

        assert_eq!(read_bcd::<u32>(&[0x12, 0x34, 0x56, 0x78], 1), 1);
        assert_eq!(read_bcd::<u32>(&[0x12, 0x34, 0x56, 0x78], 2), 12);
        assert_eq!(read_bcd::<u32>(&[0x12, 0x34, 0x56, 0x78], 3), 123);
        assert_eq!(read_bcd::<u32>(&[0x12, 0x34, 0x56, 0x78], 4), 1234);
        assert_eq!(read_bcd::<u32>(&[0x12, 0x34, 0x56, 0x78], 5), 12345);
        assert_eq!(read_bcd::<u32>(&[0x12, 0x34, 0x56, 0x78], 6), 123456);
        assert_eq!(read_bcd::<u32>(&[0x12, 0x34, 0x56, 0x78], 7), 1234567);
        assert_eq!(read_bcd::<u32>(&[0x12, 0x34, 0x56, 0x78], 8), 12345678);
    }

    #[test]
    fn test_boxed_array() {
        assert_eq!(
            boxed_array::<Option<[u8; 64]>, 8192, _>(|_| None),
            (0..8192)
                .map(|_| None)
                .collect::<Vec<Option<[u8; 64]>>>()
                .try_into()
                .unwrap(),
        );
    }

    #[test]
    fn test_slice_ext_split_at_checked() {
        assert_eq!(
            [0, 1, 2].split_at_checked(0),
            Some((&[] as &[_], &[0, 1, 2] as &[_])),
        );
        assert_eq!(
            [0, 1, 2].split_at_checked(1),
            Some((&[0] as &[_], &[1, 2] as &[_])),
        );
        assert_eq!(
            [0, 1, 2].split_at_checked(2),
            Some((&[0, 1] as &[_], &[2] as &[_])),
        );
        assert_eq!(
            [0, 1, 2].split_at_checked(3),
            Some((&[0, 1, 2] as &[_], &[] as &[_])),
        );
        assert_eq!([0, 1, 2].split_at_checked(4), None);
    }
}
