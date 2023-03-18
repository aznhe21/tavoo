//! 8単位符号の文字要素。

use std::fmt::{self, Write};

use super::table;

/// 文字サイズ。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CharSize {
    /// 小型。
    Small,
    /// 中型。
    Medium,
    /// 標準。
    #[default]
    Normal,
    /// 超小型。
    Micro,
    /// 縦倍。
    HighW,
    /// 横倍。
    WidthW,
    /// 縦横倍。
    SizeW,
    /// 特殊1。
    Special1,
    /// 特殊2。
    Special2,
}

impl CharSize {
    /// 文字サイズが小さめの場合に`true`を返す。
    #[inline]
    pub fn is_small(self) -> bool {
        matches!(self, CharSize::Small | CharSize::Medium | CharSize::Micro)
    }
}

/// 図形領域の符号で、`0x21..=0x7E`の範囲のみ保持する。
///
/// `get`メソッドで値を取得する際、値域がコンパイラに伝わるため最適化による分岐の削減が期待できる。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GraphicCode(u8);

impl GraphicCode {
    /// `u8`の値から`GraphicCode`を生成する。
    ///
    /// # パニック
    ///
    /// `n`の値が`0x21..=0x7E`の範囲にない場合、このメソッドはパニックする。
    #[inline]
    pub fn new(n: u8) -> GraphicCode {
        assert!((0x21..=0x7E).contains(&n));
        GraphicCode(n)
    }

    /// 符号を`0x21..=0x7E`の範囲に制限された`u8`で返す。
    #[inline]
    pub fn get(self) -> u8 {
        // Safety: `GraphicCode`を生成できている時点で値は範囲内
        unsafe { crate::utils::assume!((0x21..=0x7E).contains(&self.0)) }
        self.0
    }
}

impl fmt::Debug for GraphicCode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("0x")?;
        fmt::UpperHex::fmt(&self.0, f)
    }
}

macro_rules! generic_chars {
    (#[$doc:meta] $type:ident(2) => $decode:ident, $($tt:tt)*) => {
        #[$doc]
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $type(pub GraphicCode, pub GraphicCode);

        impl $type {
            #[doc = concat!("`", stringify!($type), "`を`char`に変換する。")]
            ///
            /// 未割り当ての文字や`char`として表現できない文字の場合は`None`を返す。
            pub fn to_char(&self, char_size: CharSize) -> Option<char> {
                table::$decode(self.0, self.1, char_size)
            }
        }

        impl fmt::Debug for $type {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                if let Some(c) = self.to_char(CharSize::Normal) {
                    fmt::Debug::fmt(&c, f)
                } else {
                    f.debug_tuple(stringify!($type)).field(&self.0).field(&self.1).finish()
                }
            }
        }

        impl fmt::Display for $type {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_char(self.to_char(CharSize::Normal).unwrap_or(char::REPLACEMENT_CHARACTER))
            }
        }

        generic_chars!($($tt)*);
    };
    (#[$doc:meta] $type:ident(1) => $decode:ident, $($tt:tt)*) => {
        #[$doc]
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $type(pub GraphicCode);

        impl $type {
            #[doc = concat!("`", stringify!($type), "`を`char`に変換する。")]
            ///
            /// 未割り当ての文字や`char`として表現できない文字の場合は`None`を返す。
            pub fn to_char(&self, char_size: CharSize) -> Option<char> {
                table::$decode(self.0, char_size)
            }
        }

        impl fmt::Debug for $type {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                if let Some(c) = self.to_char(CharSize::Normal) {
                    fmt::Debug::fmt(&c, f)
                } else {
                    f.debug_tuple(stringify!($type)).field(&self.0).finish()
                }
            }
        }

        impl fmt::Display for $type {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_char(self.to_char(CharSize::Normal).unwrap_or(char::REPLACEMENT_CHARACTER))
            }
        }

        generic_chars!($($tt)*);
    };
    () => {};
}

