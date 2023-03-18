//! 8単位符号のデコード。

use std::slice;

use arrayvec::ArrayVec;

use super::char::{
    self, AribChar, CharSize, DrcsChar, GenericChar, GraphicChar, GraphicCode, MosaicChar,
    TimeControlMode,
};

/// 符号の指示。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Designator {
    /// G0に対する指示。
    G0 = 0,
    /// G1に対する指示。
    G1 = 1,
    /// G2に対する指示。
    G2 = 2,
    /// G3に対する指示。
    G3 = 3,
}

/// 文字符号集合。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GraphicSet {
    /// 漢字、2バイト符号。
    Kanji,
    /// 英数、1バイト符号。
    Alnum,
    /// 平仮名、1バイト符号。
    Hira,
    /// 片仮名、1バイト符号。
    Kata,
    /// モザイクA、1バイト符号。
    MosaicA,
    /// モザイクB、1バイト符号。
    MosaicB,
    /// モザイクC、1バイト符号。
    MosaicC,
    /// モザイクD、1バイト符号。
    MosaicD,
    /// プロポーショナル英数、1バイト符号。
    PropAlnum,
    /// プロポーショナル平仮名、1バイト符号。
    PropHira,
    /// プロポーショナル片仮名、1バイト符号。
    PropKata,
    /// JIS X 0201 片仮名、1バイト符号。
    JisXKata,
    /// JIS互換漢字1面、2バイト符号。
    JisKanjiPlane1,
    /// JIS互換漢字2面、2バイト符号。
    JisKanjiPlane2,
    /// 追加記号、2バイト符号。
    ExtraSymbols,
    /// DRCS-0、2バイト符号。
    Drcs0,
    /// DRCS-1、1バイト符号。
    Drcs1,
    /// DRCS-2、1バイト符号。
    Drcs2,
    /// DRCS-3、1バイト符号。
    Drcs3,
    /// DRCS-4、1バイト符号。
    Drcs4,
    /// DRCS-5、1バイト符号。
    Drcs5,
    /// DRCS-6、1バイト符号。
    Drcs6,
    /// DRCS-7、1バイト符号。
    Drcs7,
    /// DRCS-8、1バイト符号。
    Drcs8,
    /// DRCS-9、1バイト符号。
    Drcs9,
    /// DRCS-10、1バイト符号。
    Drcs10,
    /// DRCS-11、1バイト符号。
    Drcs11,
    /// DRCS-12、1バイト符号。
    Drcs12,
    /// DRCS-13、1バイト符号。
    Drcs13,
    /// DRCS-14、1バイト符号。
    Drcs14,
    /// DRCS-15、1バイト符号。
    Drcs15,
    /// マクロ、1バイト符号。
    Macro,
}

/// 8単位符号をデコードする際のオプション。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    /// 初期状態でG0～G3に指示する符号集合。
    pub graphic_sets: [GraphicSet; 4],

    /// 初期状態でGLに呼び出す符号集合。
    pub gl: Designator,

    /// 初期状態でGRに呼び出す符号集合。
    pub gr: Designator,
}

impl Options {
    /// 通常の符号列をデコードする際のオプション。
    pub const DEFAULT: Options = Options {
        graphic_sets: [
            GraphicSet::Kanji,
            GraphicSet::Alnum,
            GraphicSet::Hira,
            GraphicSet::Kata,
        ],
        gl: Designator::G0,
        gr: Designator::G2,
    };

    /// 字幕の符号列をデコードする際のオプション。
    pub const CAPTION: Options = Options {
        graphic_sets: [
            GraphicSet::Kanji,
            GraphicSet::Alnum,
            GraphicSet::Hira,
            GraphicSet::Macro,
        ],
        gl: Designator::G0,
        gr: Designator::G2,
    };

    /// ワンセグにおける字幕の符号列をデコードする際のオプション。
    pub const ONESEG_CAPTION: Options = Options {
        graphic_sets: [
            GraphicSet::Kanji,
            GraphicSet::Drcs1,
            GraphicSet::Hira,
            GraphicSet::Macro,
        ],
        gl: Designator::G1,
        gr: Designator::G0,
    };
}

impl Default for Options {
    fn default() -> Self {
        Options::DEFAULT
    }
}

/// 実行中のマクロ。
#[derive(Debug, Clone)]
struct CurrentMacro {
    /// マクロの番号。
    n: GraphicCode,
    /// マクロ内の位置。
    pos: usize,
}

/// マクロ符号集合。
// 常にマクロを保持すると無駄にスタックやヒープを消費してしまうので、
// MACRO符号でマクロが設定されるまでは静的変数`DEFAULT_MACROS`を参照する。
#[derive(Debug, Clone)]
struct Macros {
    macros: Option<Box<[Vec<u8>; 0x7E - 0x21 + 1]>>,
}

impl Macros {
    #[inline]
    pub fn new() -> Macros {
        Macros { macros: None }
    }

    /// `n`で指定された符号に定義されたマクロを取得する。
    #[inline]
    pub fn get(&self, n: GraphicCode) -> &[u8] {
        let n = (n.get() - 0x21) as usize;
        if let Some(ref macros) = self.macros {
            &*macros[n]
        } else {
            DEFAULT_MACROS[n]
        }
    }

    /// `n`で指定された符号にマクロを定義する。
    pub fn set(&mut self, n: GraphicCode, makro: &[u8]) {
        let n = (n.get() - 0x21) as usize;
        let macros = match self.macros {
            Some(ref mut macros) => macros,
            None => {
                if makro.is_empty() && DEFAULT_MACROS[n].is_empty() {
                    // 空の場所に空を突っ込むためにヒープ確保するのは避ける
                    return;
                }

                self.macros
                    .insert(crate::utils::boxed_array(|i| DEFAULT_MACROS[i].to_vec()))
            }
        };

        macros[n] = makro.to_vec();
    }
}

/// `Decoder`における読み取り結果。
enum ReadResult<T> {
    /// 復号できた文字。
    Char(T),

    /// 文字は復号できなかったが処理は続ける。
    Continue,

    /// EOFに到達したため処理を取りやめる。
    Eof,
}

// `std::convert::Infallible`でもいいけど`From<AribChar | GraphicChar> for Infallible`を露出させたくない気がする
enum Never {}

