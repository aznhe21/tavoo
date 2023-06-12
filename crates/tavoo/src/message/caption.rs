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
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum DrcsCharCode {
    Drcs0 { code1: u8, code2: u8 },
    Drcs1 { code: u8 },
    Drcs2 { code: u8 },
    Drcs3 { code: u8 },
    Drcs4 { code: u8 },
    Drcs5 { code: u8 },
    Drcs6 { code: u8 },
    Drcs7 { code: u8 },
    Drcs8 { code: u8 },
    Drcs9 { code: u8 },
    Drcs10 { code: u8 },
    Drcs11 { code: u8 },
    Drcs12 { code: u8 },
    Drcs13 { code: u8 },
    Drcs14 { code: u8 },
    Drcs15 { code: u8 },
}

impl From<&isdb::pes::caption::DrcsCharCode> for DrcsCharCode {
    fn from(character_code: &isdb::pes::caption::DrcsCharCode) -> DrcsCharCode {
        use isdb::pes::caption::DrcsCharCode as DCC;

        match *character_code {
            DCC::Drcs0(code1, code2) => DrcsCharCode::Drcs0 { code1, code2 },
            DCC::Drcs1(code) => DrcsCharCode::Drcs1 { code },
            DCC::Drcs2(code) => DrcsCharCode::Drcs2 { code },
            DCC::Drcs3(code) => DrcsCharCode::Drcs3 { code },
            DCC::Drcs4(code) => DrcsCharCode::Drcs4 { code },
            DCC::Drcs5(code) => DrcsCharCode::Drcs5 { code },
            DCC::Drcs6(code) => DrcsCharCode::Drcs6 { code },
            DCC::Drcs7(code) => DrcsCharCode::Drcs7 { code },
            DCC::Drcs8(code) => DrcsCharCode::Drcs8 { code },
            DCC::Drcs9(code) => DrcsCharCode::Drcs9 { code },
            DCC::Drcs10(code) => DrcsCharCode::Drcs10 { code },
            DCC::Drcs11(code) => DrcsCharCode::Drcs11 { code },
            DCC::Drcs12(code) => DrcsCharCode::Drcs12 { code },
            DCC::Drcs13(code) => DrcsCharCode::Drcs13 { code },
            DCC::Drcs14(code) => DrcsCharCode::Drcs14 { code },
            DCC::Drcs15(code) => DrcsCharCode::Drcs15 { code },
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DrcsCode {
    character_code: DrcsCharCode,
    fonts: Vec<DrcsData>,
}

impl From<&isdb::pes::caption::DrcsCode<'_>> for DrcsCode {
    fn from(code: &isdb::pes::caption::DrcsCode) -> DrcsCode {
        DrcsCode {
            character_code: (&code.character_code).into(),
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
pub struct Drcs(Vec<DrcsCode>);

impl From<&isdb::pes::caption::Drcs<'_>> for Drcs {
    fn from(drcs: &isdb::pes::caption::Drcs) -> Drcs {
        Drcs(drcs.codes.iter().map(Into::into).collect())
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Bitmap {
    x_position: u16,
    y_position: u16,
    color_indices: Binary,
    png_data: Binary,
}

impl From<&isdb::pes::caption::Bitmap<'_>> for Bitmap {
    fn from(bitmap: &isdb::pes::caption::Bitmap) -> Bitmap {
        Bitmap {
            x_position: bitmap.x_position,
            y_position: bitmap.y_position,
            color_indices: Binary(bitmap.color_indices.to_vec()),
            png_data: Binary(bitmap.png_data.to_vec()),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum CaptionDataUnit {
    StatementBody { statement: AribString },
    Drcs { drcs: Drcs },
    Bitmap { bitmap: Bitmap },
}

impl CaptionDataUnit {
    pub fn new(
        data_unit: &isdb::pes::caption::DataUnit,
        is_oneseg: bool,
    ) -> Option<CaptionDataUnit> {
        use isdb::pes::caption::DataUnit;

        match data_unit {
            DataUnit::StatementBody(s) => Some(CaptionDataUnit::StatementBody {
                statement: AribString::new(*s, is_oneseg),
            }),
            DataUnit::Drcs(drcs) => Some(CaptionDataUnit::Drcs { drcs: drcs.into() }),
            DataUnit::Bitmap(bitmap) => Some(CaptionDataUnit::Bitmap {
                bitmap: bitmap.into(),
            }),
            _ => {
                log::trace!("字幕のデータユニットを無視：{:?}", data_unit);
                None
            }
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
            _ => {
                log::trace!("字幕の時間制御モードを無視：{:?}", value);
                Err(())
            }
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
            _ => {
                log::trace!("字幕の表示モードを無視：{:?}", value);
                Err(())
            }
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
            _ => {
                log::trace!("字幕の表示形式を無視：{:?}", value);
                Err(())
            }
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
            language_tag: lang.language_tag.0,
            dmf_recv: lang.dmf_recv.try_into().unwrap_or(DisplayMode::Selectable),
            dmf_playback: lang.dmf_recv.try_into().unwrap_or(DisplayMode::Selectable),
            format: lang.format.try_into().ok(),
            lang_code: lang.lang_code.to_string(),
            rollup_mode: lang.rollup_mode.into(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub enum CaptionGroup {
    A,
    B,
}

impl From<isdb::filters::sorter::CaptionGroup> for CaptionGroup {
    fn from(group: isdb::filters::sorter::CaptionGroup) -> CaptionGroup {
        match group {
            isdb::filters::sorter::CaptionGroup::GroupA => CaptionGroup::A,
            isdb::filters::sorter::CaptionGroup::GroupB => CaptionGroup::B,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CaptionManagementData {
    group: CaptionGroup,
    tmd: TimeControlMode,
    // 要素数は1～2
    languages: Vec<CaptionLanguage>,
    // DRCSを格納する
    data_units: Vec<CaptionDataUnit>,
}

impl CaptionManagementData {
    pub fn new(
        management_data: &isdb::filters::sorter::CaptionManagementData,
        is_oneseg: bool,
    ) -> CaptionManagementData {
        CaptionManagementData {
            group: management_data.group.into(),
            tmd: management_data
                .tmd
                .try_into()
                .unwrap_or(TimeControlMode::Free),
            languages: management_data.languages.iter().map(Into::into).collect(),
            data_units: management_data
                .data_units
                .iter()
                .filter_map(|du| CaptionDataUnit::new(du, is_oneseg))
                .collect(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CaptionData {
    group: CaptionGroup,
    language_tag: u8,
    tmd: TimeControlMode,
    stm: Option<u32>,
    data_units: Vec<CaptionDataUnit>,
}

impl CaptionData {
    pub fn new(data: &isdb::filters::sorter::CaptionData, is_oneseg: bool) -> CaptionData {
        CaptionData {
            group: data.group.into(),
            language_tag: data.language_tag.0,
            tmd: data.tmd.try_into().unwrap_or(TimeControlMode::Free),
            stm: data.stm,
            data_units: data
                .data_units
                .iter()
                .filter_map(|du| CaptionDataUnit::new(du, is_oneseg))
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

impl Caption {
    pub fn new(caption: &isdb::filters::sorter::Caption, is_oneseg: bool) -> Caption {
        match caption {
            isdb::filters::sorter::Caption::ManagementData(management_data) => {
                Caption::ManagementData(CaptionManagementData::new(management_data, is_oneseg))
            }
            isdb::filters::sorter::Caption::Data(data) => {
                Caption::Data(CaptionData::new(data, is_oneseg))
            }
        }
    }
}