generic_chars! {
    /// 漢字。
    Kanji(2) => decode_kanji,

    /// 英数。
    Alnum(1) => decode_alnum,

    /// 平仮名。
    Hira(1) => decode_hira,

    /// 片仮名。
    Kata(1) => decode_kata,

    /// プロポーショナル英数。
    PropAlnum(1) => decode_alnum,

    /// プロポーショナル平仮名。
    PropHira(1) => decode_hira,

    /// プロポーショナル片仮名。
    PropKata(1) => decode_kata,

    /// JIS X 0201 片仮名。
    JisXKata(1) => decode_jis_x_kata,

    /// JIS互換漢字1面。
    JisKanjiPlane1(2) => decode_jis_kanji_plane1,

    /// JIS互換漢字2面。
    JisKanjiPlane2(2) => decode_jis_kanji_plane2,

    /// 追加記号。
    ExtraSymbols(2) => decode_extra_symbols,
}

/// 一般的な図形文字。
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum GenericChar {
    /// 漢字。
    Kanji(Kanji),
    /// 英数。
    Alnum(Alnum),
    /// 平仮名。
    Hira(Hira),
    /// 片仮名。
    Kata(Kata),
    /// プロポーショナル英数。
    PropAlnum(PropAlnum),
    /// プロポーショナル平仮名。
    PropHira(PropHira),
    /// プロポーショナル片仮名。
    PropKata(PropKata),
    /// JIS X 0201 片仮名。
    JisXKata(JisXKata),
    /// JIS互換漢字1面。
    JisKanjiPlane1(JisKanjiPlane1),
    /// JIS互換漢字2面。
    JisKanjiPlane2(JisKanjiPlane2),
    /// 追加記号。
    ExtraSymbols(ExtraSymbols),
}

impl GenericChar {
    /// `GenericChar`を`char`に変換する。
    ///
    /// 未割り当ての文字や`char`として表現できない文字の場合は`None`を返す。
    pub fn to_char(&self, char_size: CharSize) -> Option<char> {
        match self {
            GenericChar::Kanji(c) => c.to_char(char_size),
            GenericChar::Alnum(c) => c.to_char(char_size),
            GenericChar::Hira(c) => c.to_char(char_size),
            GenericChar::Kata(c) => c.to_char(char_size),
            GenericChar::PropAlnum(c) => c.to_char(char_size),
            GenericChar::PropHira(c) => c.to_char(char_size),
            GenericChar::PropKata(c) => c.to_char(char_size),
            GenericChar::JisXKata(c) => c.to_char(char_size),
            GenericChar::JisKanjiPlane1(c) => c.to_char(char_size),
            GenericChar::JisKanjiPlane2(c) => c.to_char(char_size),
            GenericChar::ExtraSymbols(c) => c.to_char(char_size),
        }
    }
}

impl fmt::Debug for GenericChar {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            GenericChar::Kanji(c) => fmt::Debug::fmt(c, f),
            GenericChar::Alnum(c) => fmt::Debug::fmt(c, f),
            GenericChar::Hira(c) => fmt::Debug::fmt(c, f),
            GenericChar::Kata(c) => fmt::Debug::fmt(c, f),
            GenericChar::PropAlnum(c) => fmt::Debug::fmt(c, f),
            GenericChar::PropHira(c) => fmt::Debug::fmt(c, f),
            GenericChar::PropKata(c) => fmt::Debug::fmt(c, f),
            GenericChar::JisXKata(c) => fmt::Debug::fmt(c, f),
            GenericChar::JisKanjiPlane1(c) => fmt::Debug::fmt(c, f),
            GenericChar::JisKanjiPlane2(c) => fmt::Debug::fmt(c, f),
            GenericChar::ExtraSymbols(c) => fmt::Debug::fmt(c, f),
        }
    }
}

impl fmt::Display for GenericChar {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            GenericChar::Kanji(c) => fmt::Display::fmt(c, f),
            GenericChar::Alnum(c) => fmt::Display::fmt(c, f),
            GenericChar::Hira(c) => fmt::Display::fmt(c, f),
            GenericChar::Kata(c) => fmt::Display::fmt(c, f),
            GenericChar::PropAlnum(c) => fmt::Display::fmt(c, f),
            GenericChar::PropHira(c) => fmt::Display::fmt(c, f),
            GenericChar::PropKata(c) => fmt::Display::fmt(c, f),
            GenericChar::JisXKata(c) => fmt::Display::fmt(c, f),
            GenericChar::JisKanjiPlane1(c) => fmt::Display::fmt(c, f),
            GenericChar::JisKanjiPlane2(c) => fmt::Display::fmt(c, f),
            GenericChar::ExtraSymbols(c) => fmt::Display::fmt(c, f),
        }
    }
}

