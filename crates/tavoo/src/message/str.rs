use isdb::eight::char::{AribChar as IsdbChar, CharSize as IsdbCharSize, DrcsChar};
use isdb::eight::str::AribStr as IsdbStr;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CharSize {
    Small,
    Medium,
    Normal,
    HeightW,
    WidthW,
    SizeW,
}

/// ARIB TR-B14で使用可とされている符号。
///
/// [`isdb::eight::char::AribChar`]のサブセットであるため、
/// 各バリアントと符号の対応等についてはそちらの文書を参照。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum AribChar {
    CharSize { char_size: CharSize },
    String { string: String },
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

    Null,
    ActivePositionBackward,
    ActivePositionForward,
    ActivePositionDown,
    ActivePositionUp,
    ActivePositionReturn,
    ParameterizedActivePositionForward { p1: u8 },
    ActivePositionSet { p1: u8, p2: u8 },
    ClearScreen,
    UnitSeparator,
    Space,
    Delete,
    ColorForeground { p1: u8 },
    ColorBackground { p1: u8 },
    ColorHalfForeground { p1: u8 },
    ColorHalfBackground { p1: u8 },
    ColorPalette { p1: u8 },
    PatternPolarityNormal,
    PatternPolarityInverted1,
    FlushingControlStartNormal,
    FlushingControlStartInverted,
    FlushingControlStop,
    WaitForProcess { p1: u8 },
    RepeatCharacter { p1: u8 },
    StopLining,
    StartLining,
    HighlightBlock { p1: u8 },
    SetWritingFormatInit { p1: u8 },
    RasterColorCommand { p1: u8 },
    ActiveCoordinatePositionSet { p1: u32, p2: u32 },
    SetDisplayFormat { p1: u32, p2: u32 },
    SetDisplayPosition { p1: u32, p2: u32 },
    CharacterCompositionDotDesignation { p1: u32, p2: u32 },
    SetHorizontalSpacing { p1: u32 },
    SetVerticalSpacing { p1: u32 },
    OrnamentControlClear,
    OrnamentControlHemming { p1: u8 },
    BuiltinSoundReplay { p1: u32 },
    ScrollDesignation { p1: u8, p2: u8 },
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct AribString(Vec<AribChar>);

