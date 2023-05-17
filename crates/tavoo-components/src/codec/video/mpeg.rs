//! MPEGのメタデータをパースする。

use std::fmt;

use crate::codec::Rational;

/// MPEG-2のプロファイル。
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Profile(pub u8);

impl Profile {
    /// Simple profile
    pub const SIMPLE: Profile = Profile(0b101);
    /// Main profile
    pub const MAIN: Profile = Profile(0b100);
    /// SNR Scalable profile
    pub const SNR_SCALABLE: Profile = Profile(0b011);
    /// Spatially Scalable profile
    pub const SPATIALLY_SCALABLE: Profile = Profile(0b010);
    /// High profile
    pub const HIGH: Profile = Profile(0b001);
}

impl fmt::Debug for Profile {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Profile::SIMPLE => f.write_str("Simple"),
            Profile::MAIN => f.write_str("Main"),
            Profile::SNR_SCALABLE => f.write_str("SNR Scalable"),
            Profile::SPATIALLY_SCALABLE => f.write_str("Spatially Scalable"),
            Profile::HIGH => f.write_str("High"),
            _ => f.debug_tuple("Profile").field(&self.0).finish(),
        }
    }
}

/// MPEG-2のレベル。
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Level(pub u8);

impl Level {
    /// Low Level
    pub const LOW: Level = Level(0b1010);
    /// Main Level
    pub const MAIN: Level = Level(0b1000);
    /// High 1440 Level
    pub const HIGH_1440: Level = Level(0b0110);
    /// High Level
    pub const HIGH: Level = Level(0b0100);
}

impl fmt::Debug for Level {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Level::LOW => f.write_str("Low"),
            Level::MAIN => f.write_str("Main"),
            Level::HIGH_1440 => f.write_str("High 1440"),
            Level::HIGH => f.write_str("High"),
            _ => f.debug_tuple("Level").field(&self.0).finish(),
        }
    }
}

/// 色差フォーマット。
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ChromaFormat {
    /// 不明。
    Unknown,
    /// 4:2:0
    CF420,
    /// 4:2:2
    CF422,
    /// 4:4:4
    CF444,
}

impl fmt::Debug for ChromaFormat {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Unknown => f.write_str("Unknown"),
            Self::CF420 => f.write_str("4:2:0"),
            Self::CF422 => f.write_str("4:2:2"),
            Self::CF444 => f.write_str("4:4:4"),
        }
    }
}

/// MPEGのシーケンスヘッダ。
#[derive(Debug, Clone)]
pub struct Sequence {
    /// 横方向の大きさ。
    pub horizontal_size: u16,
    /// 縦方向の大きさ。
    pub vertical_size: u16,
    /// ピクセルアスペクト比。
    pub pixel_aspect_ratio: Rational<u32>,
    /// フレームレート。
    pub frame_rate: Rational<u32>,
    /// ビットレート。
    pub bit_rate: u32,
    /// MPEG-2の拡張ヘッダ。
    pub extension: Option<SeqExt>,
}

/// MPEG-2の拡張ヘッダ。
#[derive(Debug, Clone)]
pub struct SeqExt {
    /// レベル。
    pub level: Level,
    /// プロファイル。
    pub profile: Profile,
    /// progressiveフィールド。
    pub progressive: bool,
    /// 色差フォーマット。
    pub chroma_format: ChromaFormat,
}

impl Sequence {
    /// `buf`からMPEGのシーケンスヘッダを探して返す。
    ///
    /// 拡張ヘッダが続く場合はそれを加味した情報を返す。
    pub fn find(mut buf: &[u8]) -> Option<Sequence> {
        let pos = memchr::memmem::find(buf, &[0x00, 0x00, 0x01, 0xB3])?;
        buf = &buf[pos..];
        if buf.len() < 12 {
            return None;
        }

        let mut horizontal_size = ((buf[4] as u16) << 4) | ((buf[5] >> 4) as u16);
        let mut vertical_size = (((buf[5] & 0x0F) as u16) << 8) | (buf[6] as u16);
        let pixel_aspect_ratio = match buf[7] >> 4 {
            1 => Rational::new(1, 1),
            2 => Rational::new(4, 3),
            3 => Rational::new(16, 9),
            4 => Rational::new(221, 100),
            _ => Rational::new(0, 1),
        };
        let mut frame_rate = match buf[7] & 0x0F {
            1 => Rational::new(24000, 1001),
            2 => Rational::new(24, 1),
            3 => Rational::new(25, 1),
            4 => Rational::new(30000, 1001),
            5 => Rational::new(30, 1),
            6 => Rational::new(50, 1),
            7 => Rational::new(60000, 1001),
            8 => Rational::new(60, 1),
            _ => Rational::new(0, 1),
        };
        let mut bit_rate =
            ((buf[8] as u32) << 10) | ((buf[9] as u32) << 2) | ((buf[10] >> 6) as u32);

        buf = &buf[12..];
        let ext_pos = memchr::memmem::find(buf, &[0x00, 0x00, 0x01, 0xB5]);
        let extension = ext_pos.and_then(|pos| {
            let buf = &buf[pos..];
            if buf.len() < 10 {
                return None;
            }

            let profile = Profile(buf[4] & 0x07);
            let level = Level(buf[5] >> 4);

            let progressive = buf[5] & 0b1000 != 0;
            let chroma_format = match (buf[5] & 0b110) >> 1 {
                0 => ChromaFormat::Unknown,
                1 => ChromaFormat::CF420,
                2 => ChromaFormat::CF422,
                3 => ChromaFormat::CF444,
                _ => unreachable!(),
            };

            let horz_size_ext = ((buf[5] & 1) << 1) | (buf[6] >> 7);
            let vert_size_ext = buf[6] >> 5;
            horizontal_size |= (horz_size_ext as u16) << 12;
            vertical_size |= (vert_size_ext as u16) << 12;

            let bit_rate_ext = (((buf[6] & 0x1F) as u32) << 7) | ((buf[7] >> 1) as u32);
            bit_rate |= bit_rate_ext << 18;

            let frame_rate_n = (buf[9] >> 5) + 1;
            let frame_rate_d = (buf[9] & 0x1F) + 1;
            frame_rate.numerator *= frame_rate_n as u32;
            frame_rate.denominator *= frame_rate_d as u32;

            Some(SeqExt {
                level,
                profile,
                progressive,
                chroma_format,
            })
        });

        Some(Sequence {
            horizontal_size,
            vertical_size,
            pixel_aspect_ratio,
            frame_rate,
            bit_rate,
            extension,
        })
    }
}
