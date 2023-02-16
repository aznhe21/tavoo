//! 8単位符号の文字列表現。

use std::borrow::{Borrow, Cow};
use std::fmt::{self, Write};
use std::ops;

use super::char::{AribChar, CharSize, GraphicChar};
use super::decode::{self, Decoder};

/// 借用された8単位符号を表す型。
///
/// `AribStr`と[`AribString`]は、<code>&[str]</code>と[`String`]の関係と相似しており、
/// 前者は借用された参照、後者は所有権を持つ文字列である。
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AribStr([u8]);

impl AribStr {
    /// バイト列から`AribStr`を生成する。
    #[must_use]
    #[inline]
    pub const fn from_bytes(bytes: &[u8]) -> &AribStr {
        unsafe { &*(bytes as *const [u8] as *const AribStr) }
    }

    /// この文字列の長さを返す。
    #[must_use]
    #[inline]
    pub const fn len(&self) -> usize {
        self.as_bytes().len()
    }

    /// この文字列が空であるかどうかを返す。
    #[must_use]
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// 文字列スライスをバイトスライスに変換する。
    #[must_use]
    #[inline]
    pub const fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// コピー無しに`Box<AribStr>`を`Box<[u8]>`に変換する。
    #[must_use]
    #[inline]
    pub fn into_boxed_bytes(self: Box<AribStr>) -> Box<[u8]> {
        self.into()
    }

    /// コピー無しに`Box<AribStr>`を`AribString`に変換する。
    #[must_use]
    #[inline]
    pub fn into_arib_string(self: Box<AribStr>) -> AribString {
        let slice = Box::<[u8]>::from(self);
        AribString(slice.into_vec())
    }

    /// [`decode::Options`]に従い8単位符号を`String`に変換する。
    ///
    /// 文字に変換できない文字符号は[`U+FFFD REPLACEMENT
    /// CHARACTER`][`char::REPLACEMENT_CHARACTER`]に変換される。
    ///
    /// 動作としては`self.display(opts).to_string()`と同じだが、こちらの方がより効率的な可能性がある。
    pub fn to_string(&self, opts: decode::Options) -> String {
        let mut decoder = Decoder::new(self.as_bytes(), opts);
        let Some(mut c) = decoder.next_graphic() else {
            return String::new();
        };

        // 制御文字があるにしてもUTF-8にしたら同じくらい？（適当）
        let mut buf = String::with_capacity(self.len());
        let mut char_size = CharSize::default();

        const REPLACEMENT_STR: &str = "\u{FFFD}";
        loop {
            match c {
                GraphicChar::Generic(c) => {
                    let c = c.to_char(char_size).unwrap_or(char::REPLACEMENT_CHARACTER);
                    buf.push(c);
                }
                GraphicChar::Mosaic(_) | GraphicChar::Drcs(_) => {
                    buf.push_str(REPLACEMENT_STR);
                }
                GraphicChar::ActivePositionReturn => buf.push_str("\n"),
                GraphicChar::Space => buf.push_str(if char_size.is_small() { " " } else { "　" }),
                GraphicChar::CharSize(size) => char_size = size,
            }

            let Some(next) = decoder.next_graphic() else { break };
            c = next;
        }

        buf
    }

    /// `opts`に従い文字符号を安全に表示するための、
    /// [`Display`][`fmt::Display`]を実装したオブジェクトを返す。
    #[inline]
    pub fn display(&self, opts: decode::Options) -> Display {
        Display { inner: self, opts }
    }

    /// 文字列中の各[`AribChar`]を返すイテレーターを生成する。
    #[inline]
    pub fn decode(&self, opts: decode::Options) -> AribChars {
        AribChars {
            decoder: Decoder::new(self.as_bytes(), opts),
        }
    }
}

impl Default for &AribStr {
    fn default() -> Self {
        AribStr::from_bytes(&[])
    }
}

impl fmt::Debug for AribStr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AribStr(")?;
        f.debug_list()
            .entries(self.0.iter().map(|c| crate::utils::UpperHex(c)))
            .finish()?;
        f.write_str(")")
    }
}

impl From<&AribStr> for Box<AribStr> {
    fn from(s: &AribStr) -> Self {
        let boxed: Box<[u8]> = Box::from(s.as_bytes());
        // Safety: [u8]とAribStrは互換
        unsafe { Box::from_raw(Box::into_raw(boxed) as *mut AribStr) }
    }
}

impl From<Box<AribStr>> for Box<[u8]> {
    #[inline]
    fn from(s: Box<AribStr>) -> Self {
        // Safety: AribStrと[u8]は互換
        unsafe { Box::from_raw(Box::into_raw(s) as *mut [u8]) }
    }
}

impl<'a> From<&'a AribStr> for Cow<'a, AribStr> {
    #[inline]
    fn from(s: &'a AribStr) -> Self {
        Cow::Borrowed(s)
    }
}

impl From<Cow<'_, AribStr>> for Box<AribStr> {
    #[inline]
    fn from(s: Cow<'_, AribStr>) -> Self {
        match s {
            Cow::Borrowed(s) => Box::from(s),
            Cow::Owned(s) => Box::from(s),
        }
    }
}

impl AsRef<AribStr> for AribStr {
    #[inline]
    fn as_ref(&self) -> &AribStr {
        self
    }
}

/// 所有権を持つ8単位符号を表す型。
///
/// `AribString`と<code>&[AribStr]</code>は、[`String`]と<code>&[str]</code>の関係と相似しており、
/// 前者は所有権を持つ文字列、後者は借用された参照である。
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AribString(Vec<u8>);

