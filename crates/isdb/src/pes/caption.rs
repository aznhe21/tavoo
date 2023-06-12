//! ARIB STD-B24第一編で規定される字幕に関する定義。

use crate::eight::char::TimeControlMode;
use crate::eight::str::AribStr;
use crate::lang::LangCode;
use crate::utils::{BytesExt, SliceExt};

/// データグループ。
#[derive(Debug, PartialEq, Eq)]
pub struct DataGroup<'a> {
    /// データグループ識別、DGI（6ビット）。
    pub data_group_id: u8,
    /// データグループバージョン（2ビット）。
    pub data_group_version: u8,
    /// データグループリンク番号。
    pub data_group_link_number: u8,
    /// 最終データグループリンク番号。
    pub last_data_group_link_number: u8,
    /// データグループデータ、DGD。
    pub data_group_data: &'a [u8],
}

impl<'a> DataGroup<'a> {
    /// `data`から`DataGroup`を読み取る。
    pub fn read(data: &'a [u8]) -> Option<DataGroup<'a>> {
        if data.len() < 5 {
            log::debug!("invalid DataGroup");
            return None;
        }

        let data_group_id = (data[0] & 0b11111100) >> 2;
        let data_group_version = data[0] & 0b00000011;
        let data_group_link_number = data[1];
        let last_data_group_link_number = data[2];
        let data_group_size = u16::from_be_bytes(data[3..=4].try_into().unwrap());
        if data.len() < 5 + data_group_size as usize {
            log::debug!("invalid DataGroup::data_group_size");
            return None;
        }
        let data_group_data = &data[5..][..data_group_size as usize];

        Some(DataGroup {
            data_group_id,
            data_group_version,
            data_group_link_number,
            last_data_group_link_number,
            data_group_data,
        })
    }
}

/// `value`から`TimeControlMode`を生成する。
///
/// # パニック
///
/// 値が範囲外の場合、このメソッドはパニックする。
fn time_control_mode(value: u8) -> TimeControlMode {
    match value {
        0b00 => TimeControlMode::Free,
        0b01 => TimeControlMode::RealTime,
        0b10 => TimeControlMode::OffsetTime,
        0b11 => TimeControlMode::Reserved,
        _ => unreachable!(),
    }
}

/// 表示モード。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DisplayMode {
    /// 自動表示。
    AutoDisplay,
    /// 自動非表示。
    AutoHide,
    /// 選択表示。
    Selectable,
    /// 受信時特定条件自動表示／非表示。
    ///
    /// ただし記録再生時では未定義。
    MayDisplay,
}

impl DisplayMode {
    /// `value`から`DisplayMode`を生成する。
    ///
    /// # パニック
    ///
    /// 値が範囲外の場合、このメソッドはパニックする。
    #[inline]
    pub fn new(value: u8) -> DisplayMode {
        match value {
            0b00 => DisplayMode::AutoDisplay,
            0b01 => DisplayMode::AutoHide,
            0b10 => DisplayMode::Selectable,
            0b11 => DisplayMode::MayDisplay,
            _ => unreachable!(),
        }
    }
}

/// 字幕の表示形式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CaptionFormat {
    /// 標準密度の横書。
    StandardDensityHorz,
    /// 標準密度の縦書。
    StandardDensityVert,
    /// 高密度の横書。
    HighDensityHorz,
    /// 高密度の縦書。
    HighDensityVert,
    /// 欧文の横書き。
    WesternHorz,
    /// 1920x1080の横書。
    FhdHorz,
    /// 1920x1080の縦書。
    FhdVert,
    /// 960x540の横書。
    QhdHorz,
    /// 960x540の縦書。
    QhdVert,
    /// 1280x720の横書。
    HdHorz,
    /// 1280x720の縦書。
    HdVert,
    /// 720x480の横書。
    SdHorz,
    /// 720x480の縦書。
    SdVert,
    /// 不明。
    Unknown,
}

/// 字幕の文字符号化方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CaptionCharCode {
    /// 8単位符号。
    EightBit,
    /// UCSを用いる符号化方式。
    UCS,
    /// 予備。
    Reserved,
}

/// 字幕のロールアップモード。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CaptionRollupMode {
    /// 非ロールアップ。
    NonRollup,
    /// ロールアップ。
    Rollup,
    /// 予約。
    Reserved,
}