impl From<Never> for GraphicChar {
    #[inline]
    fn from(value: Never) -> GraphicChar {
        match value {}
    }
}

impl From<Never> for AribChar {
    #[inline]
    fn from(value: Never) -> AribChar {
        match value {}
    }
}

/// `Option<T>`の値が`None`であれば`return ReadResult::Eof`する。
macro_rules! try_opt {
    ($v:expr) => {
        match $v {
            Some(v) => v,
            None => return ReadResult::Eof,
        }
    };
}

/// `ReadResult<T>`の値が`Continue`でなければ値を`Option`にして`return`する。
macro_rules! try_rr {
    ($v:expr) => {
        match $v {
            ReadResult::Char(c) => return Some(c.into()),
            ReadResult::Eof => return None,
            ReadResult::Continue => {}
        }
    };
}

/// ARIB STD-B24の8単位符号をデコードする。
#[derive(Debug, Clone)]
pub(super) struct Decoder<'a> {
    iter: slice::Iter<'a, u8>,
    graphic_sets: [GraphicSet; 4],
    gl: Designator,
    gr: Designator,

    macros: Macros,
    current_macro: Option<CurrentMacro>,
}

impl<'a> Decoder<'a> {
    /// `otps`に従い`bytes`をデコードする`Decoder`を生成する。
    #[inline]
    pub fn new(bytes: &'a [u8], options: Options) -> Decoder<'a> {
        Decoder {
            iter: bytes.iter(),
            graphic_sets: options.graphic_sets,
            gl: options.gl,
            gr: options.gr,
            macros: Macros::new(),
            current_macro: None,
        }
    }

    /// `Decoder`で未処理の部分をスライスとして返す。
    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        self.iter.as_slice()
    }

    fn read_byte(&mut self) -> Option<u8> {
        if let Some(cm) = &mut self.current_macro {
            let makro = self.macros.get(cm.n);
            let b = makro[cm.pos];

            cm.pos += 1;
            if cm.pos == makro.len() {
                self.current_macro = None;
            }

            Some(b)
        } else {
            self.iter.next().copied()
        }
    }

    /// 長さが最低`n`があると分かっている場合に`n`個の符号をスキップする。
    fn skip(&mut self, n: usize) {
        if let Some(cm) = &mut self.current_macro {
            let makro = self.macros.get(cm.n);

            cm.pos += n;
            debug_assert!(cm.pos <= makro.len());
            if cm.pos == makro.len() {
                self.current_macro = None;
            }
        } else {
            let _r = self.iter.nth(n - 1);
            debug_assert!(_r.is_some());
        }
    }

    /// `n`個の符号をスキップする。
    fn try_skip(&mut self, n: usize) -> ReadResult<Never> {
        if let Some(cm) = &mut self.current_macro {
            let makro = self.macros.get(cm.n);

            cm.pos += n;
            if cm.pos > makro.len() {
                ReadResult::Eof
            } else if cm.pos == makro.len() {
                self.current_macro = None;
                ReadResult::Continue
            } else {
                ReadResult::Continue
            }
        } else {
            match self.iter.nth(n - 1) {
                Some(_) => ReadResult::Continue,
                None => ReadResult::Eof,
            }
        }
    }

    fn cur_bytes(&self) -> &[u8] {
        if let Some(cm) = &self.current_macro {
            &self.macros.get(cm.n)[cm.pos..]
        } else {
            self.iter.as_slice()
        }
    }

    /// 符号の指示。
    fn designate(&mut self, g: Designator, set: GraphicSet) {
        self.graphic_sets[g as usize] = set;
    }

    /// 符号の指示と図形符号から文字を得る。
    fn get_graphic(&mut self, g: Designator, c1: GraphicCode) -> ReadResult<GraphicChar> {
        macro_rules! c2 {
            () => {
                match self.read_byte() {
                    Some(c2 @ (0x21..=0x7E | 0xA1..=0xFE)) => GraphicCode::new(c2 & 0x7F),
                    // 不明な値は無視
                    Some(_) => return ReadResult::Continue,
                    None => return ReadResult::Eof,
                }
            };
        }

        let c = match self.graphic_sets[g as usize] {
            GraphicSet::Kanji => GraphicChar::Generic(GenericChar::Kanji(char::Kanji(c1, c2!()))),
            GraphicSet::Alnum => GraphicChar::Generic(GenericChar::Alnum(char::Alnum(c1))),
            GraphicSet::Hira => GraphicChar::Generic(GenericChar::Hira(char::Hira(c1))),
            GraphicSet::Kata => GraphicChar::Generic(GenericChar::Kata(char::Kata(c1))),
            GraphicSet::PropAlnum => {
                GraphicChar::Generic(GenericChar::PropAlnum(char::PropAlnum(c1)))
            }
            GraphicSet::PropHira => GraphicChar::Generic(GenericChar::PropHira(char::PropHira(c1))),
            GraphicSet::PropKata => GraphicChar::Generic(GenericChar::PropKata(char::PropKata(c1))),
            GraphicSet::JisXKata => GraphicChar::Generic(GenericChar::JisXKata(char::JisXKata(c1))),
            GraphicSet::JisKanjiPlane1 => {
                GraphicChar::Generic(GenericChar::JisKanjiPlane1(char::JisKanjiPlane1(c1, c2!())))
            }
            GraphicSet::JisKanjiPlane2 => {
                GraphicChar::Generic(GenericChar::JisKanjiPlane2(char::JisKanjiPlane2(c1, c2!())))
            }
            GraphicSet::ExtraSymbols => {
                GraphicChar::Generic(GenericChar::ExtraSymbols(char::ExtraSymbols(c1, c2!())))
            }

            GraphicSet::MosaicA => GraphicChar::Mosaic(MosaicChar::MosaicA(c1)),
            GraphicSet::MosaicB => GraphicChar::Mosaic(MosaicChar::MosaicB(c1)),
            GraphicSet::MosaicC => GraphicChar::Mosaic(MosaicChar::MosaicC(c1)),
            GraphicSet::MosaicD => GraphicChar::Mosaic(MosaicChar::MosaicD(c1)),

            GraphicSet::Drcs0 => GraphicChar::Drcs(DrcsChar::Drcs0(c1, c2!())),
            GraphicSet::Drcs1 => GraphicChar::Drcs(DrcsChar::Drcs1(c1)),
            GraphicSet::Drcs2 => GraphicChar::Drcs(DrcsChar::Drcs2(c1)),
            GraphicSet::Drcs3 => GraphicChar::Drcs(DrcsChar::Drcs3(c1)),
            GraphicSet::Drcs4 => GraphicChar::Drcs(DrcsChar::Drcs4(c1)),
            GraphicSet::Drcs5 => GraphicChar::Drcs(DrcsChar::Drcs5(c1)),
            GraphicSet::Drcs6 => GraphicChar::Drcs(DrcsChar::Drcs6(c1)),
            GraphicSet::Drcs7 => GraphicChar::Drcs(DrcsChar::Drcs7(c1)),
            GraphicSet::Drcs8 => GraphicChar::Drcs(DrcsChar::Drcs8(c1)),
            GraphicSet::Drcs9 => GraphicChar::Drcs(DrcsChar::Drcs9(c1)),
            GraphicSet::Drcs10 => GraphicChar::Drcs(DrcsChar::Drcs10(c1)),
            GraphicSet::Drcs11 => GraphicChar::Drcs(DrcsChar::Drcs11(c1)),
            GraphicSet::Drcs12 => GraphicChar::Drcs(DrcsChar::Drcs12(c1)),
            GraphicSet::Drcs13 => GraphicChar::Drcs(DrcsChar::Drcs13(c1)),
            GraphicSet::Drcs14 => GraphicChar::Drcs(DrcsChar::Drcs14(c1)),
            GraphicSet::Drcs15 => GraphicChar::Drcs(DrcsChar::Drcs15(c1)),

            GraphicSet::Macro => {
                // マクロ実行はネストできない、かつ空のマクロは設定しない
                if self.current_macro.is_none() && !self.macros.get(c1).is_empty() {
                    self.current_macro = Some(CurrentMacro { n: c1, pos: 0 });
                }
                return ReadResult::Continue;
            }
        };
        ReadResult::Char(c)
    }