impl AribString {
    pub fn new(s: &IsdbStr, is_oneseg: bool) -> AribString {
        let options = if is_oneseg {
            isdb::eight::decode::Options::ONESEG_CAPTION
        } else {
            isdb::eight::decode::Options::CAPTION
        };

        let mut chars = Vec::new();

        let mut string = String::new();
        let mut char_size = IsdbCharSize::Normal;
        for c in s.decode(options) {
            let ch = match c {
                IsdbChar::CharSize(IsdbCharSize::Small) => {
                    char_size = IsdbCharSize::Small;
                    AribChar::CharSize {
                        char_size: CharSize::Small,
                    }
                }
                IsdbChar::CharSize(IsdbCharSize::Medium) => {
                    char_size = IsdbCharSize::Medium;
                    AribChar::CharSize {
                        char_size: CharSize::Medium,
                    }
                }
                IsdbChar::CharSize(IsdbCharSize::Normal) => {
                    char_size = IsdbCharSize::Normal;
                    AribChar::CharSize {
                        char_size: CharSize::Normal,
                    }
                }
                IsdbChar::CharSize(IsdbCharSize::HeightW) => {
                    char_size = IsdbCharSize::HeightW;
                    AribChar::CharSize {
                        char_size: CharSize::HeightW,
                    }
                }
                IsdbChar::CharSize(IsdbCharSize::WidthW) => {
                    char_size = IsdbCharSize::WidthW;
                    AribChar::CharSize {
                        char_size: CharSize::WidthW,
                    }
                }
                IsdbChar::CharSize(IsdbCharSize::SizeW) => {
                    char_size = IsdbCharSize::SizeW;
                    AribChar::CharSize {
                        char_size: CharSize::SizeW,
                    }
                }

                IsdbChar::Generic(c) => {
                    string.push(c.to_char(char_size).unwrap_or(char::REPLACEMENT_CHARACTER));
                    continue;
                }

                IsdbChar::Drcs(DrcsChar::Drcs0(c1, c2)) => AribChar::Drcs0 {
                    code1: c1.get(),
                    code2: c2.get(),
                },
                IsdbChar::Drcs(DrcsChar::Drcs1(c)) => AribChar::Drcs1 { code: c.get() },
                IsdbChar::Drcs(DrcsChar::Drcs2(c)) => AribChar::Drcs2 { code: c.get() },
                IsdbChar::Drcs(DrcsChar::Drcs3(c)) => AribChar::Drcs3 { code: c.get() },
                IsdbChar::Drcs(DrcsChar::Drcs4(c)) => AribChar::Drcs4 { code: c.get() },
                IsdbChar::Drcs(DrcsChar::Drcs5(c)) => AribChar::Drcs5 { code: c.get() },
                IsdbChar::Drcs(DrcsChar::Drcs6(c)) => AribChar::Drcs6 { code: c.get() },
                IsdbChar::Drcs(DrcsChar::Drcs7(c)) => AribChar::Drcs7 { code: c.get() },
                IsdbChar::Drcs(DrcsChar::Drcs8(c)) => AribChar::Drcs8 { code: c.get() },
                IsdbChar::Drcs(DrcsChar::Drcs9(c)) => AribChar::Drcs9 { code: c.get() },
                IsdbChar::Drcs(DrcsChar::Drcs10(c)) => AribChar::Drcs10 { code: c.get() },
                IsdbChar::Drcs(DrcsChar::Drcs11(c)) => AribChar::Drcs11 { code: c.get() },
                IsdbChar::Drcs(DrcsChar::Drcs12(c)) => AribChar::Drcs12 { code: c.get() },
                IsdbChar::Drcs(DrcsChar::Drcs13(c)) => AribChar::Drcs13 { code: c.get() },
                IsdbChar::Drcs(DrcsChar::Drcs14(c)) => AribChar::Drcs14 { code: c.get() },
                IsdbChar::Drcs(DrcsChar::Drcs15(c)) => AribChar::Drcs15 { code: c.get() },

                IsdbChar::Null => AribChar::Null,
                IsdbChar::ActivePositionBackward => AribChar::ActivePositionBackward,
                IsdbChar::ActivePositionForward => AribChar::ActivePositionForward,
                IsdbChar::ActivePositionDown => AribChar::ActivePositionDown,
                IsdbChar::ActivePositionUp => AribChar::ActivePositionUp,
                IsdbChar::ActivePositionReturn => AribChar::ActivePositionReturn,
                IsdbChar::ParameterizedActivePositionForward(p1) => {
                    AribChar::ParameterizedActivePositionForward { p1 }
                }
                IsdbChar::ActivePositionSet(p1, p2) => AribChar::ActivePositionSet { p1, p2 },
                IsdbChar::ClearScreen => AribChar::ClearScreen,
                IsdbChar::UnitSeparator => AribChar::UnitSeparator,
                IsdbChar::Space => AribChar::Space,
                IsdbChar::Delete => AribChar::Delete,
                IsdbChar::ColorForeground(p1) => AribChar::ColorForeground { p1 },
                IsdbChar::ColorBackground(p1) => AribChar::ColorBackground { p1 },
                IsdbChar::ColorHalfForeground(p1) => AribChar::ColorHalfForeground { p1 },
                IsdbChar::ColorHalfBackground(p1) => AribChar::ColorHalfBackground { p1 },
                IsdbChar::ColorPalette(p1) => AribChar::ColorPalette { p1 },
                IsdbChar::PatternPolarityNormal => AribChar::PatternPolarityNormal,
                IsdbChar::PatternPolarityInverted1 => AribChar::PatternPolarityInverted1,
                IsdbChar::FlushingControlStartNormal => AribChar::FlushingControlStartNormal,
                IsdbChar::FlushingControlStartInverted => AribChar::FlushingControlStartInverted,
                IsdbChar::FlushingControlStop => AribChar::FlushingControlStop,
                // TIMEは処理待ちのみ使用可能
                IsdbChar::WaitForProcess(p1) => AribChar::WaitForProcess { p1 },
                IsdbChar::RepeatCharacter(p1) => AribChar::RepeatCharacter { p1 },
                IsdbChar::StopLining => AribChar::StopLining,
                IsdbChar::StartLining => AribChar::StartLining,
                IsdbChar::HighlightBlock(p1) => AribChar::HighlightBlock { p1 },
                IsdbChar::SetWritingFormatInit(p1) => AribChar::SetWritingFormatInit { p1 },
                IsdbChar::RasterColorCommand(p1) => AribChar::RasterColorCommand { p1 },
                IsdbChar::ActiveCoordinatePositionSet(p1, p2) => {
                    AribChar::ActiveCoordinatePositionSet { p1, p2 }
                }
                IsdbChar::SetDisplayFormat(p1, p2) => AribChar::SetDisplayFormat { p1, p2 },
                IsdbChar::SetDisplayPosition(p1, p2) => AribChar::SetDisplayPosition { p1, p2 },
                IsdbChar::CharacterCompositionDotDesignation(p1, p2) => {
                    AribChar::CharacterCompositionDotDesignation { p1, p2 }
                }
                IsdbChar::SetHorizontalSpacing(p1) => AribChar::SetHorizontalSpacing { p1 },
                IsdbChar::SetVerticalSpacing(p1) => AribChar::SetVerticalSpacing { p1 },
                IsdbChar::OrnamentControlClear => AribChar::OrnamentControlClear,
                IsdbChar::OrnamentControlHemming(p1) => AribChar::OrnamentControlHemming { p1 },
                IsdbChar::BuiltinSoundReplay(p1) => AribChar::BuiltinSoundReplay { p1 },
                IsdbChar::ScrollDesignation(p1, p2) => AribChar::ScrollDesignation { p1, p2 },

                // ARIB TR-B14より使われない符号は無視
                _ => {
                    log::trace!("字幕の符号を無視：{:?}", c);
                    continue;
                }
            };

            if !string.is_empty() {
                chars.push(AribChar::String {
                    string: std::mem::take(&mut string),
                });
            }
            chars.push(ch);
        }

        if !string.is_empty() {
            chars.push(AribChar::String {
                string: std::mem::take(&mut string),
            });
        }

        AribString(chars)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arib_string() {
        assert_eq!(
            serde_json::to_value(&AribString(vec![])).unwrap(),
            serde_json::json!([]),
        );

        assert_eq!(
            serde_json::to_value(&AribString(vec![
                AribChar::ClearScreen,
                AribChar::String {
                    string: "hoge\0fuga".to_string()
                }
            ]))
            .unwrap(),
            serde_json::json!([
                {
                    "type": "clear-screen",
                },
                {
                    "type": "string",
                    "string": "hoge\0fuga",
                },
            ]),
        );
    }
}