/// 字幕管理データにおける言語識別。
///
/// 値は`0..=7`の範囲であり、それぞれ第1言語～第8言語を表す。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LanguageTag(pub u8);

/// 字幕管理データにおける言語。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptionLanguage {
    /// 言語識別。
    pub language_tag: LanguageTag,

    /// 受信時の表示モード。
    pub dmf_recv: DisplayMode,
    /// 記録再生時の時の表示モード。
    pub dmf_playback: DisplayMode,
    /// 表示条件指定。
    pub dc: Option<u8>,
    /// 言語コード。
    pub lang_code: LangCode,
    /// 表示形式。
    pub format: CaptionFormat,
    /// 文字符号化方式。
    pub tcs: CaptionCharCode,
    /// ロールアップモード。
    pub rollup_mode: CaptionRollupMode,
}

/// 字幕管理データ。
#[derive(Debug, PartialEq, Eq)]
pub struct CaptionManagementData<'a> {
    /// 時刻制御モード。
    pub tmd: TimeControlMode,

    /// オフセット時刻（単位はミリ秒）。
    pub otm: Option<u32>,

    /// 言語。
    pub languages: Vec<CaptionLanguage>,

    /// データユニット。
    pub data_units: Vec<DataUnit<'a>>,
}

impl<'a> CaptionManagementData<'a> {
    /// `data`から`CaptionManagementData`を読み取る。
    pub fn read(data: &'a [u8]) -> Option<CaptionManagementData<'a>> {
        if data.len() < 1 {
            log::debug!("invalid CaptionManagementData");
            return None;
        }

        let tmd = time_control_mode((data[0] & 0b11000000) >> 6);
        let mut data = &data[1..];

        let otm = if tmd == TimeControlMode::OffsetTime {
            if data.len() < 5 {
                log::debug!("invalid CaptionManagementData::OTM");
                return None;
            }

            let otm = data[0..=4].read_bcd_milli();
            data = &data[5..];

            Some(otm)
        } else {
            None
        };

        if data.len() < 1 {
            log::debug!("invalid CaptionManagementData::num_languages");
            return None;
        }
        let num_languages = data[0];
        let mut data = &data[1..];

        let mut languages = Vec::with_capacity(num_languages as usize);
        for _ in 0..num_languages {
            if data.len() < 1 {
                log::debug!("invalid CaptionLanguage");
                return None;
            }

            let language_tag = LanguageTag((data[0] & 0b11100000) >> 5);
            let dmf_recv = DisplayMode::new((data[0] & 0b00001100) >> 2);
            let dmf_playback = DisplayMode::new(data[0] & 0b00000011);

            data = &data[1..];
            let dc =
                if dmf_recv == DisplayMode::MayDisplay && dmf_playback != DisplayMode::MayDisplay {
                    if data.len() < 1 {
                        log::debug!("invalid CaptionLanguage::dc");
                        return None;
                    }

                    let dc = data[0];
                    data = &data[1..];

                    Some(dc)
                } else {
                    None
                };

            if data.len() < 4 {
                log::debug!("invalid CaptionLanguage::lang_code");
                return None;
            }

            let lang_code = LangCode(data[0..=2].try_into().unwrap());
            let format = match (data[3] & 0b11110000) >> 4 {
                0b0000 => CaptionFormat::StandardDensityHorz,
                0b0001 => CaptionFormat::StandardDensityVert,
                0b0010 => CaptionFormat::HighDensityHorz,
                0b0011 => CaptionFormat::HighDensityVert,
                0b0100 => CaptionFormat::WesternHorz,
                0b0110 => CaptionFormat::FhdHorz,
                0b0111 => CaptionFormat::FhdVert,
                0b1000 => CaptionFormat::QhdHorz,
                0b1001 => CaptionFormat::QhdVert,
                0b1100 => CaptionFormat::HdHorz,
                0b1101 => CaptionFormat::HdVert,
                0b1010 => CaptionFormat::SdHorz,
                0b1011 => CaptionFormat::SdVert,
                _ => CaptionFormat::Unknown,
            };
            let tcs = match (data[3] & 0b00001100) >> 2 {
                0b00 => CaptionCharCode::EightBit,
                0b01 => CaptionCharCode::UCS,
                0b10 | 0b11 => CaptionCharCode::Reserved,
                _ => unreachable!(),
            };
            let rollup_mode = match data[3] & 0b00000011 {
                0b00 => CaptionRollupMode::NonRollup,
                0b01 => CaptionRollupMode::Rollup,
                0b10 | 0b11 => CaptionRollupMode::Reserved,
                _ => unreachable!(),
            };
            data = &data[4..];

            languages.push(CaptionLanguage {
                language_tag,
                dmf_recv,
                dmf_playback,
                dc,
                lang_code,
                format,
                tcs,
                rollup_mode,
            });
        }

        let (data_units, _) = DataUnit::read(data)?;
        Some(CaptionManagementData {
            tmd,
            otm,
            languages,
            data_units,
        })
    }
}

/// 字幕文データ。
#[derive(Debug, PartialEq, Eq)]
pub struct CaptionData<'a> {
    /// 時刻制御モード。
    pub tmd: TimeControlMode,