    /// 符号集合と次の符号から文字を得る。
    fn read_graphic(&mut self, g: Designator) -> ReadResult<GraphicChar> {
        match self.read_byte() {
            Some(c1 @ (0x21..=0x7E | 0xA1..=0xFE)) => {
                self.get_graphic(g, GraphicCode::new(c1 & 0x7F))
            }
            Some(_) => ReadResult::Continue,
            None => ReadResult::Eof,
        }
    }

    /// エスケープを読み取る。
    fn read_esc(&mut self) -> ReadResult<Never> {
        fn invoke_to_gl(this: &mut Decoder, g: Designator) -> ReadResult<Never> {
            this.skip(1);
            this.gl = g;
            ReadResult::Continue
        }
        fn invoke_to_gr(this: &mut Decoder, g: Designator) -> ReadResult<Never> {
            this.skip(1);
            this.gr = g;
            ReadResult::Continue
        }
        fn designate(this: &mut Decoder, read: usize, g: u8, set: GraphicSet) -> ReadResult<Never> {
            debug_assert!((0x28..=0x2B).contains(&g));
            this.skip(read);

            let g = match g - 0x28 {
                0 => Designator::G0,
                1 => Designator::G1,
                2 => Designator::G2,
                3 => Designator::G3,
                _ => unreachable!(),
            };
            this.designate(g, set);
            ReadResult::Continue
        }

        match *self.cur_bytes() {
            [] => return ReadResult::Eof,

            // 符号の呼び出し

            // LS2
            [0x6E, ..] => invoke_to_gl(self, Designator::G2),
            // LS3
            [0x6F, ..] => invoke_to_gl(self, Designator::G3),
            // LS1R
            [0x7E, ..] => invoke_to_gr(self, Designator::G1),
            // LS2R
            [0x7D, ..] => invoke_to_gr(self, Designator::G2),
            // LS3R
            [0x7C, ..] => invoke_to_gr(self, Designator::G3),

            // 符号の指示

            // 1バイトGセット
            [g @ 0x28..=0x2B, 0x4A, ..] => designate(self, 2, g, GraphicSet::Alnum),
            [g @ 0x28..=0x2B, 0x30, ..] => designate(self, 2, g, GraphicSet::Hira),
            [g @ 0x28..=0x2B, 0x31, ..] => designate(self, 2, g, GraphicSet::Kata),
            [g @ 0x28..=0x2B, 0x32, ..] => designate(self, 2, g, GraphicSet::MosaicA),
            [g @ 0x28..=0x2B, 0x33, ..] => designate(self, 2, g, GraphicSet::MosaicB),
            [g @ 0x28..=0x2B, 0x34, ..] => designate(self, 2, g, GraphicSet::MosaicC),
            [g @ 0x28..=0x2B, 0x35, ..] => designate(self, 2, g, GraphicSet::MosaicD),
            [g @ 0x28..=0x2B, 0x36, ..] => designate(self, 2, g, GraphicSet::PropAlnum),
            [g @ 0x28..=0x2B, 0x37, ..] => designate(self, 2, g, GraphicSet::PropHira),
            [g @ 0x28..=0x2B, 0x38, ..] => designate(self, 2, g, GraphicSet::PropKata),
            [g @ 0x28..=0x2B, 0x49, ..] => designate(self, 2, g, GraphicSet::JisXKata),

            // 2バイトGセット
            [0x24, 0x42, ..] => designate(self, 2, 0x28, GraphicSet::Kanji),
            [0x24, 0x39, ..] => designate(self, 2, 0x28, GraphicSet::JisKanjiPlane1),
            [0x24, 0x3A, ..] => designate(self, 2, 0x28, GraphicSet::JisKanjiPlane2),
            [0x24, 0x3B, ..] => designate(self, 2, 0x28, GraphicSet::ExtraSymbols),
            [0x24, g @ 0x29..=0x2B, 0x42, ..] => designate(self, 3, g, GraphicSet::Kanji),
            [0x24, g @ 0x29..=0x2B, 0x39, ..] => designate(self, 3, g, GraphicSet::JisKanjiPlane1),
            [0x24, g @ 0x29..=0x2B, 0x3A, ..] => designate(self, 3, g, GraphicSet::JisKanjiPlane2),
            [0x24, g @ 0x29..=0x2B, 0x3B, ..] => designate(self, 3, g, GraphicSet::ExtraSymbols),

            // 1バイトDRCS
            [g @ 0x28..=0x2B, 0x20, 0x41, ..] => designate(self, 3, g, GraphicSet::Drcs1),
            [g @ 0x28..=0x2B, 0x20, 0x42, ..] => designate(self, 3, g, GraphicSet::Drcs2),
            [g @ 0x28..=0x2B, 0x20, 0x43, ..] => designate(self, 3, g, GraphicSet::Drcs3),
            [g @ 0x28..=0x2B, 0x20, 0x44, ..] => designate(self, 3, g, GraphicSet::Drcs4),
            [g @ 0x28..=0x2B, 0x20, 0x45, ..] => designate(self, 3, g, GraphicSet::Drcs5),
            [g @ 0x28..=0x2B, 0x20, 0x46, ..] => designate(self, 3, g, GraphicSet::Drcs6),
            [g @ 0x28..=0x2B, 0x20, 0x47, ..] => designate(self, 3, g, GraphicSet::Drcs7),
            [g @ 0x28..=0x2B, 0x20, 0x48, ..] => designate(self, 3, g, GraphicSet::Drcs8),
            [g @ 0x28..=0x2B, 0x20, 0x49, ..] => designate(self, 3, g, GraphicSet::Drcs9),
            [g @ 0x28..=0x2B, 0x20, 0x4A, ..] => designate(self, 3, g, GraphicSet::Drcs10),
            [g @ 0x28..=0x2B, 0x20, 0x4B, ..] => designate(self, 3, g, GraphicSet::Drcs11),
            [g @ 0x28..=0x2B, 0x20, 0x4C, ..] => designate(self, 3, g, GraphicSet::Drcs12),
            [g @ 0x28..=0x2B, 0x20, 0x4D, ..] => designate(self, 3, g, GraphicSet::Drcs13),
            [g @ 0x28..=0x2B, 0x20, 0x4E, ..] => designate(self, 3, g, GraphicSet::Drcs14),
            [g @ 0x28..=0x2B, 0x20, 0x4F, ..] => designate(self, 3, g, GraphicSet::Drcs15),
            [g @ 0x28..=0x2B, 0x20, 0x70, ..] => designate(self, 3, g, GraphicSet::Macro),

            // 2バイトDRCS
            [0x24, g @ 0x28..=0x2B, 0x20, 0x40, ..] => designate(self, 4, g, GraphicSet::Drcs0),

            // 変な値は無視
            [0x24, 0x28..=0x2B, 0x20, ..] => self.try_skip(4),
            [0x28..=0x2B, 0x20, ..] => self.try_skip(3),
            [0x24, 0x29..=0x2B, ..] => self.try_skip(3),
            [0x24, ..] => self.try_skip(2),
            [0x28..=0x2B, ..] => self.try_skip(2),
            [_, ..] => {
                self.skip(1);
                ReadResult::Continue
            }
        }
    }

