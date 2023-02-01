//! ARIB STD-B10で定義されている、定数を伴う型。

use std::fmt;

/// サービスの種別。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ServiceType(pub u8);

impl ServiceType {
    /// デジタルTVサービス
    pub const DIGITAL_TV: ServiceType = ServiceType(0x01);
    /// デジタル音声サービス
    pub const DIGITAL_AUDIO: ServiceType = ServiceType(0x02);
    // 0x03 - 0x7F 未定義
    // 0x80 - 0xA0 事業者定義
    /// 臨時映像サービス
    pub const TEMPORARY_VIDEO: ServiceType = ServiceType(0xA1);
    /// 臨時音声サービス
    pub const TEMPORARY_AUDIO: ServiceType = ServiceType(0xA2);
    /// 臨時データサービス
    pub const TEMPORARY_DATA: ServiceType = ServiceType(0xA3);
    /// エンジニアリングサービス
    pub const ENGINEERING: ServiceType = ServiceType(0xA4);
    /// プロモーション映像サービス
    pub const PROMOTION_VIDEO: ServiceType = ServiceType(0xA5);
    /// プロモーション音声サービス
    pub const PROMOTION_AUDIO: ServiceType = ServiceType(0xA6);
    /// プロモーションデータサービス
    pub const PROMOTION_DATA: ServiceType = ServiceType(0xA7);
    /// 事前蓄積用データサービス
    pub const ACCUMULATION_DATA: ServiceType = ServiceType(0xA8);
    /// 蓄積専用データサービス
    pub const ACCUMULATION_ONLY_DATA: ServiceType = ServiceType(0xA9);
    /// ブックマーク一覧データサービス
    pub const BOOKMARK_LIST_DATA: ServiceType = ServiceType(0xAA);
    /// サーバー型サイマルサービス
    pub const SERVER_TYPE_SIMULTANEOUS: ServiceType = ServiceType(0xAB);
    /// 独立ファイルサービス
    pub const INDEPENDENT_FILE: ServiceType = ServiceType(0xAC);
    /// 超高精細度4K専用TVサービス
    pub const UHD_TV: ServiceType = ServiceType(0xAD);
    // 0xAD - 0xBF 未定義(標準化機関定義領域)
    /// データサービス
    pub const DATA: ServiceType = ServiceType(0xC0);
    /// TLVを用いた蓄積型サービス
    pub const TLV_ACCUMULATION: ServiceType = ServiceType(0xC1);
    /// マルチメディアサービス
    pub const MULTIMEDIA: ServiceType = ServiceType(0xC2);
    // 0xC3 - 0xFF 未定義
    /// 無効
    pub const INVALID: ServiceType = ServiceType(0xFF);

    /// 定義されているサービス種別かどうかを返す。
    #[inline]
    pub fn is_known(&self) -> bool {
        matches!(self.0, 0x01 | 0x02 | 0xA1..=0xAD | 0xC0..=0xC2)
    }
}

impl fmt::Display for ServiceType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = match *self {
            ServiceType::DIGITAL_TV => "デジタルTVサービス",
            ServiceType::DIGITAL_AUDIO => "デジタル音声サービス",
            ServiceType::TEMPORARY_VIDEO => "臨時映像サービス",
            ServiceType::TEMPORARY_AUDIO => "臨時音声サービス",
            ServiceType::TEMPORARY_DATA => "臨時データサービス",
            ServiceType::ENGINEERING => "エンジニアリングサービス",
            ServiceType::PROMOTION_VIDEO => "プロモーション映像サービス",
            ServiceType::PROMOTION_AUDIO => "プロモーション音声サービス",
            ServiceType::PROMOTION_DATA => "プロモーションデータサービス",
            ServiceType::ACCUMULATION_DATA => "事前蓄積用データサービス",
            ServiceType::ACCUMULATION_ONLY_DATA => "蓄積専用データサービス",
            ServiceType::BOOKMARK_LIST_DATA => "ブックマーク一覧データサービス",
            ServiceType::SERVER_TYPE_SIMULTANEOUS => "サーバー型サイマルサービス",
            ServiceType::INDEPENDENT_FILE => "独立ファイルサービス",
            ServiceType::UHD_TV => "超高精細度4K専用TVサービス",
            ServiceType::DATA => "データサービス",
            ServiceType::TLV_ACCUMULATION => "TLVを用いた蓄積型サービス",
            ServiceType::MULTIMEDIA => "マルチメディアサービス",
            _ => return write!(f, "未定義（0x{:02X}）", self.0),
        };
        f.write_str(s)
    }
}

/// ストリームの種別。
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StreamType(pub u8);