    /// 提示開始時刻（単位はミリ秒）。
    pub stm: Option<u32>,

    /// データユニット。
    pub data_units: Vec<DataUnit<'a>>,
}

impl<'a> CaptionData<'a> {
    /// `data`から`CaptionData`を読み取る。
    pub fn read(data: &'a [u8]) -> Option<CaptionData<'a>> {
        if data.len() < 1 {
            log::debug!("invalid CaptionData");
            return None;
        }

        let tmd = time_control_mode((data[0] & 0b11000000) >> 6);
        let mut data = &data[1..];

        let stm = if matches!(tmd, TimeControlMode::RealTime | TimeControlMode::OffsetTime) {
            if data.len() < 5 {
                log::debug!("invalid CaptionData::STM");
                return None;
            }

            let stm = data[0..=4].read_bcd_milli();
            data = &data[5..];

            Some(stm)
        } else {
            None
        };

        let (data_units, _) = DataUnit::read(data)?;

        Some(CaptionData {
            tmd,
            stm,
            data_units,
        })
    }
}

/// データユニット。
#[derive(Debug, PartialEq, Eq)]
pub enum DataUnit<'a> {
    /// 字幕本文。
    StatementBody(&'a AribStr),

    /// ジオメトリック。
    Geometric(&'a [u8]),

    /// 付加音。
    SynthesizedSound(&'a [u8]),

    /// DRCS。
    Drcs(Drcs<'a>),

    /// カラーマップ。
    Colormap(&'a [u8]),

    /// ビットマップ。
    Bitmap(Bitmap<'a>),

    /// 不明。
    Unknown,
}

impl<'a> DataUnit<'a> {
    /// `data`から`DataUnit`を読み取る。
    ///
    /// 戻り値は`Vec<DataUnit>`と、それを読み取ったあとの残りのバイト列である。
    pub fn read(data: &'a [u8]) -> Option<(Vec<DataUnit<'a>>, &'a [u8])> {
        if data.len() < 3 {
            log::debug!("invalid DataUnit::data_unit_loop_length");
            return None;
        }

        let data_unit_loop_length = ((data[0..=1].read_be_16() as u32) << 8) | data[2] as u32;
        let Some((mut data, rem)) = data[3..].split_at_checked(data_unit_loop_length as usize)
        else {
            log::debug!("invalid DataUnit::data_units");
            return None;
        };

        let mut data_units = Vec::new();
        while !data.is_empty() {
            if data.len() < 5 {
                log::debug!("invalid DataUnit");
                return None;
            }

            let unit_separator = data[0];
            if unit_separator != 0x1F {
                log::debug!("invalid DataUnit::unit_separator");
                return None;
            }

            let data_unit_parameter = data[1];
            let data_unit_size = ((data[2..=3].read_be_16() as u32) << 8) | (data[4] as u32);
            let Some((data_unit_data, rem)) = data[5..].split_at_checked(data_unit_size as usize)
            else {
                log::debug!("invalid DataUnit::data_unit_data");
                return None;
            };
            data = rem;

            let data_unit = match data_unit_parameter {
                // 本文
                0x20 => DataUnit::StatementBody(AribStr::from_bytes(data_unit_data)),
                // ジオメトリック
                0x28 => DataUnit::Geometric(data_unit_data),
                // 付加音
                0x2C => DataUnit::SynthesizedSound(data_unit_data),
                // 1バイトDRCS
                0x30 => DataUnit::Drcs(Drcs::read(data_unit_data, true)?),
                // 2バイトDRCS
                0x31 => DataUnit::Drcs(Drcs::read(data_unit_data, false)?),
                // カラーマップ
                0x34 => DataUnit::Colormap(data_unit_data),
                // ビットマップ
                0x35 => DataUnit::Bitmap(Bitmap::read(data_unit_data)?),
                _ => DataUnit::Unknown,
            };
            data_units.push(data_unit);
        }

        Some((data_units, rem))
    }
}

/// DRCS図形の符号化。
#[derive(Debug, PartialEq, Eq)]
pub struct Drcs<'a> {
    /// DRCSにおける各符号。
    pub codes: Vec<DrcsCode<'a>>,
}

impl<'a> Drcs<'a> {
    /// `data`から`Drcs`を読み取る。
    pub fn read(data: &'a [u8], is_sb: bool) -> Option<Drcs<'a>> {
        let [number_of_code, ref rem @ ..] = *data else {
            log::debug!("invalid Drcs::number_of_code");
            return None;
        };

        let mut data = rem;
        let mut codes = Vec::with_capacity(number_of_code as usize);
        for _ in 0..number_of_code {
            if data.len() < 3 {
                log::debug!("invalid Drcs::character_code");
                return None;
            }

            let character_code = if is_sb {
                let code = data[1];
                match data[0] {
                    0x41 => DrcsCharCode::Drcs1(code),
                    0x42 => DrcsCharCode::Drcs2(code),
                    0x43 => DrcsCharCode::Drcs3(code),
                    0x44 => DrcsCharCode::Drcs4(code),
                    0x45 => DrcsCharCode::Drcs5(code),
                    0x46 => DrcsCharCode::Drcs6(code),
                    0x47 => DrcsCharCode::Drcs7(code),
                    0x48 => DrcsCharCode::Drcs8(code),
                    0x49 => DrcsCharCode::Drcs9(code),
                    0x4A => DrcsCharCode::Drcs10(code),
                    0x4B => DrcsCharCode::Drcs11(code),
                    0x4C => DrcsCharCode::Drcs12(code),
                    0x4D => DrcsCharCode::Drcs13(code),
                    0x4E => DrcsCharCode::Drcs14(code),
                    0x4F => DrcsCharCode::Drcs15(code),
                    _ => {
                        log::debug!("invalid DrcsCharCode: {:02X}", data[0]);
                        return None;
                    }
                }
            } else {
                DrcsCharCode::Drcs0(data[0], data[1])
            };
            let number_of_font = data[2];
            data = &data[3..];

            let mut fonts = Vec::with_capacity(number_of_font as usize);
            for _ in 0..number_of_font {
                if data.len() < 1 {
                    log::debug!("invalid DrcsFont::font_id");
                    return None;
                }

                let font_id = (data[0] & 0b11110000) >> 4;
                let mode = data[0] & 0b00001111;
                data = &data[1..];

                let font_data = match mode {
                    0b0000 | 0b0001 => {
                        let [depth, width, height, ref rem @ ..] = *data else {
                            log::debug!("invalid DrcsUncompressedData");
                            return None;
                        };

                        // bits per pixel
                        let bpp = ((depth + 2) as f32).log2().ceil() as u32;
                        let size = (width as u32) * (height as u32);
                        // let len = (size * bpp).div_ceil(8) as usize;
                        let len = ((size * bpp + 7) / 8) as usize;
                        let Some((pattern_data, rem)) = rem.split_at_checked(len) else {
                            log::debug!("invalid DrcsUncompressedData::pattern_data");
                            return None;
                        };
                        data = rem;

                        let drcs_data = DrcsUncompressedData {
                            depth,
                            width,
                            height,
                            pattern_data,
                        };
                        if mode == 0b0000 {
                            DrcsFontData::UncompressedTwotone(drcs_data)
                        } else {
                            DrcsFontData::UncompressedMultitone(drcs_data)
                        }
                    }

                    0b0010 | 0b0011 => {
                        if data.len() < 4 {
                            log::debug!("invalid DrcsCompressedData");
                            return None;
                        }

                        let region_x = data[0];
                        let region_y = data[1];
                        let geometric_data_len = data[2..=3].read_be_16();
                        let Some((geometric_data, rem)) = data[4..]
                            .split_at_checked(geometric_data_len as usize)
                        else {
                            log::debug!("invalid DrcsCompressedData::geometric_data");
                            return None;
                        };
                        data = rem;

                        let drcs_data = DrcsCompressedData {
                            region_x,
                            region_y,
                            geometric_data,
                        };
                        if mode == 0b0010 {
                            DrcsFontData::CompressedMonochrome(drcs_data)
                        } else {
                            DrcsFontData::CompressedMulticolor(drcs_data)
                        }
                    }
                    _ => DrcsFontData::Unknown,
                };

                fonts.push(DrcsFont {
                    font_id,
                    data: font_data,
                });
            }

            codes.push(DrcsCode {
                character_code,
                fonts,
            });
        }

        Some(Drcs { codes })
    }
}

/// DRCSにおける符号。
#[derive(Debug, PartialEq, Eq)]
pub struct DrcsCode<'a> {
    /// 外字符号。
    pub character_code: DrcsCharCode,
    /// 符号における各フォント。
    pub fonts: Vec<DrcsFont<'a>>,
}

/// DRCSの外字コード。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DrcsCharCode {
    /// DRCS-0。
    Drcs0(u8, u8),

    /// DRCS-1。
    Drcs1(u8),

    /// DRCS-2。
    Drcs2(u8),

    /// DRCS-3。
    Drcs3(u8),

    /// DRCS-4。
    Drcs4(u8),

    /// DRCS-5。
    Drcs5(u8),

    /// DRCS-6。
    Drcs6(u8),

    /// DRCS-7。
    Drcs7(u8),

    /// DRCS-8。
    Drcs8(u8),

    /// DRCS-9。
    Drcs9(u8),

    /// DRCS-10。
    Drcs10(u8),

    /// DRCS-11。
    Drcs11(u8),

    /// DRCS-12。
    Drcs12(u8),

    /// DRCS-13。
    Drcs13(u8),

    /// DRCS-14。
    Drcs14(u8),

    /// DRCS-15。
    Drcs15(u8),
}

/// DRCSにおけるフォント。
#[derive(Debug, PartialEq, Eq)]
pub struct DrcsFont<'a> {
    /// フォント識別（4ビット）。
    pub font_id: u8,
    /// 伝送モードごとのデータ。
    pub data: DrcsFontData<'a>,
}

/// DRCSにおける伝送モードごとのデータ。
#[derive(Debug, PartialEq, Eq)]
pub enum DrcsFontData<'a> {
    /// 2階調、圧縮なし。
    UncompressedTwotone(DrcsUncompressedData<'a>),
    /// 多階調、圧縮なし。
    UncompressedMultitone(DrcsUncompressedData<'a>),
    /// 2色、圧縮あり。
    CompressedMonochrome(DrcsCompressedData<'a>),
    /// 多色、圧縮あり。
    CompressedMulticolor(DrcsCompressedData<'a>),
    /// 不明な伝送モード。
    Unknown,
}

impl<'a> DrcsFontData<'a> {
    /// 圧縮なしのデータであれば内包する`DrcsUncompressedData`を`Some`で包んで返す。
    #[inline]
    pub fn uncompressed(&self) -> Option<&DrcsUncompressedData> {
        match self {
            DrcsFontData::UncompressedTwotone(data) | DrcsFontData::UncompressedMultitone(data) => {
                Some(data)
            }
            _ => None,
        }
    }

    /// 圧縮ありのデータであれば内包する`DrcsCompressedData`を`Some`で包んで返す。
    #[inline]
    pub fn compressed(&self) -> Option<&DrcsCompressedData> {
        match self {
            DrcsFontData::CompressedMonochrome(data) | DrcsFontData::CompressedMulticolor(data) => {
                Some(data)
            }
            _ => None,
        }
    }
}

/// 圧縮なしのフォントデータ。
#[derive(Debug, PartialEq, Eq)]
pub struct DrcsUncompressedData<'a> {
    /// 階層深さ。
    pub depth: u8,
    /// 横方向サイズ。
    pub width: u8,
    /// 縦方向サイズ。
    pub height: u8,
    /// パターンデータ。
    pub pattern_data: &'a [u8],
}

/// 圧縮ありのフォントデータ。
#[derive(Debug, PartialEq, Eq)]
pub struct DrcsCompressedData<'a> {
    /// 縦の論理画素領域。
    pub region_x: u8,
    /// 横の。論理画素領域。
    pub region_y: u8,
    /// ジオメトリックデータ。
    pub geometric_data: &'a [u8],
}

/// ビットマップ図形の符号化。
#[derive(Debug, PartialEq, Eq)]
pub struct Bitmap<'a> {
    /// PNG描画開始のX座標。
    pub x_position: u16,
    /// PNG描画開始のY座標。
    pub y_position: u16,
    /// フラッシングすべき色のインデックス値。
    pub color_indices: &'a [u8],
    /// PNG符号化データ。
    pub png_data: &'a [u8],
}

impl<'a> Bitmap<'a> {
    /// `data`から`Bitmap`を読み取る。
    pub fn read(mut data: &'a [u8]) -> Option<Bitmap<'a>> {
        if data.len() < 5 {
            log::debug!("invalid Bitmap");
            return None;
        }

        let x_position = u16::from_be_bytes(data[0..=1].try_into().unwrap());
        let y_position = u16::from_be_bytes(data[2..=3].try_into().unwrap());
        let num_of_flc_colors = data[4] as usize;
        data = &data[5..];

        if data.len() < num_of_flc_colors {
            log::debug!("invalid Bitmap::num_of_flc_colors");
            return None;
        }
        let color_indices = &data[0..num_of_flc_colors];
        let png_data = &data[num_of_flc_colors..];

        Some(Bitmap {
            x_position,
            y_position,
            color_indices,
            png_data,
        })
    }
}

#[cfg(test)]
mod tests {
    use assert_matches::assert_matches;

    use super::*;

    #[test]
    fn test_drcs() {
        /// ■と□からなる文字列を、1を■、0を□としたビット列としてパースする。
        fn parse_pattern(bits: &str) -> Vec<u8> {
            let bits: Vec<bool> = bits
                .chars()
                .filter_map(|c| match c {
                    '■' => Some(true),
                    '□' => Some(false),
                    _ => None,
                })
                .collect();
            bits.chunks_exact(8)
                .map(|chunk| chunk.iter().fold(0, |n, &b| (n << 1) | u8::from(b)))
                .collect()
        }

        const DRCS1: &[u8] = &hex_literal::hex!(
            "
01 41 21 01 01 02 24 24 00 0F 00 00 00 00 0F 00
00 00 0F 00 00 00 00 0F 00 00 00 F0 0F 00 00 F0
00 F0 00 00 F0 0F 00 00 F0 00 F0 00 00 F0 0F 00
00 F0 00 F0 00 00 F0 0F 00 00 F0 00 F0 00 00 F0
FF F0 00 F0 00 F0 00 00 F0 FF F0 00 F0 00 F0 00
00 F0 00 F0 FF FF F0 F0 00 00 F0 00 F0 FF FF F0
F0 00 0F 00 00 F0 00 F0 00 0F 00 0F 00 00 F0 00
F0 00 0F 00 0F 00 00 F0 00 F0 00 0F 00 0F 00 00
F0 00 F0 00 0F 00 0F 00 0F 00 00 F0 00 0F 00 0F
00 0F 00 00 F0 00 0F 00 0F 00 FF F0 00 F0 00 0F
00 0F 00 FF F0 00 F0 00 0F 00 0F 0F 0F 0F 00 F0
00 0F 00 0F 0F 0F 0F 00 F0 00 0F 00 0F 0F 0F 0F
00 F0 00 0F 00 0F 0F 0F 0F 00 F0 00 0F 00 0F 00
0F 00 00 F0 00 0F 00 0F 00 0F 00 00 F0 00 0F 00
0F 00 0F 00 00 F0 00 0F 00 0F 00 0F 00 00 F0 00
0F 00 00 F0 0F 00 00 F0 00 F0 00 00 F0 0F 00 00
F0 00 F0 00 00 F0 0F 00 00 F0 00 F0 00 00 F0 0F
00 00 F0 00 F0 00 00 F0 0F 00 00 F0 00 F0 00 00
F0 0F 00 00 F0 00 F0 00 00 F0 0F 0F FF FF FF F0
00 00 F0 0F 0F FF FF FF F0 00 00 0F 00 00 00 00
0F 00 00 00 0F 00 00 00 00 0F 00 00
"
        );

        let drcs = Drcs::read(DRCS1, true).unwrap();
        assert_eq!(drcs.codes.len(), 1);

        let code = &drcs.codes[0];
        assert_eq!(code.character_code, DrcsCharCode::Drcs1(0x21));
        assert_eq!(code.fonts.len(), 1);

        let font = &code.fonts[0];
        assert_eq!(font.font_id, 0);
        assert_matches!(font.data, DrcsFontData::UncompressedMultitone(_));

        let DrcsFontData::UncompressedMultitone(data) = &font.data else { unreachable!() };
        assert_eq!(data.depth, 2);
        assert_eq!(data.width, 36);
        assert_eq!(data.height, 36);
        assert_eq!(
            data.pattern_data,
            parse_pattern(
                "
□□□□□□□□□□□□■■■■□□□□□□□□□□□□□□□□□□□□□□□□□□□□□□□□□□□□■■■■□□□□□□□□□□□□□□□□
□□□□□□□□□□□□■■■■□□□□□□□□□□□□□□□□□□□□□□□□□□□□□□□□□□□□■■■■□□□□□□□□□□□□□□□□
□□□□□□□□■■■■□□□□□□□□■■■■□□□□□□□□□□□□□□□□■■■■□□□□□□□□□□□□■■■■□□□□□□□□□□□□
□□□□□□□□■■■■□□□□□□□□■■■■□□□□□□□□□□□□□□□□■■■■□□□□□□□□□□□□■■■■□□□□□□□□□□□□
□□□□□□□□■■■■□□□□□□□□■■■■□□□□□□□□□□□□□□□□■■■■□□□□□□□□□□□□■■■■□□□□□□□□□□□□
□□□□□□□□■■■■□□□□□□□□■■■■□□□□□□□□□□□□□□□□■■■■□□□□□□□□□□□□■■■■□□□□□□□□□□□□
□□□□□□□□■■■■□□□□■■■■■■■■■■■■□□□□□□□□□□□□■■■■□□□□□□□□□□□□■■■■□□□□□□□□□□□□
□□□□□□□□■■■■□□□□■■■■■■■■■■■■□□□□□□□□□□□□■■■■□□□□□□□□□□□□■■■■□□□□□□□□□□□□
□□□□□□□□■■■■□□□□□□□□□□□□■■■■□□□□■■■■■■■■■■■■■■■■■■■■□□□□■■■■□□□□□□□□□□□□
□□□□□□□□■■■■□□□□□□□□□□□□■■■■□□□□■■■■■■■■■■■■■■■■■■■■□□□□■■■■□□□□□□□□□□□□
□□□□■■■■□□□□□□□□□□□□□□□□■■■■□□□□□□□□□□□□■■■■□□□□□□□□□□□□□□□□■■■■□□□□□□□□
□□□□■■■■□□□□□□□□□□□□□□□□■■■■□□□□□□□□□□□□■■■■□□□□□□□□□□□□□□□□■■■■□□□□□□□□
□□□□■■■■□□□□□□□□□□□□□□□□■■■■□□□□□□□□□□□□■■■■□□□□□□□□□□□□□□□□■■■■□□□□□□□□
□□□□■■■■□□□□□□□□□□□□□□□□■■■■□□□□□□□□□□□□■■■■□□□□□□□□□□□□□□□□■■■■□□□□□□□□
□□□□■■■■□□□□□□□□□□□□■■■■□□□□□□□□□□□□□□□□■■■■□□□□□□□□□□□□□□□□■■■■□□□□□□□□
□□□□■■■■□□□□□□□□□□□□■■■■□□□□□□□□□□□□□□□□■■■■□□□□□□□□□□□□□□□□■■■■□□□□□□□□
□□□□■■■■□□□□□□□□■■■■■■■■■■■■□□□□□□□□□□□□■■■■□□□□□□□□□□□□□□□□■■■■□□□□□□□□
□□□□■■■■□□□□□□□□■■■■■■■■■■■■□□□□□□□□□□□□■■■■□□□□□□□□□□□□□□□□■■■■□□□□□□□□
□□□□■■■■□□□□■■■■□□□□■■■■□□□□■■■■□□□□□□□□■■■■□□□□□□□□□□□□□□□□■■■■□□□□□□□□
□□□□■■■■□□□□■■■■□□□□■■■■□□□□■■■■□□□□□□□□■■■■□□□□□□□□□□□□□□□□■■■■□□□□□□□□
□□□□■■■■□□□□■■■■□□□□■■■■□□□□■■■■□□□□□□□□■■■■□□□□□□□□□□□□□□□□■■■■□□□□□□□□
□□□□■■■■□□□□■■■■□□□□■■■■□□□□■■■■□□□□□□□□■■■■□□□□□□□□□□□□□□□□■■■■□□□□□□□□
□□□□■■■■□□□□□□□□□□□□■■■■□□□□□□□□□□□□□□□□■■■■□□□□□□□□□□□□□□□□■■■■□□□□□□□□
□□□□■■■■□□□□□□□□□□□□■■■■□□□□□□□□□□□□□□□□■■■■□□□□□□□□□□□□□□□□■■■■□□□□□□□□
□□□□■■■■□□□□□□□□□□□□■■■■□□□□□□□□□□□□□□□□■■■■□□□□□□□□□□□□□□□□■■■■□□□□□□□□
□□□□■■■■□□□□□□□□□□□□■■■■□□□□□□□□□□□□□□□□■■■■□□□□□□□□□□□□□□□□■■■■□□□□□□□□
□□□□□□□□■■■■□□□□□□□□■■■■□□□□□□□□□□□□□□□□■■■■□□□□□□□□□□□□■■■■□□□□□□□□□□□□
□□□□□□□□■■■■□□□□□□□□■■■■□□□□□□□□□□□□□□□□■■■■□□□□□□□□□□□□■■■■□□□□□□□□□□□□
□□□□□□□□■■■■□□□□□□□□■■■■□□□□□□□□□□□□□□□□■■■■□□□□□□□□□□□□■■■■□□□□□□□□□□□□
□□□□□□□□■■■■□□□□□□□□■■■■□□□□□□□□□□□□□□□□■■■■□□□□□□□□□□□□■■■■□□□□□□□□□□□□
□□□□□□□□■■■■□□□□□□□□■■■■□□□□□□□□□□□□□□□□■■■■□□□□□□□□□□□□■■■■□□□□□□□□□□□□
□□□□□□□□■■■■□□□□□□□□■■■■□□□□□□□□□□□□□□□□■■■■□□□□□□□□□□□□■■■■□□□□□□□□□□□□
□□□□□□□□■■■■□□□□□□□□■■■■□□□□■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■□□□□□□□□□□□□
□□□□□□□□■■■■□□□□□□□□■■■■□□□□■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■□□□□□□□□□□□□
□□□□□□□□□□□□■■■■□□□□□□□□□□□□□□□□□□□□□□□□□□□□□□□□□□□□■■■■□□□□□□□□□□□□□□□□
□□□□□□□□□□□□■■■■□□□□□□□□□□□□□□□□□□□□□□□□□□□□□□□□□□□□■■■■□□□□□□□□□□□□□□□□
                "
            )
        );

        const DRCS2: &[u8] = &hex_literal::hex!(
            "
01 41 21 01 00 00 10 12 10 04 24 22 24 22 2E 22
22 FA 42 21 42 21 44 21 4E 21 55 21 55 21 44 21
44 21 24 22 24 22 24 22 25 FE 10 04
            "
        );

        let drcs = Drcs::read(DRCS2, true).unwrap();
        assert_eq!(drcs.codes.len(), 1);

        let code = &drcs.codes[0];
        assert_eq!(code.character_code, DrcsCharCode::Drcs1(0x21));
        assert_eq!(code.fonts.len(), 1);

        let font = &code.fonts[0];
        assert_eq!(font.font_id, 0);
        assert_matches!(font.data, DrcsFontData::UncompressedTwotone(_));

        let DrcsFontData::UncompressedTwotone(data) = &font.data else { unreachable!() };
        assert_eq!(data.depth, 0);
        assert_eq!(data.width, 16);
        assert_eq!(data.height, 18);
        assert_eq!(
            data.pattern_data,
            parse_pattern(
                "
□□□■□□□□□□□□□■□□
□□■□□■□□□□■□□□■□
□□■□□■□□□□■□□□■□
□□■□■■■□□□■□□□■□
□□■□□□■□■■■■■□■□
□■□□□□■□□□■□□□□■
□■□□□□■□□□■□□□□■
□■□□□■□□□□■□□□□■
□■□□■■■□□□■□□□□■
□■□■□■□■□□■□□□□■
□■□■□■□■□□■□□□□■
□■□□□■□□□□■□□□□■
□■□□□■□□□□■□□□□■
□□■□□■□□□□■□□□■□
□□■□□■□□□□■□□□■□
□□■□□■□□□□■□□□■□
□□■□□■□■■■■■■■■□
□□□■□□□□□□□□□■□□
                "
            )
        );
    }
}