    fn read_macro(&mut self) -> ReadResult<Never> {
        // MACRO符号入りのデータを見たことないので分かるようにしておく
        log::debug!("read_macro");

        debug_assert!(self.current_macro.is_none());
        // マクロ実行中にマクロが出てくることはないのでself.iterを直接使う

        let run = match *try_opt!(self.iter.next()) {
            0x40 => false,
            0x41 => true,
            _ => return ReadResult::Continue,
        };
        let mc @ 0x21..=0x7E = *try_opt!(self.iter.next()) else {
            return ReadResult::Continue;
        };
        let mc = GraphicCode::new(mc);

        fn do_skip(this: &mut Decoder, n: usize) {
            let _r = this.iter.nth(n - 1);
            debug_assert!(_r.is_some());
        }
        macro_rules! skip {
            // 途中でEOFになるかもしれない場合
            (try $n:expr) => {
                match self.iter.nth($n - 1) {
                    None => return ReadResult::Eof,
                    Some(_) => {}
                }
            };
            // 長さが分かっている場合
            ($n:expr) => {
                // インライン化するかどうか最適化に任せる
                do_skip(self, $n)
            };
        }

        let start = self.iter.as_slice();
        loop {
            match *self.iter.as_slice() {
                // マクロ終了前に符号列が終了した
                [] => return ReadResult::Eof,

                // MACRO -> マクロ終了
                [0x95, 0x4F, ..] => break,
                // MACRO -> マクロ開始：マクロはネストできない
                [0x95, 0x40 | 0x41, ..] => {
                    skip!(2);
                    return ReadResult::Continue;
                }
                // MACRO -> 不明
                [0x95, _, ..] => skip!(2),

                // GL/GR：場合によっては2文字目があるけど次のループでスキップされる
                [0x21..=0x7E | 0xA1..=0xFE, ..] => skip!(1),

                // C0

                // SS2/SS3：場合によっては2文字目があるけど次のループでスキップされる
                [0x19 | 0x1D, ..] => skip!(try 2),
                // PAPF
                [0x16, ..] => skip!(try 2),
                // APS
                [0x1C, ..] => skip!(try 3),

                // ESC -> LS2/LS3/LS1R/LS2R/LS3R
                [0x1B, 0x6E | 0x6F | 0x7E | 0x7D | 0x7C, ..] => skip!(2),
                // ESC -> 符号の指示：2バイトDRCS
                [0x1B, 0x24, 0x28..=0x2B, 0x20, ..] => skip!(try 5),
                // ESC -> 符号の指示：1バイトDRCS
                [0x1B, 0x28..=0x2B, 0x20, ..] => skip!(try 4),
                // ESC -> 符号の指示：2バイトGセット
                [0x1B, 0x24, 0x29..=0x2B, ..] => skip!(try 4),
                [0x1B, 0x24, ..] => skip!(try 3),
                // ESC -> 符号の指示：1バイトGセット
                [0x1B, 0x28..=0x2B, ..] => skip!(try 3),
                // ESC -> 不明
                [0x1B, ..] => skip!(try 2),

                // C1

                // COL
                [0x90, 0x20, ..] => skip!(try 3),
                [0x90, ..] => skip!(try 2),

                // CDC
                [0x92, 0x20, ..] => skip!(try 3),
                [0x92, ..] => skip!(try 2),

                // SZX/FLC/POL/WMM/HLC/RPC（パラメータ1つ）
                [0x8B | 0x91 | 0x93 | 0x94 | 0x97 | 0x98, ..] => skip!(try 2),

                // CSI
                [0x9B, ..] => loop {
                    skip!(1);
                    match *try_opt!(self.iter.next()) {
                        // 中間文字
                        0x20 => {
                            // 終端文字
                            skip!(try 1);
                            break;
                        }
                        // PLD/PLU/SCS（パラメータなし）
                        0x5B | 0x5C | 0x6F => break,
                        _ => {}
                    }
                },
                // TIME -> STM/DTM/OTM/PTM/ETM
                [0x9D, 0x29, ..] => {
                    skip!(2);
                    // 中間文字までスキップ
                    while *try_opt!(self.iter.next()) != 0x20 {}
                    // 終端文字
                    skip!(try 1);
                }
                // TIME -> 処理待ち/時間制御モード/不明
                [0x9D, ..] => skip!(try 3),

                [_, ..] => skip!(1),
            }
        }

        let len = start.len() - self.iter.as_slice().len();
        skip!(2); // マクロ終了の分

        self.macros.set(mc, &start[..len]);

        if run && len > 0 {
            self.current_macro = Some(CurrentMacro { n: mc, pos: 0 });
        }

        ReadResult::Continue
    }

