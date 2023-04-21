use super::bin::Binary;
use super::str::AribString;

#[derive(Debug, Clone, serde::Serialize)]
pub struct DrcsData {
    depth: u8,
    width: u8,
    height: u8,
    pattern_data: Binary,
}

impl From<&isdb::pes::caption::DrcsUncompressedData<'_>> for DrcsData {
    fn from(data: &isdb::pes::caption::DrcsUncompressedData) -> DrcsData {
        DrcsData {
            depth: data.depth,
            width: data.width,
            height: data.height,
            pattern_data: data.pattern_data.to_vec().into(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DrcsCode {
    character_code: u16,
    fonts: Vec<DrcsData>,
}

impl From<&isdb::pes::caption::DrcsCode<'_>> for DrcsCode {
    fn from(code: &isdb::pes::caption::DrcsCode) -> DrcsCode {
        DrcsCode {
            character_code: code.character_code,
            fonts: code
                .fonts
                .iter()
                .filter_map(|font| {
                    // font_idは0のみなので無視
                    // modeはフルセグやBSでは0001（多階調、圧縮なし）のみ、
                    //       ワンセグでは0000（２階調、圧縮なし）のみ
                    font.data.uncompressed().map(Into::into)
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Drcs {
    codes: Vec<DrcsCode>,
}

impl From<&isdb::pes::caption::Drcs<'_>> for Drcs {
    fn from(drcs: &isdb::pes::caption::Drcs) -> Drcs {
        Drcs {
            codes: drcs.codes.iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum CaptionDataUnit {
    StatementBody { statement: AribString },
    Drcs { drcs: Drcs },
}

impl TryFrom<&isdb::pes::caption::DataUnit<'_>> for CaptionDataUnit {
    type Error = ();

    fn try_from(data_unit: &isdb::pes::caption::DataUnit) -> Result<CaptionDataUnit, Self::Error> {
        use isdb::pes::caption::DataUnit;

        match data_unit {
            DataUnit::StatementBody(s) => Ok(CaptionDataUnit::StatementBody {
                statement: (*s).into(),
            }),
            DataUnit::DrcsSb(drcs) | DataUnit::DrcsDb(drcs) => {
                Ok(CaptionDataUnit::Drcs { drcs: drcs.into() })
            }
            _ => Err(()),
        }
    }
}

// 字幕では「フリー」のみ
// 文字スーパーでは「フリー」「リアルタイム」
#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TimeControlMode {
    /// フリー。
    Free,
    /// リアルタイム。
    RealTime,
}

impl TryFrom<isdb::eight::char::TimeControlMode> for TimeControlMode {
    type Error = ();

    fn try_from(value: isdb::eight::char::TimeControlMode) -> Result<TimeControlMode, Self::Error> {
        use isdb::eight::char;

        match value {
            char::TimeControlMode::Free => Ok(TimeControlMode::Free),
            char::TimeControlMode::RealTime => Ok(TimeControlMode::RealTime),
            _ => Err(()),
        }
    }
}

// 自動表示または選択表示のみ
#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DisplayMode {
    /// 自動表示。
    AutoDisplay,
    /// 選択表示。
    Selectable,
}

impl TryFrom<isdb::pes::caption::DisplayMode> for DisplayMode {
    type Error = ();

    fn try_from(value: isdb::pes::caption::DisplayMode) -> Result<DisplayMode, Self::Error> {
        use isdb::pes::caption;

        match value {
            caption::DisplayMode::AutoDisplay => Ok(DisplayMode::AutoDisplay),
            caption::DisplayMode::Selectable => Ok(DisplayMode::Selectable),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CaptionFormat {
    /// 960x540の横書。
    QhdHorz,
    /// 960x540の縦書。
    QhdVert,
    /// 720x480の横書。
    SdHorz,
    /// 720x480の縦書。
    SdVert,
}

impl TryFrom<isdb::pes::caption::CaptionFormat> for CaptionFormat {
    type Error = ();

    fn try_from(value: isdb::pes::caption::CaptionFormat) -> Result<CaptionFormat, Self::Error> {
        use isdb::pes::caption;

        match value {
            caption::CaptionFormat::QhdHorz => Ok(CaptionFormat::QhdHorz),
            caption::CaptionFormat::QhdVert => Ok(CaptionFormat::QhdVert),
            caption::CaptionFormat::SdHorz => Ok(CaptionFormat::SdHorz),
            caption::CaptionFormat::SdVert => Ok(CaptionFormat::SdVert),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CaptionRollupMode {
    /// 非ロールアップ。
    NonRollup,
    /// ロールアップ。
    Rollup,
    /// 予約。
    Reserved,
}

impl From<isdb::pes::caption::CaptionRollupMode> for CaptionRollupMode {
    fn from(value: isdb::pes::caption::CaptionRollupMode) -> CaptionRollupMode {
        use isdb::pes::caption;

        match value {
            caption::CaptionRollupMode::NonRollup => CaptionRollupMode::NonRollup,
            caption::CaptionRollupMode::Rollup => CaptionRollupMode::Rollup,
            caption::CaptionRollupMode::Reserved => CaptionRollupMode::Reserved,
        }
    }
}

// - TCSは「8単位符号」のみ
#[derive(Debug, Clone, serde::Serialize)]
pub struct CaptionLanguage {
    // 0～1
    language_tag: u8,
    dmf_recv: DisplayMode,
    dmf_playback: DisplayMode,
    format: Option<CaptionFormat>,
    lang_code: String,
    rollup_mode: CaptionRollupMode,
}

impl From<&isdb::pes::caption::CaptionLanguage> for CaptionLanguage {
    fn from(lang: &isdb::pes::caption::CaptionLanguage) -> CaptionLanguage {
        CaptionLanguage {
            language_tag: lang.language_tag,
            dmf_recv: lang.dmf_recv.try_into().unwrap_or(DisplayMode::Selectable),
            dmf_playback: lang.dmf_recv.try_into().unwrap_or(DisplayMode::Selectable),
            format: lang.format.try_into().ok(),
            lang_code: lang.lang_code.to_string(),
            rollup_mode: lang.rollup_mode.into(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CaptionManagementData {
    tmd: TimeControlMode,
    // 要素数は1～2
    languages: Vec<CaptionLanguage>,
    // DRCSを格納する
    data_units: Vec<CaptionDataUnit>,
}

impl From<&isdb::pes::caption::CaptionManagementData<'_>> for CaptionManagementData {
    fn from(management_data: &isdb::pes::caption::CaptionManagementData) -> CaptionManagementData {
        CaptionManagementData {
            tmd: management_data
                .tmd
                .try_into()
                .unwrap_or(TimeControlMode::Free),
            languages: management_data.languages.iter().map(Into::into).collect(),
            data_units: management_data
                .data_units
                .iter()
                .filter_map(|du| du.try_into().ok())
                .collect(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CaptionData {
    tmd: TimeControlMode,
    stm: Option<u32>,
    data_units: Vec<CaptionDataUnit>,
}

impl From<&isdb::pes::caption::CaptionData<'_>> for CaptionData {
    fn from(data: &isdb::pes::caption::CaptionData) -> CaptionData {
        CaptionData {
            tmd: data.tmd.try_into().unwrap_or(TimeControlMode::Free),
            stm: data.stm,
            data_units: data
                .data_units
                .iter()
                .filter_map(|du| du.try_into().ok())
                .collect(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Caption {
    ManagementData(CaptionManagementData),
    Data(CaptionData),
}

impl From<&isdb::filters::sorter::Caption<'_>> for Caption {
    fn from(caption: &isdb::filters::sorter::Caption) -> Caption {
        match caption {
            isdb::filters::sorter::Caption::ManagementData(management_data) => {
                Caption::ManagementData(management_data.into())
            }
            isdb::filters::sorter::Caption::Data(data) => Caption::Data(data.into()),
        }
    }
}