/// モザイク図形文字。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum MosaicChar {
    /// モザイク集合A。
    MosaicA(GraphicCode),

    /// モザイク集合B。
    MosaicB(GraphicCode),

    /// モザイク集合C。
    MosaicC(GraphicCode),

    /// モザイク集合D。
    MosaicD(GraphicCode),
}

/// 動的に再定義が可能な外字。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum DrcsChar {
    /// DRCS-0。
    Drcs0(GraphicCode, GraphicCode),

    /// DRCS-1。
    Drcs1(GraphicCode),

    /// DRCS-2。
    Drcs2(GraphicCode),

    /// DRCS-3。
    Drcs3(GraphicCode),

    /// DRCS-4。
    Drcs4(GraphicCode),

    /// DRCS-5。
    Drcs5(GraphicCode),

    /// DRCS-6。
    Drcs6(GraphicCode),

    /// DRCS-7。
    Drcs7(GraphicCode),

    /// DRCS-8。
    Drcs8(GraphicCode),

    /// DRCS-9。
    Drcs9(GraphicCode),

    /// DRCS-10。
    Drcs10(GraphicCode),

    /// DRCS-11。
    Drcs11(GraphicCode),

    /// DRCS-12。
    Drcs12(GraphicCode),

    /// DRCS-13。
    Drcs13(GraphicCode),

    /// DRCS-14。
    Drcs14(GraphicCode),

    /// DRCS-15。
    Drcs15(GraphicCode),
}

impl DrcsChar {
    /// DRCS-0から順の連番を得る。
    ///
    /// DRCS-0の場合は内包する値がそのまま使われ、
    /// DRCS-1以降はその後に続くように値が続く。
    ///
    /// # サンプル
    ///
    /// ```
    /// use isdb::eight::char::{DrcsChar, GraphicCode};
    ///
    /// assert_eq!(DrcsChar::Drcs0(GraphicCode::new(0x21), GraphicCode::new(0x21)).to_number(), 0);
    /// assert_eq!(DrcsChar::Drcs1(GraphicCode::new(0x21)).to_number(), 8836);
    /// assert_eq!(DrcsChar::Drcs2(GraphicCode::new(0x21)).to_number(), 8930);
    /// assert_eq!(DrcsChar::Drcs3(GraphicCode::new(0x22)).to_number(), 9025);
    /// ```
    pub fn to_number(&self) -> u16 {
        match *self {
            DrcsChar::Drcs0(c1, c2) => {
                (((c1.get() - 0x21) as u16) * 94) + ((c2.get() - 0x21) as u16)
            }
            DrcsChar::Drcs1(c) => 8836 + 94 * 0 + (c.get() - 0x21) as u16,
            DrcsChar::Drcs2(c) => 8836 + 94 * 1 + (c.get() - 0x21) as u16,
            DrcsChar::Drcs3(c) => 8836 + 94 * 2 + (c.get() - 0x21) as u16,
            DrcsChar::Drcs4(c) => 8836 + 94 * 3 + (c.get() - 0x21) as u16,
            DrcsChar::Drcs5(c) => 8836 + 94 * 4 + (c.get() - 0x21) as u16,
            DrcsChar::Drcs6(c) => 8836 + 94 * 5 + (c.get() - 0x21) as u16,
            DrcsChar::Drcs7(c) => 8836 + 94 * 6 + (c.get() - 0x21) as u16,
            DrcsChar::Drcs8(c) => 8836 + 94 * 7 + (c.get() - 0x21) as u16,
            DrcsChar::Drcs9(c) => 8836 + 94 * 8 + (c.get() - 0x21) as u16,
            DrcsChar::Drcs10(c) => 8836 + 94 * 9 + (c.get() - 0x21) as u16,
            DrcsChar::Drcs11(c) => 8836 + 94 * 10 + (c.get() - 0x21) as u16,
            DrcsChar::Drcs12(c) => 8836 + 94 * 11 + (c.get() - 0x21) as u16,
            DrcsChar::Drcs13(c) => 8836 + 94 * 12 + (c.get() - 0x21) as u16,
            DrcsChar::Drcs14(c) => 8836 + 94 * 13 + (c.get() - 0x21) as u16,
            DrcsChar::Drcs15(c) => 8836 + 94 * 14 + (c.get() - 0x21) as u16,
        }
    }
}