    fn read_csi(&mut self) -> ReadResult<AribChar> {
        fn skip_to_the_end(this: &mut Decoder) -> ReadResult<AribChar> {
            // 中間文字まで飛ばす
            while try_opt!(this.read_byte()) != 0x20 {}

            // 終端文字
            let _: u8 = try_opt!(this.read_byte());
            ReadResult::Continue
        }

        // パラメータは最大4つ。またパラメータの値は10桁も行かなさそうなのでu32で十分
        let mut params = ArrayVec::<u32, 4>::new();
        let mut param = 0;
        let f = loop {
            match try_opt!(self.read_byte()) {
                p @ 0x30..=0x39 => {
                    param = param * 10 + (p - 0x30) as u32;
                }

                0x3B => {
                    // パラメータ過多の時はCSI全体を無視
                    if params.try_push(param).is_err() {
                        log::trace!("CSI: too many params");
                        return skip_to_the_end(self);
                    }
                    param = 0;
                }

                // 中間文字
                0x20 => {
                    // パラメータ過多の時はCSI全体を無視
                    if params.try_push(param).is_err() {
                        log::trace!("CSI: too many params");
                        return skip_to_the_end(self);
                    }

                    break try_opt!(self.read_byte());
                }
                // PLD/PLU/SCS（パラメータなし）
                f @ (0x5B | 0x5C | 0x6F) => break f,

                // 不明な値が来たらCSI全体を無視
                b => {
                    log::trace!("CSI: unknown byte {}", b);
                    return skip_to_the_end(self);
                }
            }
        };

        match (f, params.as_slice()) {
            // SWF
            (0x53, &[p1 @ 0..=12]) => ReadResult::Char(AribChar::SetWritingFormatInit(p1 as u8)),
            (0x53, &[p1, p2 @ 0..=2, p3]) => ReadResult::Char(AribChar::SetWritingFormatDetails(
                p1 == 8,
                p2 as u8,
                p3,
                None,
            )),
            (0x53, &[p1, p2 @ 0..=2, p3, p4]) => ReadResult::Char(
                AribChar::SetWritingFormatDetails(p1 == 8, p2 as u8, p3, Some(p4)),
            ),

            // CCC
            (0x54, &[2]) => ReadResult::Char(AribChar::CompositeCharacterCompositionStartOr),
            (0x54, &[3]) => ReadResult::Char(AribChar::CompositeCharacterCompositionStartAnd),
            (0x54, &[4]) => ReadResult::Char(AribChar::CompositeCharacterCompositionStartXor),
            (0x54, &[0]) => ReadResult::Char(AribChar::CompositeCharacterCompositionEnd),

            // RCS
            (0x6E, &[p1 @ 0..=15]) => ReadResult::Char(AribChar::RasterColorCommand(p1 as u8)),

            // ACPS
            (0x61, &[p1, p2]) => ReadResult::Char(AribChar::ActiveCoordinatePositionSet(p1, p2)),

            // SDF
            (0x56, &[p1, p2]) => ReadResult::Char(AribChar::SetDisplayFormat(p1, p2)),

            // SDP
            (0x5F, &[p1, p2]) => ReadResult::Char(AribChar::SetDisplayPosition(p1, p2)),

            // SSM
            (0x57, &[p1, p2]) => {
                ReadResult::Char(AribChar::CharacterCompositionDotDesignation(p1, p2))
            }

            // PLD
            (0x5B, &[]) => {
                log::trace!("deprecated PLD");
                ReadResult::Continue
            }

            // PLU
            (0x5C, &[]) => {
                log::trace!("deprecated PLU");
                ReadResult::Continue
            }

            // SHS
            (0x58, &[p1]) => ReadResult::Char(AribChar::SetHorizontalSpacing(p1)),

            // SVS
            (0x59, &[p1]) => ReadResult::Char(AribChar::SetVerticalSpacing(p1)),

            // GSM
            (0x42, &[p1, p2]) => ReadResult::Char(AribChar::CharacterDeformation(p1, p2)),

            // GAA
            (0x5D, &[p1 @ (0 | 1)]) => ReadResult::Char(AribChar::ColoringBlock(p1 == 0)),

            // SRC
            (0x5E, &[p1 @ (0..=3), p2]) => {
                let p2 = ((((p2 / 100) & 0xF) as u8) << 4) | (((p2 % 100) & 0x0F) as u8);
                ReadResult::Char(AribChar::RasterColorDesignation(p1 as u8, p2))
            }

            // TCC
            (0x62, &[p1 @ 0..=9, p2 @ 0..=3, p3]) => {
                ReadResult::Char(AribChar::SwitchControl(p1 as u8, p2 as u8, p3))
            }

            // CFS
            (0x65, &[p1]) => ReadResult::Char(AribChar::CharacterFontSet(p1)),

            // ORN
            (0x63, &[0] | &[0, _]) => ReadResult::Char(AribChar::OrnamentControlClear),
            (0x63, &[1, p2]) => {
                let p2 = ((((p2 / 100) & 0xF) as u8) << 4) | (((p2 % 100) & 0x0F) as u8);
                ReadResult::Char(AribChar::OrnamentControlHemming(p2))
            }
            (0x63, &[2, p2]) => {
                let p2 = ((((p2 / 100) & 0xF) as u8) << 4) | (((p2 % 100) & 0x0F) as u8);
                ReadResult::Char(AribChar::OrnamentControlShade(p2))
            }
            (0x63, &[3] | &[3, _]) => ReadResult::Char(AribChar::OrnamentControlHollow),

            // MDF
            (0x64, &[0]) => ReadResult::Char(AribChar::FontStandard),
            (0x64, &[1]) => ReadResult::Char(AribChar::FontBold),
            (0x64, &[2]) => ReadResult::Char(AribChar::FontSlated),
            (0x64, &[3]) => ReadResult::Char(AribChar::FontBoldSlated),

            // XCS
            (0x66, &[0]) => ReadResult::Char(AribChar::ExternalCharacterSetStart),
            (0x66, &[1]) => ReadResult::Char(AribChar::ExternalCharacterSetEnd),

            // PRA
            (0x68, &[p1]) => ReadResult::Char(AribChar::BuiltinSoundReplay(p1)),

            // ACS
            (0x69, &[0]) => ReadResult::Char(AribChar::AlternativeCharacterSetStart),
            (0x69, &[1]) => ReadResult::Char(AribChar::AlternativeCharacterSetEnd),
            (0x69, &[2]) => ReadResult::Char(AribChar::AlternativeCharacterSetAlnumKataStart),
            (0x69, &[3]) => ReadResult::Char(AribChar::AlternativeCharacterSetAlnumKataEnd),
            (0x69, &[4]) => ReadResult::Char(AribChar::AlternativeCharacterSetSpeechStart),
            (0x69, &[5]) => ReadResult::Char(AribChar::AlternativeCharacterSetSpeechEnd),

            // UED
            (0x6A, &[0]) => ReadResult::Char(AribChar::EmbedInvisibleDataStart),
            (0x6A, &[1]) => ReadResult::Char(AribChar::EmbedInvisibleDataEnd),
            (0x6A, &[2]) => ReadResult::Char(AribChar::EmbedInvisibleDataLinkedCaptionStart),
            (0x6A, &[3]) => ReadResult::Char(AribChar::EmbedInvisibleDataLinkedCaptionEnd),

            // SCS
            (0x6F, &[]) => ReadResult::Char(AribChar::SkipCharacterSet),

            // 不明なものは無視
            (f, _) => {
                log::trace!("unknown CSI: {:02X}={:?}", f, params);
                ReadResult::Continue
            }
        }
    }

