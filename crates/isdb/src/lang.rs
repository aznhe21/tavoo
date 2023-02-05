//! ARIBで使用される文字関係の定義。

use std::fmt;

/// ISO 639-2で規定される3文字の言語コード。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LangCode(pub [u8; 3]);

// ARIB TR-B14より。
impl LangCode {
    /// 日本語。
    pub const JPN: LangCode = LangCode(*b"jpn");
    /// 英語。
    pub const ENG: LangCode = LangCode(*b"eng");
    /// ドイツ語。
    pub const DEU: LangCode = LangCode(*b"DEU");
    /// フランス語。
    pub const FRA: LangCode = LangCode(*b"fra");
    /// イタリア語。
    pub const ITA: LangCode = LangCode(*b"ita");
    /// ロシア語。
    pub const RUS: LangCode = LangCode(*b"rus");
    /// 中国語。
    pub const ZHO: LangCode = LangCode(*b"zho");
    /// 韓国語。
    pub const KOR: LangCode = LangCode(*b"kor");
    /// スペイン語。
    pub const SPA: LangCode = LangCode(*b"spa");
    /// 外国語。
    pub const ETC: LangCode = LangCode(*b"etc");
}

impl fmt::Display for LangCode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.escape_ascii().fmt(f)
    }
}