/// 時刻制御モード。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TimeControlMode {
    /// フリー。
    Free,

    /// リアルタイム。
    RealTime,

    /// オフセットタイム。
    OffsetTime,

    /// 不明。
    Reserved,
}

/// 図形文字と制御文字からなる、8単位符号の文字型。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AribChar {
    /// 一般的な図形文字。
    Generic(GenericChar),

    /// モザイク図形文字。
    Mosaic(MosaicChar),

    /// DRCS。
    Drcs(DrcsChar),

    /// NUL、空白。
    Null,

    /// APB、動作位置後退。
    ActivePositionBackward,

    /// APF、動作位置前進。
    ActivePositionForward,

    /// APD、動作行前進。
    ActivePositionDown,

    /// APU、動作行後退。
    ActivePositionUp,

    /// APR、動作位置改行。
    ActivePositionReturn,

    /// PAPF、指定動作位置前進。
    ///
    /// 内包する値は`0..=63`の範囲である。
    ParameterizedActivePositionForward(u8),

    /// APS、動作位置指定。
    ///
    /// 内包する値はどちらとも`0..=63`の範囲である。
    ActivePositionSet(u8, u8),

    /// CS、画面消去。
    ClearScreen,

    /// RS、データヘッダ識別符号。
    RecordSeparator,

    /// US、データユニット識別符号。
    UnitSeparator,

    /// SP、スペース。
    Space,

    /// DEL、デリート。
    Delete,

    /// BKFやCOLなどでの前景色の指定。
    ///
    /// 内包する値はカラーパレット下位アドレスで`0..=15`の範囲である。
    ColorForeground(u8),

    /// COLでの背景色の指定。
    ///
    /// 内包する値はカラーパレット下位アドレスで`0..=15`の範囲である。
    ColorBackground(u8),

    /// COLでの前中間色の指定。
    ///
    /// 内包する値はカラーパレット下位アドレスで`0..=15`の範囲である。
    ColorHalfForeground(u8),

    /// COLでの背中間色の指定。
    ///
    /// 内包する値はカラーパレット下位アドレスで`0..=15`の範囲である。
    ColorHalfBackground(u8),

    /// COLでのパレット番号の指定。
    ///
    /// 内包する値はパレット番号で`0..=15`の範囲である。
    ColorPalette(u8),

    /// SSZ・MSZ・NSZ・SZX、指定サイズ等。
    CharSize(CharSize),

    /// FLC、フラッシング制御での正相フラッシング開始。
    FlushingControlStartNormal,
    /// FLC、フラッシング制御での逆相フラッシング開始。
    FlushingControlStartInverted,
    /// FLC、フラッシング制御でのフラッシング終了。
    FlushingControlStop,

    /// POL、パターン極性での正常極性。
    PatternPolarityNormal,
    /// POL、パターン極性での反転極性1。
    PatternPolarityInverted1,
    /// POL、パターン極性での反転極性2。
    PatternPolarityInverted2,

    /// WMM、書込みモード変更での前景色及び背景色を書き込むモード。
    WritingModeBoth,
    /// WMM、書込みモード変更での前景色のみ書き込むモード。
    WritingModeForeground,
    /// WMM、書込みモード変更での背景色のみを書き込むモード。
    WritingModeBackground,

    /// TIMEでの処理待ち。
    ///
    /// 内包する値は中断する時間（単位は0.1秒）で`0..=63`の範囲である。
    WaitForProcess(u8),

    /// TIMEでの時刻制御モード。
    TimeControlMode(TimeControlMode),

    /// TIMEでの提示開始時刻、再生時刻。
    ///
    /// 内包する値は時刻（単位はミリ秒）。
    PresentationStartPlaybackTime(u64),

    /// TIMEでのオフセット時間。
    ///
    /// 内包する値は時間（単位はミリ秒）。
    OffsetTime(u64),

    /// TIMEでの演奏時間。
    ///
    /// 内包する値は時間（単位は秒）。
    PerformanceTime(u64),

    /// TIMEでの表示終了時刻。
    ///
    /// 内包する値は時刻（単位は秒）。
    DisplayEndTime(u64),

    /// RPC、文字繰り返し。
    ///
    /// 内包する値は`0..=63`の範囲である。
    RepeatCharacter(u8),

    /// SPL、アンダーライン終了およびモザイク分離終了。
    StopLining,

    /// STL、アンダーライン開始およびモザイク分離開始。
    StartLining,

    /// HLC、囲み制御。
    ///
    /// 内包する値は`0`なら囲み終了、`1..=15`なら囲み1～囲み15開始である。
    HighlightBlock(u8),

    /// CSIのSWF、書式選択（初期化）。
    ///
    /// 内包する値は書式の種類で、範囲は`0..=12`である。
    SetWritingFormatInit(u8),

    /// CSIのSWF、書式選択（書式設定）。
    ///
    /// 内包する値はそれぞれ以下の通りである。
    /// 1. 文字表示方向。
    /// 2. 字数・行数の単位となる文字サイズで、範囲は`0..=2`。
    /// 3. 一行の文字数。
    /// 4. 行数。
    SetWritingFormatDetails(bool, u8, u32, Option<u32>),

    /// CSIのCCC、合成制御でのOR合成開始。
    CompositeCharacterCompositionStartOr,
    /// CSIのCCC、合成制御でのAND合成開始。
    CompositeCharacterCompositionStartAnd,
    /// CSIのCCC、合成制御でのXOR合成開始。
    CompositeCharacterCompositionStartXor,
    /// CSIのCCC、合成制御での合成終了。
    CompositeCharacterCompositionEnd,

    /// CSIのRCS、ラスタ色制御。
    ///
    /// 内包する値はラスタ色で、`0..=15`の範囲である。
    RasterColorCommand(u8),

    /// CSIのACPS、動作位置座標指定。
    ///
    /// 内包する値は水平方向の座標と垂直方向の座標である。
    ActiveCoordinatePositionSet(u32, u32),

    /// CSIのSDF、表示構成ドット指定。
    ///
    /// 内包する値は水平方向のドット数と垂直方向のドット数である。
    SetDisplayFormat(u32, u32),

    /// CSIのSDP、表示位置指定。
    ///
    /// 内包する値は水平方向の座標と垂直方向の座標である。
    SetDisplayPosition(u32, u32),

    /// CSIのSSM、文字構成ドット指定。
    ///
    /// 内包する値は横方向のドット数と縦方向のドット数である。
    CharacterCompositionDotDesignation(u32, u32),

    /// CSIのSHS、字間隔指定。
    ///
    /// 内包する値は動作方向のドット数である。
    SetHorizontalSpacing(u32),

    /// CSIのSVS、行間隔指定。
    ///
    /// 内包する値は行方向のドット数である。
    SetVerticalSpacing(u32),

    /// CSIのGSM、文字変形。
    ///
    /// 内包する値は行方向の倍率×10と動作方向の倍率×10である。
    CharacterDeformation(u32, u32),

    /// CSIのGAA、着色区画。
    ///
    /// 内包する値は`true`なら全表示区画、`false`ならデザイン枠である。
    ColoringBlock(bool),

    /// CSIのSRC、ラスタ指定。
    ///
    /// 内包する値はそれぞれ以下の通りである。
    /// 1. スーパー表示の指定で範囲は`0..=3`。
    /// 2. ラスタ色のカラーマップアドレスで範囲は`0x00..=0xFF`。
    RasterColorDesignation(u8, u8),

    /// CSIのTCC、切替制御。
    ///
    /// 内包する値はそれぞれ以下の通りである。
    /// 1. 切替モード指定で範囲は`0..=9`。
    /// 2. 切替方向で範囲は`0..=3`
    /// 3. 切替時間指定（単位は0.1秒）。
    SwitchControl(u8, u8, u32),

    /// CSIのCFS、文字フォント設定。
    ///
    /// 内包する値はフォント指定である。
    CharacterFontSet(u32),

    /// CSIのORN、文字飾り指定の文字飾りなし。
    OrnamentControlClear,
    /// CSIのORN、文字飾り指定の縁取り。
    ///
    /// 内包する値は文字飾り色のカラーマップアドレスで範囲は`0x00..=0xFF`である。
    OrnamentControlHemming(u8),
    /// CSIのORN、文字飾り指定の影付き。
    ///
    /// 内包する値は文字飾り色のカラーマップアドレスで範囲は`0x00..=0xFF`である。
    OrnamentControlShade(u8),
    /// CSIのORN、文字飾り指定の中抜き。
    OrnamentControlHollow,

    /// CSIのMDF、字体指定の標準。
    FontStandard,
    /// CSIのMDF、字体指定の太字。
    FontBold,
    /// CSIのMDF、字体指定の斜体。
    FontSlated,
    /// CSIのMDF、字体指定の太字斜体。
    FontBoldSlated,

    /// CSIのXCS、外字代替符号列定義の定義開始。
    ExternalCharacterSetStart,
    /// CSIのXCS、外字代替符号列定義の定義終了。
    ExternalCharacterSetEnd,

    /// CSIのPRA、内蔵音再生。
    ///
    /// 内包する値は内蔵音指定である。
    BuiltinSoundReplay(u32),

    /// CSIのACS、代替符号列制御の代替元符号列開始。
    AlternativeCharacterSetStart,
    /// CSIのACS、代替符号列制御の代替元符号列終了。
    AlternativeCharacterSetEnd,
    /// CSIのACS、代替符号列制御の代替符号列（英数片仮名）定義開始。。
    AlternativeCharacterSetAlnumKataStart,
    /// CSIのACS、代替符号列制御の代替符号列（英数片仮名）定義終了。
    AlternativeCharacterSetAlnumKataEnd,
    /// CSIのACS、代替符号列制御の代替符号列（読上げ）定義開始。。
    AlternativeCharacterSetSpeechStart,
    /// CSIのACS、代替符号列制御の代替符号列（読上げ）定義終了。
    AlternativeCharacterSetSpeechEnd,

    /// CSIのUED、不可視データ埋め込み制御の符号列開始。
    EmbedInvisibleDataStart,
    /// CSIのUED、不可視データ埋め込み制御の符号列終了。
    EmbedInvisibleDataEnd,
    /// CSIのUED、不可視データ埋め込み制御のリンクする字幕表示文字列の開始。
    EmbedInvisibleDataLinkedCaptionStart,
    /// CSIのUED、不可視データ埋め込み制御のリンクする字幕表示文字列の終了。
    EmbedInvisibleDataLinkedCaptionEnd,

    /// CSIのSCS、後続符号列読み飛ばし制御。
    SkipCharacterSet,
}

/// 図形文字と図形化に関わる制御文字からなる、8単位符号の文字型。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphicChar {
    /// 一般的な図形文字。
    Generic(GenericChar),

    /// モザイク図形文字。
    Mosaic(MosaicChar),

    /// DRCS。
    Drcs(DrcsChar),

    /// APR、動作位置改行。
    ActivePositionReturn,

    /// SP、スペース。
    Space,

    /// SSZ・MSZ・NSZ・SZX、指定サイズ等。
    CharSize(CharSize),
}

impl From<GraphicChar> for AribChar {
    #[inline]
    fn from(value: GraphicChar) -> AribChar {
        match value {
            GraphicChar::Generic(c) => AribChar::Generic(c),
            GraphicChar::Mosaic(c) => AribChar::Mosaic(c),
            GraphicChar::Drcs(c) => AribChar::Drcs(c),
            GraphicChar::ActivePositionReturn => AribChar::ActivePositionReturn,
            GraphicChar::Space => AribChar::Space,
            GraphicChar::CharSize(s) => AribChar::CharSize(s),
        }
    }
}