    fn read_time(&mut self) -> ReadResult<AribChar> {
        match try_opt!(self.read_byte()) {
            0x20 => ReadResult::Char(AribChar::WaitForProcess(try_opt!(self.read_byte()) & 0x3F)),
            0x28 => match try_opt!(self.read_byte()) {
                0x40 => ReadResult::Char(AribChar::TimeControlMode(TimeControlMode::Free)),
                0x41 => ReadResult::Char(AribChar::TimeControlMode(TimeControlMode::RealTime)),
                0x42 => ReadResult::Char(AribChar::TimeControlMode(TimeControlMode::OffsetTime)),
                0x43 => ReadResult::Char(AribChar::TimeControlMode(TimeControlMode::Reserved)),
                p2 => {
                    log::trace!("unknown TIME: p1=28, p2={:02X}", p2);
                    ReadResult::Continue
                }
            },
            0x29 => {
                fn skip_to_the_end(this: &mut Decoder) -> ReadResult<AribChar> {
                    // 中間文字まで飛ばす
                    while try_opt!(this.read_byte()) != 0x20 {}

                    // 終端文字
                    let _: u8 = try_opt!(this.read_byte());
                    ReadResult::Continue
                }

                // パラメータは最大4つ。またパラメータの値は10桁も行かなさそうなのでu32で十分
                let mut params = ArrayVec::<u32, 4>::new();
                let mut param = 0;
                let f = loop {
                    match try_opt!(self.read_byte()) {
                        p @ 0x30..=0x39 => {
                            param = param * 10 + (p - 0x30) as u32;
                        }

                        0x3B => {
                            // パラメータ過多の時はTIME全体を無視
                            if params.try_push(param).is_err() {
                                log::trace!("TIME: too many params");
                                return skip_to_the_end(self);
                            }
                            param = 0;
                        }

                        // 中間文字
                        0x20 => {
                            // パラメータ過多の時はTIME全体を無視
                            if params.try_push(param).is_err() {
                                log::trace!("TIME: too many params");
                                return skip_to_the_end(self);
                            }

                            break try_opt!(self.read_byte());
                        }

                        // 不明な値が来たらTIME全体を無視
                        b => {
                            log::trace!("TIME: unknown byte {}", b);
                            return skip_to_the_end(self);
                        }
                    }
                };

                match (f, params.as_slice()) {
                    (0x40, &[hour, minute, second, millisecond]) => {
                        ReadResult::Char(AribChar::PresentationStartPlaybackTime(
                            (hour as u64) * (60 * 60 * 1000)
                                + (minute as u64) * (60 * 1000)
                                + (second as u64) * 1000
                                + (millisecond as u64),
                        ))
                    }
                    (0x41, &[hour, minute, second, millisecond]) => {
                        ReadResult::Char(AribChar::OffsetTime(
                            (hour as u64) * (60 * 60 * 1000)
                                + (minute as u64) * (60 * 1000)
                                + (second as u64) * 1000
                                + (millisecond as u64),
                        ))
                    }
                    (0x42, &[hour, minute, second, ..]) => {
                        ReadResult::Char(AribChar::PerformanceTime(
                            (hour as u64) * (60 * 60) + (minute as u64) * 60 + (second as u64),
                        ))
                    }
                    (0x43, &[hour, minute, second, millisecond]) => {
                        ReadResult::Char(AribChar::DisplayEndTime(
                            (hour as u64) * (60 * 60 * 1000)
                                + (minute as u64) * (60 * 1000)
                                + (second as u64) * 1000
                                + (millisecond as u64),
                        ))
                    }
                    (_, _) => {
                        log::trace!("unknown TIME: p1=29, f={:02X}, params={:?}", f, params);
                        ReadResult::Continue
                    }
                }
            }
            p1 => {
                log::trace!("unknown TIME: p1={:02X}", p1);
                ReadResult::Continue
            }
        }
    }

