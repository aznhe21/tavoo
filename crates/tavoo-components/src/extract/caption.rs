//! [`isdb::filters::sorter::Caption`]を借用なしに保持する。

#[derive(Debug, Clone)]
struct DrcsCompressedData {
    region_x: u8,
    region_y: u8,
    geometric_data: Vec<u8>,
}

impl DrcsCompressedData {
    pub fn new(data: &isdb::pes::caption::DrcsCompressedData) -> DrcsCompressedData {
        DrcsCompressedData {
            region_x: data.region_x,
            region_y: data.region_y,
            geometric_data: data.geometric_data.to_vec(),
        }
    }
}

impl<'a> From<&'a DrcsCompressedData> for isdb::pes::caption::DrcsCompressedData<'a> {
    fn from(data: &'a DrcsCompressedData) -> isdb::pes::caption::DrcsCompressedData<'a> {
        isdb::pes::caption::DrcsCompressedData {
            region_x: data.region_x,
            region_y: data.region_y,
            geometric_data: &*data.geometric_data,
        }
    }
}

#[derive(Debug, Clone)]
struct DrcsUncompressedData {
    depth: u8,
    width: u8,
    height: u8,
    pattern_data: Vec<u8>,
}

impl DrcsUncompressedData {
    pub fn new(data: &isdb::pes::caption::DrcsUncompressedData) -> DrcsUncompressedData {
        DrcsUncompressedData {
            depth: data.depth,
            width: data.width,
            height: data.height,
            pattern_data: data.pattern_data.to_vec(),
        }
    }
}

impl<'a> From<&'a DrcsUncompressedData> for isdb::pes::caption::DrcsUncompressedData<'a> {
    fn from(data: &'a DrcsUncompressedData) -> isdb::pes::caption::DrcsUncompressedData<'a> {
        isdb::pes::caption::DrcsUncompressedData {
            depth: data.depth,
            width: data.width,
            height: data.height,
            pattern_data: &*data.pattern_data,
        }
    }
}

#[derive(Debug, Clone)]
enum DrcsFontData {
    UncompressedTwotone(DrcsUncompressedData),
    UncompressedMultitone(DrcsUncompressedData),
    CompressedMonochrome(DrcsCompressedData),
    CompressedMulticolor(DrcsCompressedData),
    Unknown,
}

impl DrcsFontData {
    pub fn new(font_data: &isdb::pes::caption::DrcsFontData) -> DrcsFontData {
        match font_data {
            isdb::pes::caption::DrcsFontData::UncompressedTwotone(data) => {
                DrcsFontData::UncompressedTwotone(DrcsUncompressedData::new(data))
            }
            isdb::pes::caption::DrcsFontData::UncompressedMultitone(data) => {
                DrcsFontData::UncompressedMultitone(DrcsUncompressedData::new(data))
            }
            isdb::pes::caption::DrcsFontData::CompressedMonochrome(data) => {
                DrcsFontData::CompressedMonochrome(DrcsCompressedData::new(data))
            }
            isdb::pes::caption::DrcsFontData::CompressedMulticolor(data) => {
                DrcsFontData::CompressedMulticolor(DrcsCompressedData::new(data))
            }
            isdb::pes::caption::DrcsFontData::Unknown => DrcsFontData::Unknown,
        }
    }
}

impl<'a> From<&'a DrcsFontData> for isdb::pes::caption::DrcsFontData<'a> {
    fn from(font_data: &'a DrcsFontData) -> isdb::pes::caption::DrcsFontData<'a> {
        match font_data {
            DrcsFontData::UncompressedTwotone(data) => {
                isdb::pes::caption::DrcsFontData::UncompressedTwotone(data.into())
            }
            DrcsFontData::UncompressedMultitone(data) => {
                isdb::pes::caption::DrcsFontData::UncompressedMultitone(data.into())
            }
            DrcsFontData::CompressedMonochrome(data) => {
                isdb::pes::caption::DrcsFontData::CompressedMonochrome(data.into())
            }
            DrcsFontData::CompressedMulticolor(data) => {
                isdb::pes::caption::DrcsFontData::CompressedMulticolor(data.into())
            }
            DrcsFontData::Unknown => isdb::pes::caption::DrcsFontData::Unknown,
        }
    }
}