impl StreamType {
    /// ISO/IEC 11172-2 Video
    pub const MPEG1_VIDEO: StreamType = StreamType(0x01);

    /// ITU-T Rec. H.262 | ISO/IEC 13818-2 Video or ISO/IEC 11172-2 constrained parameter video stream
    pub const MPEG2_VIDEO: StreamType = StreamType(0x02);
    /// ISO/IEC 11172-3 Audio
    pub const MPEG1_AUDIO: StreamType = StreamType(0x03);
    /// ISO/IEC 13818-3 Audio
    pub const MPEG2_AUDIO: StreamType = StreamType(0x04);
    /// ITU-T Rec. H.222.0 | ISO/IEC 13818-1 private_sections
    pub const PRIVATE_SECTIONS: StreamType = StreamType(0x05);
    /// ITU-T Rec. H.222.0 | ISO/IEC 13818-1 PES packets containing private data
    pub const PRIVATE_DATA: StreamType = StreamType(0x06);
    /// ISO/IEC 13522 MHEG
    pub const MHEG: StreamType = StreamType(0x07);
    /// ITU-T Rec. H.222.0 | ISO/IEC 13818-1 Annex A DSM-CC
    pub const DSM_CC: StreamType = StreamType(0x08);
    /// ITU-T Rec. H.222.1
    pub const ITU_T_REC_H222_1: StreamType = StreamType(0x09);
    /// ISO/IEC 13818-6 type A
    pub const ISO_IEC_13818_6_TYPE_A: StreamType = StreamType(0x0A);
    /// ISO/IEC 13818-6 type B
    pub const ISO_IEC_13818_6_TYPE_B: StreamType = StreamType(0x0B);
    /// ISO/IEC 13818-6 type C
    pub const ISO_IEC_13818_6_TYPE_C: StreamType = StreamType(0x0C);
    /// ISO/IEC 13818-6 type D
    pub const ISO_IEC_13818_6_TYPE_D: StreamType = StreamType(0x0D);
    /// ITU-T Rec. H.222.0 | ISO/IEC 13818-1 auxiliary
    pub const ISO_IEC_13818_1_AUXILIARY: StreamType = StreamType(0x0E);
    /// ISO/IEC 13818-7 Audio with ADTS transport syntax
    pub const AAC: StreamType = StreamType(0x0F);
    /// ISO/IEC 14496-2 Visual
    pub const MPEG4_VISUAL: StreamType = StreamType(0x10);
    /// ISO/IEC 14496-3 Audio with the LATM transport syntax as defined in ISO/IEC 14496-3 / AMD 1
    pub const MPEG4_AUDIO: StreamType = StreamType(0x11);
    /// ISO/IEC 14496-1 SL-packetized stream or FlexMux stream carried in PES packets
    pub const ISO_IEC_14496_1_IN_PES: StreamType = StreamType(0x12);
    /// ISO/IEC 14496-1 SL-packetized stream or FlexMux stream carried in ISO/IEC 14496_sections
    pub const ISO_IEC_14496_1_IN_SECTIONS: StreamType = StreamType(0x13);
    /// ISO/IEC 13818-6 Synchronized Download Protocol
    pub const ISO_IEC_13818_6_DOWNLOAD: StreamType = StreamType(0x14);
    /// Metadata carried in PES packets
    pub const METADATA_IN_PES: StreamType = StreamType(0x15);
    /// Metadata carried in metadata_sections
    pub const METADATA_IN_SECTIONS: StreamType = StreamType(0x16);
    /// Metadata carried in ISO/IEC 13818-6 Data Carousel
    pub const METADATA_IN_DATA_CAROUSEL: StreamType = StreamType(0x17);
    /// Metadata carried in ISO/IEC 13818-6 Object Carousel
    pub const METADATA_IN_OBJECT_CAROUSEL: StreamType = StreamType(0x18);
    /// Metadata carried in ISO/IEC 13818-6 Synchronized Download Protocol
    pub const METADATA_IN_DOWNLOAD_PROTOCOL: StreamType = StreamType(0x19);
    /// ISO/IEC 13818-11 IPMP on MPEG-2 systems
    pub const IPMP: StreamType = StreamType(0x1A);
    /// ITU-T Rec. H.264 | ISO/IEC 14496-10 Video
    pub const H264: StreamType = StreamType(0x1B);
    /// ITU-T Rec. H.265 | ISO/IEC 23008-2
    pub const H265: StreamType = StreamType(0x24);
    /// ISO/IEC User Private
    pub const USER_PRIVATE: StreamType = StreamType(0x80);
    /// Dolby AC-3
    pub const AC3: StreamType = StreamType(0x81);
    /// DTS
    pub const DTS: StreamType = StreamType(0x82);
    /// Dolby TrueHD
    pub const TRUEHD: StreamType = StreamType(0x83);
    /// Dolby Digital Plus
    pub const DOLBY_DIGITAL_PLUS: StreamType = StreamType(0x87);