    /// 次の文字を得る。
    pub fn next_char(&mut self) -> Option<AribChar> {
        loop {
            match self.read_byte()? {
                // GL: 0x21..=0x7E
                c1 @ 0x21..=0x7E => try_rr!(self.get_graphic(self.gl, GraphicCode::new(c1))),

                // GR: 0xA1..=0xFE
                c1 @ 0xA1..=0xFE => try_rr!(self.get_graphic(self.gr, GraphicCode::new(c1 & 0x7F))),

                // C0: 0x00..=0x20

                // SS2
                0x19 => try_rr!(self.read_graphic(Designator::G2)),

                // SS3
                0x1D => try_rr!(self.read_graphic(Designator::G3)),

                // LS0
                0x0F => self.gl = Designator::G0,

                // LS1
                0x0E => self.gl = Designator::G1,

                // ESC
                0x1B => try_rr!(self.read_esc()),

                // NUL
                0x00 => break Some(AribChar::Null),

                // SP
                0x20 => break Some(AribChar::Space),

                // BEL
                0x07 => log::trace!("deprecated BEL"),

                // APB
                0x08 => break Some(AribChar::ActivePositionBackward),

                // APF
                0x09 => break Some(AribChar::ActivePositionForward),

                // APD
                0x0A => break Some(AribChar::ActivePositionDown),

                // APU
                0x0B => break Some(AribChar::ActivePositionUp),

                // CS
                0x0C => break Some(AribChar::ClearScreen),

                // APR
                0x0D => break Some(AribChar::ActivePositionReturn),

                // PAPF
                0x16 => {
                    let p1 = self.read_byte()? & 0x3F;
                    break Some(AribChar::ParameterizedActivePositionForward(p1));
                }

                // CAN
                0x18 => log::trace!("deprecated CAN"),

                // APS
                0x1C => {
                    let p1 = self.read_byte()? & 0x3F;
                    let p2 = self.read_byte()? & 0x3F;
                    break Some(AribChar::ActivePositionSet(p1, p2));
                }

                // RS
                0x1E => break Some(AribChar::RecordSeparator),

                // US
                0x1F => break Some(AribChar::UnitSeparator),

                // C1: 0x7F..=0xA0

                // DEL
                0x7F => break Some(AribChar::Delete),

                // BKF..=WHF
                c @ 0x80..=0x87 => break Some(AribChar::ColorForeground(c & 0x07)),

                // COL
                0x90 => match self.read_byte()? {
                    p1 @ 0x48..=0x4F => break Some(AribChar::ColorForeground(p1 & 0x0F)),
                    p1 @ 0x50..=0x5F => break Some(AribChar::ColorBackground(p1 & 0x0F)),
                    p1 @ 0x60..=0x6F => break Some(AribChar::ColorHalfForeground(p1 & 0x0F)),
                    p1 @ 0x70..=0x7F => break Some(AribChar::ColorHalfBackground(p1 & 0x0F)),
                    0x20 => {
                        let p2 = self.read_byte()? & 0x0F;
                        break Some(AribChar::ColorPalette(p2));
                    }
                    b => log::trace!("unknown COL: p1={:02X}", b),
                },

                // SSZ
                0x88 => break Some(AribChar::CharSize(CharSize::Small)),
                // MSZ
                0x89 => break Some(AribChar::CharSize(CharSize::Medium)),
                // NSZ
                0x8A => break Some(AribChar::CharSize(CharSize::Normal)),
                // SZX
                0x8B => match self.read_byte()? {
                    0x60 => break Some(AribChar::CharSize(CharSize::Micro)),
                    0x41 => break Some(AribChar::CharSize(CharSize::HighW)),
                    0x44 => break Some(AribChar::CharSize(CharSize::WidthW)),
                    0x45 => break Some(AribChar::CharSize(CharSize::SizeW)),
                    0x6B => break Some(AribChar::CharSize(CharSize::Special1)),
                    0x64 => break Some(AribChar::CharSize(CharSize::Special2)),
                    b => log::trace!("unknown SZX: p1={:02X}", b),
                },

                // MACRO
                0x95 => try_rr!(self.read_macro()),

                // FLC
                0x91 => match self.read_byte()? {
                    0x40 => break Some(AribChar::FlushingControlStartNormal),
                    0x47 => break Some(AribChar::FlushingControlStartInverted),
                    0x4F => break Some(AribChar::FlushingControlStop),
                    b => log::trace!("unknown FLC: p1={:02X}", b),
                },
                // CDC：非使用
                0x92 => match self.read_byte()? {
                    0x20 => {
                        let b = self.read_byte()?;
                        log::trace!("deprecated CDC: p1=20, p2={:02X}", b);
                    }
                    b => log::trace!("deprecated CDC: p1={:02X}", b),
                },
                // POL
                0x93 => match self.read_byte()? {
                    0x40 => break Some(AribChar::PatternPolarityNormal),
                    0x41 => break Some(AribChar::PatternPolarityInverted1),
                    0x42 => break Some(AribChar::PatternPolarityInverted2),
                    b => log::trace!("unknown POL: p1={:02X}", b),
                },
                // WMM
                0x94 => match self.read_byte()? {
                    0x40 => break Some(AribChar::WritingModeBoth),
                    0x44 => break Some(AribChar::WritingModeForeground),
                    0x45 => break Some(AribChar::WritingModeBackground),
                    b => log::trace!("unknown WMM: p1={:02X}", b),
                },

                // HLC
                0x97 => break Some(AribChar::HighlightBlock(self.read_byte()? & 0x0F)),

                // RPC
                0x98 => break Some(AribChar::RepeatCharacter(self.read_byte()? & 0x3F)),

                // SPL
                0x99 => break Some(AribChar::StopLining),

                // STL
                0x9A => break Some(AribChar::StartLining),

                // CSI
                0x9B => try_rr!(self.read_csi()),

                // TIME
                0x9D => try_rr!(self.read_time()),

                // 未知
                b => log::trace!("unknown arib char: {:02X}", b),
            }
        }
    }