#[derive(Debug, Clone)]
struct DrcsFont {
    font_id: u8,
    data: DrcsFontData,
}

impl DrcsFont {
    pub fn new(font: &isdb::pes::caption::DrcsFont) -> DrcsFont {
        DrcsFont {
            font_id: font.font_id,
            data: DrcsFontData::new(&font.data),
        }
    }
}

impl<'a> From<&'a DrcsFont> for isdb::pes::caption::DrcsFont<'a> {
    fn from(font: &'a DrcsFont) -> isdb::pes::caption::DrcsFont<'a> {
        isdb::pes::caption::DrcsFont {
            font_id: font.font_id,
            data: (&font.data).into(),
        }
    }
}

#[derive(Debug, Clone)]
struct DrcsCode {
    character_code: isdb::pes::caption::DrcsCharCode,
    fonts: Vec<DrcsFont>,
}

impl DrcsCode {
    pub fn new(code: &isdb::pes::caption::DrcsCode) -> DrcsCode {
        DrcsCode {
            character_code: code.character_code.clone(),
            fonts: code.fonts.iter().map(DrcsFont::new).collect(),
        }
    }
}

impl<'a> From<&'a DrcsCode> for isdb::pes::caption::DrcsCode<'a> {
    fn from(code: &'a DrcsCode) -> isdb::pes::caption::DrcsCode<'a> {
        isdb::pes::caption::DrcsCode {
            character_code: code.character_code.clone(),
            fonts: code.fonts.iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone)]
struct Drcs {
    codes: Vec<DrcsCode>,
}

impl Drcs {
    pub fn new(drcs: &isdb::pes::caption::Drcs) -> Drcs {
        Drcs {
            codes: drcs.codes.iter().map(DrcsCode::new).collect(),
        }
    }
}

impl<'a> From<&'a Drcs> for isdb::pes::caption::Drcs<'a> {
    fn from(drcs: &'a Drcs) -> isdb::pes::caption::Drcs<'a> {
        isdb::pes::caption::Drcs {
            codes: drcs.codes.iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone)]
struct Bitmap {
    x_position: u16,
    y_position: u16,
    color_indices: Vec<u8>,
    png_data: Vec<u8>,
}

impl Bitmap {
    pub fn new(bitmap: &isdb::pes::caption::Bitmap) -> Bitmap {
        Bitmap {
            x_position: bitmap.x_position,
            y_position: bitmap.y_position,
            color_indices: bitmap.color_indices.to_vec(),
            png_data: bitmap.png_data.to_vec(),
        }
    }
}

impl<'a> From<&'a Bitmap> for isdb::pes::caption::Bitmap<'a> {
    fn from(bitmap: &'a Bitmap) -> isdb::pes::caption::Bitmap<'a> {
        isdb::pes::caption::Bitmap {
            x_position: bitmap.x_position,
            y_position: bitmap.y_position,
            color_indices: &*bitmap.color_indices,
            png_data: &*bitmap.png_data,
        }
    }
}

#[derive(Debug, Clone)]
enum DataUnit {
    StatementBody(isdb::eight::str::AribString),
    Geometric(Vec<u8>),
    SynthesizedSound(Vec<u8>),
    Drcs(Drcs),
    Colormap(Vec<u8>),
    Bitmap(Bitmap),
    Unknown,
}

impl DataUnit {
    pub fn new(unit: &isdb::pes::caption::DataUnit) -> DataUnit {
        match *unit {
            isdb::pes::caption::DataUnit::StatementBody(s) => DataUnit::StatementBody(s.to_owned()),
            isdb::pes::caption::DataUnit::Geometric(g) => DataUnit::Geometric(g.to_vec()),
            isdb::pes::caption::DataUnit::SynthesizedSound(s) => {
                DataUnit::SynthesizedSound(s.to_vec())
            }
            isdb::pes::caption::DataUnit::Drcs(ref d) => DataUnit::Drcs(Drcs::new(d)),
            isdb::pes::caption::DataUnit::Colormap(c) => DataUnit::Colormap(c.to_vec()),
            isdb::pes::caption::DataUnit::Bitmap(ref b) => DataUnit::Bitmap(Bitmap::new(b)),
            isdb::pes::caption::DataUnit::Unknown => DataUnit::Unknown,
        }
    }
}

impl<'a> From<&'a DataUnit> for isdb::pes::caption::DataUnit<'a> {
    fn from(unit: &'a DataUnit) -> isdb::pes::caption::DataUnit<'a> {
        match unit {
            DataUnit::StatementBody(s) => isdb::pes::caption::DataUnit::StatementBody(&**s),
            DataUnit::Geometric(g) => isdb::pes::caption::DataUnit::Geometric(&**g),
            DataUnit::SynthesizedSound(s) => isdb::pes::caption::DataUnit::SynthesizedSound(&**s),
            DataUnit::Drcs(d) => isdb::pes::caption::DataUnit::Drcs(d.into()),
            DataUnit::Colormap(c) => isdb::pes::caption::DataUnit::Colormap(&**c),
            DataUnit::Bitmap(b) => isdb::pes::caption::DataUnit::Bitmap(b.into()),
            DataUnit::Unknown => isdb::pes::caption::DataUnit::Unknown,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CaptionManagementData {
    group: isdb::filters::sorter::CaptionGroup,
    tmd: isdb::eight::char::TimeControlMode,
    otm: Option<u32>,
    languages: Vec<isdb::pes::caption::CaptionLanguage>,
    data_units: Vec<DataUnit>,
}

impl CaptionManagementData {
    pub fn new(data: &isdb::filters::sorter::CaptionManagementData) -> CaptionManagementData {
        CaptionManagementData {
            group: data.group,
            tmd: data.tmd,
            otm: data.otm,
            languages: data.languages.clone(),
            data_units: data.data_units.iter().map(DataUnit::new).collect(),
        }
    }
}

impl<'a> From<&'a CaptionManagementData> for isdb::filters::sorter::CaptionManagementData<'a> {
    fn from(data: &'a CaptionManagementData) -> isdb::filters::sorter::CaptionManagementData<'a> {
        isdb::filters::sorter::CaptionManagementData {
            group: data.group,
            tmd: data.tmd,
            otm: data.otm,
            languages: data.languages.clone(),
            data_units: data.data_units.iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CaptionData {
    group: isdb::filters::sorter::CaptionGroup,
    language_tag: isdb::pes::caption::LanguageTag,
    tmd: isdb::eight::char::TimeControlMode,
    stm: Option<u32>,
    data_units: Vec<DataUnit>,
}

impl CaptionData {
    pub fn new(data: &isdb::filters::sorter::CaptionData) -> CaptionData {
        CaptionData {
            group: data.group,
            language_tag: data.language_tag,
            tmd: data.tmd,
            stm: data.stm,
            data_units: data.data_units.iter().map(DataUnit::new).collect(),
        }
    }
}

impl<'a> From<&'a CaptionData> for isdb::filters::sorter::CaptionData<'a> {
    fn from(data: &'a CaptionData) -> isdb::filters::sorter::CaptionData<'a> {
        isdb::filters::sorter::CaptionData {
            group: data.group,
            language_tag: data.language_tag,
            tmd: data.tmd,
            stm: data.stm,
            data_units: data.data_units.iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Caption {
    ManagementData(CaptionManagementData),
    Data(CaptionData),
}

impl Caption {
    pub fn new(caption: &isdb::filters::sorter::Caption) -> Caption {
        match caption {
            isdb::filters::sorter::Caption::ManagementData(data) => {
                Caption::ManagementData(CaptionManagementData::new(data))
            }
            isdb::filters::sorter::Caption::Data(data) => Caption::Data(CaptionData::new(data)),
        }
    }
}

impl<'a> From<&'a Caption> for isdb::filters::sorter::Caption<'a> {
    fn from(caption: &'a Caption) -> isdb::filters::sorter::Caption<'a> {
        match caption {
            Caption::ManagementData(data) => {
                isdb::filters::sorter::Caption::ManagementData(data.into())
            }
            Caption::Data(data) => isdb::filters::sorter::Caption::Data(data.into()),
        }
    }
}