impl AribString {
    /// 空の`AribString`を生成する。
    #[inline]
    #[must_use]
    pub const fn new() -> AribString {
        AribString(Vec::new())
    }

    /// `AribString`をバイトのベクタに変換する。
    ///
    /// `AribString`を消費するため内容はコピーされない。
    #[inline]
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.0
    }

    /// `AribString`の内容をバイトのスライスで返す。
    #[inline]
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &*self.0
    }

    /// 文字列全体を含む[`AribStr`]スライスを抽出する。
    #[inline]
    #[must_use]
    pub fn as_arib_str(&self) -> &AribStr {
        &**self
    }

    /// `AribString`を`Box`に包んだ[`AribStr`]で返す。
    #[inline]
    pub fn into_boxed_arib_str(self) -> Box<AribStr> {
        // Safety: [u8]とAribStrは互換
        unsafe { Box::from_raw(Box::into_raw(self.0.into_boxed_slice()) as *mut AribStr) }
    }

    /// `AribString`を切り詰めて全内容を削除する。
    pub fn clear(&mut self) {
        self.0.clear();
    }

    /// `AribString`に指定された8単位符号を追記する。
    #[inline]
    pub fn push_str(&mut self, string: &AribStr) {
        self.0.extend_from_slice(string.as_bytes());
    }
}

impl Default for AribString {
    #[inline]
    fn default() -> AribString {
        AribString::new()
    }
}

impl ops::Deref for AribString {
    type Target = AribStr;

    #[inline]
    fn deref(&self) -> &Self::Target {
        AribStr::from_bytes(&*self.0)
    }
}

impl fmt::Debug for AribString {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl AsRef<AribStr> for AribString {
    #[inline]
    fn as_ref(&self) -> &AribStr {
        &**self
    }
}

impl AsRef<[u8]> for AribString {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl ops::AddAssign<&AribStr> for AribString {
    #[inline]
    fn add_assign(&mut self, rhs: &AribStr) {
        self.push_str(rhs);
    }
}

impl From<AribString> for Vec<u8> {
    #[inline]
    fn from(s: AribString) -> Self {
        s.into_bytes()
    }
}

impl From<AribString> for Box<AribStr> {
    #[inline]
    fn from(s: AribString) -> Self {
        s.into_boxed_arib_str()
    }
}

impl From<&AribStr> for AribString {
    fn from(s: &AribStr) -> Self {
        s.to_owned()
    }
}

impl From<AribString> for Cow<'_, AribStr> {
    #[inline]
    fn from(s: AribString) -> Self {
        Cow::Owned(s)
    }
}

impl<'a> From<&'a AribString> for Cow<'a, AribStr> {
    #[inline]
    fn from(s: &'a AribString) -> Self {
        Cow::Borrowed(s.as_arib_str())
    }
}

impl From<Cow<'_, AribStr>> for AribString {
    #[inline]
    fn from(s: Cow<'_, AribStr>) -> Self {
        s.into_owned()
    }
}

impl From<Box<AribStr>> for AribString {
    #[inline]
    fn from(s: Box<AribStr>) -> Self {
        s.into_arib_string()
    }
}

impl Borrow<AribStr> for AribString {
    #[inline]
    fn borrow(&self) -> &AribStr {
        &**self
    }
}

impl ToOwned for AribStr {
    type Owned = AribString;

    fn to_owned(&self) -> Self::Owned {
        AribString(self.as_bytes().into())
    }

    fn clone_into(&self, target: &mut Self::Owned) {
        let mut b = std::mem::take(&mut target.0);
        self.as_bytes().clone_into(&mut b);
        target.0 = b;
    }
}

/// [`AribStr`]内の各[`AribChar`]を返すイテレーター。
#[derive(Clone)]
pub struct AribChars<'a> {
    decoder: Decoder<'a>,
}

impl<'a> AribChars<'a> {
    /// 内包するデータを元の文字列に対する部分スライスとして得る。
    #[inline]
    pub fn as_arib_str(&self) -> &'a AribStr {
        AribStr::from_bytes(self.decoder.as_bytes())
    }
}

impl<'a> Iterator for AribChars<'a> {
    type Item = AribChar;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.decoder.next_char()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.decoder.as_bytes().len()))
    }
}

impl<'a> fmt::Debug for AribChars<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AribChars(")?;
        f.debug_list().entries(self.clone()).finish()?;
        f.write_str(")")
    }
}

/// [`AribStr`]をUTF-8として表示するための構造体。
pub struct Display<'a> {
    inner: &'a AribStr,
    opts: decode::Options,
}

impl<'a> fmt::Display for Display<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        const REPLACEMENT_STR: &str = "\u{FFFD}";

        let mut decoder = Decoder::new(self.inner.as_bytes(), self.opts);
        let mut char_size = CharSize::default();

        while let Some(c) = decoder.next_graphic() {
            match c {
                GraphicChar::ActivePositionReturn => f.write_str("\n")?,
                GraphicChar::Space => {
                    f.write_str(if char_size.is_small() { " " } else { "　" })?
                }
                GraphicChar::Generic(c) => {
                    let c = c.to_char(char_size).unwrap_or(char::REPLACEMENT_CHARACTER);
                    f.write_char(c)?;
                }
                GraphicChar::Mosaic(_) | GraphicChar::Drcs(_) => f.write_str(REPLACEMENT_STR)?,
                GraphicChar::CharSize(size) => char_size = size,
            }
        }

        Ok(())
    }
}

impl<'a> fmt::Debug for Display<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.inner, f)
    }
}