    /// 次の図形化可能な文字を得る。
    pub fn next_graphic(&mut self) -> Option<GraphicChar> {
        loop {
            match self.read_byte()? {
                // GL: 0x21..=0x7E
                c1 @ 0x21..=0x7E => try_rr!(self.get_graphic(self.gl, GraphicCode::new(c1))),

                // GR: 0xA1..=0xFE
                c1 @ 0xA1..=0xFE => try_rr!(self.get_graphic(self.gr, GraphicCode::new(c1 & 0x7F))),

                // C0: 0x00..=0x20

                // SS2
                0x19 => try_rr!(self.read_graphic(Designator::G2)),

                // SS3
                0x1D => try_rr!(self.read_graphic(Designator::G3)),

                // LS0
                0x0F => self.gl = Designator::G0,

                // LS1
                0x0E => self.gl = Designator::G1,

                // ESC
                0x1B => try_rr!(self.read_esc()),

                // SP
                0x20 => break Some(GraphicChar::Space),

                // APR
                0x0D => break Some(GraphicChar::ActivePositionReturn),

                // PAPF（パラメータ1つ）
                0x16 => try_rr!(self.try_skip(1)),

                // APS（パラメータ2つ）
                0x1C => try_rr!(self.try_skip(2)),

                // C1: 0x7F..=0xA0

                // COL
                0x90 => match self.read_byte()? {
                    0x20 => try_rr!(self.try_skip(1)),
                    _ => {}
                },
                // CDC
                0x92 => match self.read_byte()? {
                    0x20 => try_rr!(self.try_skip(1)),
                    _ => {}
                },

                // FLC/POL/WMM/HLC/RPC（パラメータ1つ）
                0x91 | 0x93 | 0x94 | 0x97 | 0x98 => try_rr!(self.try_skip(1)),

                // SSZ
                0x88 => break Some(GraphicChar::CharSize(CharSize::Small)),
                // MSZ
                0x89 => break Some(GraphicChar::CharSize(CharSize::Medium)),
                // NSZ
                0x8A => break Some(GraphicChar::CharSize(CharSize::Normal)),
                // SZX
                0x8B => match self.read_byte()? {
                    0x60 => break Some(GraphicChar::CharSize(CharSize::Micro)),
                    0x41 => break Some(GraphicChar::CharSize(CharSize::HighW)),
                    0x44 => break Some(GraphicChar::CharSize(CharSize::WidthW)),
                    0x45 => break Some(GraphicChar::CharSize(CharSize::SizeW)),
                    0x6B => break Some(GraphicChar::CharSize(CharSize::Special1)),
                    0x64 => break Some(GraphicChar::CharSize(CharSize::Special2)),
                    b => log::trace!("unknown SZX: p1={:02X}", b),
                },

                // MACRO
                0x95 => try_rr!(self.read_macro()),

                // CSI
                0x9B => loop {
                    match self.read_byte()? {
                        // 中間文字
                        0x20 => {
                            // 終端文字
                            try_rr!(self.try_skip(1));
                            break;
                        }
                        // PLD/PLU/SCS（パラメータなし）
                        0x5B | 0x5C | 0x6F => break,
                        _ => {}
                    }
                },
                // TIME
                0x9D => match self.read_byte()? {
                    0x20 | 0x28 => try_rr!(self.try_skip(1)),
                    0x29 => {
                        // 中間文字までスキップ
                        while self.read_byte()? != 0x20 {}
                        // 終端文字
                        try_rr!(self.try_skip(1));
                    }
                    _ => {}
                },

                // パラメータを取らない制御符号または未知
                _ => {}
            }
        }
    }
}

static DEFAULT_MACROS: [&[u8]; 0x7E - 0x21 + 1] = [
    // 0x21..=0x5F
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    // 0x60..=0x6F
    b"\x1B\x24\x42\x1B\x29\x4A\x1B\x2A\x30\x1B\x2B\x20\x70\x0F\x1B\x7D",
    b"\x1B\x24\x42\x1B\x29\x31\x1B\x2A\x30\x1B\x2B\x20\x70\x0F\x1B\x7D",
    b"\x1B\x24\x42\x1B\x29\x20\x41\x1B\x2A\x30\x1B\x2B\x20\x70\x0F\x1B\x7D",
    b"\x1B\x28\x32\x1B\x29\x34\x1B\x2A\x35\x1B\x2B\x20\x70\x0F\x1B\x7D",
    b"\x1B\x28\x32\x1B\x29\x33\x1B\x2A\x35\x1B\x2B\x20\x70\x0F\x1B\x7D",
    b"\x1B\x28\x32\x1B\x29\x20\x41\x1B\x2A\x35\x1B\x2B\x20\x70\x0F\x1B\x7D",
    b"\x1B\x28\x20\x41\x1B\x29\x20\x42\x1B\x2A\x20\x43\x1B\x2B\x20\x70\x0F\x1B\x7D",
    b"\x1B\x28\x20\x44\x1B\x29\x20\x45\x1B\x2A\x20\x46\x1B\x2B\x20\x70\x0F\x1B\x7D",
    b"\x1B\x28\x20\x47\x1B\x29\x20\x48\x1B\x2A\x20\x49\x1B\x2B\x20\x70\x0F\x1B\x7D",
    b"\x1B\x28\x20\x4A\x1B\x29\x20\x4B\x1B\x2A\x20\x4C\x1B\x2B\x20\x70\x0F\x1B\x7D",
    b"\x1B\x28\x20\x4D\x1B\x29\x20\x4E\x1B\x2A\x20\x4F\x1B\x2B\x20\x70\x0F\x1B\x7D",
    b"\x1B\x24\x42\x1B\x29\x20\x42\x1B\x2A\x30\x1B\x2B\x20\x70\x0F\x1B\x7D",
    b"\x1B\x24\x42\x1B\x29\x20\x43\x1B\x2A\x30\x1B\x2B\x20\x70\x0F\x1B\x7D",
    b"\x1B\x24\x42\x1B\x29\x20\x44\x1B\x2A\x30\x1B\x2B\x20\x70\x0F\x1B\x7D",
    b"\x1B\x28\x31\x1B\x29\x30\x1B\x2A\x4A\x1B\x2B\x20\x70\x0F\x1B\x7D",
    b"\x1B\x28\x4A\x1B\x29\x32\x1B\x2A\x20\x41\x1B\x2B\x20\x70\x0F\x1B\x7D",
    // 0x70..=0x7E
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
    b"",
];