    /// 未初期化
    pub const UNINITIALIZED: StreamType = StreamType(0x00);
    /// 無効
    pub const INVALID: StreamType = StreamType(0xFF);

    /// 字幕
    pub const CAPTION: StreamType = Self::PRIVATE_DATA;
    /// データ放送
    pub const DATA_CARROUSEL: StreamType = Self::ISO_IEC_13818_6_TYPE_D;

    /// 定義されているストリーム種別かどうかを返す。
    pub fn is_known(&self) -> bool {
        matches!(self.0, 0x01..=0x1B | 0x24 | 0x80..=0x83 | 0x87)
    }
}

impl fmt::Display for StreamType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = match *self {
            StreamType::MPEG2_VIDEO => "ITU-T Rec. H.262 | ISO/IEC 13818-2 Video or ISO/IEC 11172-2 constrained parameter video stream",
            StreamType::MPEG1_AUDIO => "ISO/IEC 11172-3 Audio",
            StreamType::MPEG2_AUDIO => "ISO/IEC 13818-3 Audio",
            StreamType::PRIVATE_SECTIONS => "ITU-T Rec. H.222.0 | ISO/IEC 13818-1 private_sections",
            StreamType::PRIVATE_DATA => "ITU-T Rec. H.222.0 | ISO/IEC 13818-1 PES packets containing private data",
            StreamType::MHEG => "ISO/IEC 13522 MHEG",
            StreamType::DSM_CC => "ITU-T Rec. H.222.0 | ISO/IEC 13818-1 Annex A DSM-CC",
            StreamType::ITU_T_REC_H222_1 => "ITU-T Rec. H.222.1",
            StreamType::ISO_IEC_13818_6_TYPE_A => "ISO/IEC 13818-6 type A",
            StreamType::ISO_IEC_13818_6_TYPE_B => "ISO/IEC 13818-6 type B",
            StreamType::ISO_IEC_13818_6_TYPE_C => "ISO/IEC 13818-6 type C",
            StreamType::ISO_IEC_13818_6_TYPE_D => "ISO/IEC 13818-6 type D",
            StreamType::ISO_IEC_13818_1_AUXILIARY => "ITU-T Rec. H.222.0 | ISO/IEC 13818-1 auxiliary",
            StreamType::AAC => "ISO/IEC 13818-7 Audio with ADTS transport syntax",
            StreamType::MPEG4_VISUAL => "ISO/IEC 14496-2 Visual",
            StreamType::MPEG4_AUDIO => "ISO/IEC 14496-3 Audio with the LATM transport syntax as defined in ISO/IEC 14496-3 / AMD 1",
            StreamType::ISO_IEC_14496_1_IN_PES => "ISO/IEC 14496-1 SL-packetized stream or FlexMux stream carried in PES packets",
            StreamType::ISO_IEC_14496_1_IN_SECTIONS => "ISO/IEC 14496-1 SL-packetized stream or FlexMux stream carried in ISO/IEC 14496_sections",
            StreamType::ISO_IEC_13818_6_DOWNLOAD => "ISO/IEC 13818-6 Synchronized Download Protocol",
            StreamType::METADATA_IN_PES => "Metadata carried in PES packets",
            StreamType::METADATA_IN_SECTIONS => "Metadata carried in metadata_sections",
            StreamType::METADATA_IN_DATA_CAROUSEL => "Metadata carried in ISO/IEC 13818-6 Data Carousel",
            StreamType::METADATA_IN_OBJECT_CAROUSEL => "Metadata carried in ISO/IEC 13818-6 Object Carousel",
            StreamType::METADATA_IN_DOWNLOAD_PROTOCOL => "Metadata carried in ISO/IEC 13818-6 Synchronized Download Protocol",
            StreamType::IPMP => "ISO/IEC 13818-11 IPMP on MPEG-2 systems",
            StreamType::H264 => "ITU-T Rec. H.264 | ISO/IEC 14496-10 Video",
            StreamType::H265 => "ITU-T Rec. H.265 | ISO/IEC 23008-2",
            StreamType::USER_PRIVATE => "ISO/IEC User Private",
            StreamType::AC3 => "Dolby AC-3",
            StreamType::DTS => "DTS",
            StreamType::TRUEHD => "Dolby TrueHD",
            StreamType::DOLBY_DIGITAL_PLUS => "Dolby Digital Plus",
            _ => return write!(f, "未定義（0x{:02X}）", self.0),
        };
        f.write_str(s)
    }
}
