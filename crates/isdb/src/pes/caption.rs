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

/// 字幕管理データにおける言語。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptionLanguage {
    /// 言語識別（3ビット）。
    pub language_tag: u8,

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

            let language_tag = (data[0] & 0b11100000) >> 5;
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

    /// 1バイトDRCS。
    DrcsSb(&'a [u8]),

    /// 2バイトDRCS。
    DrcsDb(&'a [u8]),

    /// カラーマップ。
    Colormap(&'a [u8]),

    /// ビットマップ。
    Bitmap(&'a [u8]),

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
                0x30 => DataUnit::DrcsSb(data_unit_data),
                // 2バイトDRCS
                0x31 => DataUnit::DrcsDb(data_unit_data),
                // カラーマップ
                0x34 => DataUnit::Colormap(data_unit_data),
                // ビットマップ
                0x35 => DataUnit::Bitmap(data_unit_data),
                _ => DataUnit::Unknown,
            };
            data_units.push(data_unit);
        }

        Some((data_units, rem))
    }
}
