//! ARIB STD-B10で定義されている、定数を伴う型。

/// サービス形式種別。
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

/// ストリーム形式種別。
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StreamType(pub u8);

impl StreamType {
    /// ISO/IEC 11172-2映像。
    pub const MPEG1_VIDEO: StreamType = StreamType(0x01);
    /// ITU-T勧告H.262|ISO/IEC 13818-2映像またはISO/IEC 11172-2制約パラメータ映像ストリーム。
    pub const MPEG2_VIDEO: StreamType = StreamType(0x02);
    /// ISO/IEC 11172-3音声。
    pub const MPEG1_AUDIO: StreamType = StreamType(0x03);
    /// ISO/IEC 13818-3音声。
    pub const MPEG2_AUDIO: StreamType = StreamType(0x04);
    /// ITU-T勧告H.222.0|ISO/IEC 13818-1プライベートセクション。
    pub const PRIVATE_SECTIONS: StreamType = StreamType(0x05);
    /// プライベートデータを収容したITU-T勧告H.222.0|ISO/IEC 13818-1 PESパケット。
    pub const PRIVATE_DATA: StreamType = StreamType(0x06);
    /// ISO/IEC 13522 MHEG。
    pub const MHEG: StreamType = StreamType(0x07);
    /// ITU-T勧告H.222.0|ISO/IEC 13818-1付属書A DSM-CC。
    pub const DSM_CC: StreamType = StreamType(0x08);
    /// ITU-T勧告H.222.1。
    pub const ITU_T_REC_H222_1: StreamType = StreamType(0x09);
    /// ISO/IEC 13818-6（タイプA）。
    pub const ISO_IEC_13818_6_TYPE_A: StreamType = StreamType(0x0A);
    /// ISO/IEC 13818-6（タイプB）。
    pub const ISO_IEC_13818_6_TYPE_B: StreamType = StreamType(0x0B);
    /// ISO/IEC 13818-6（タイプC）。
    pub const ISO_IEC_13818_6_TYPE_C: StreamType = StreamType(0x0C);
    /// ISO/IEC 13818-6（タイプD）。
    pub const ISO_IEC_13818_6_TYPE_D: StreamType = StreamType(0x0D);
    /// それ以外でITU-T勧告H.222.0|ISO/IEC 13818-1で規定されるデータタイプ。
    pub const ISO_IEC_13818_1_AUXILIARY: StreamType = StreamType(0x0E);
    /// ISO/IEC 13818-7音声（ADTSトランスポート構造）。
    pub const AAC: StreamType = StreamType(0x0F);
    /// ISO/IEC 14496-2映像。
    pub const MPEG4_VISUAL: StreamType = StreamType(0x10);
    /// ISO/IEC 14496-3音声（ISO/IEC 14496-3 / AMD 1で規定されるLATMトランスポート構造）。
    pub const MPEG4_AUDIO: StreamType = StreamType(0x11);
    /// PESパケットで伝送されるISO/IEC 14496-1 SLパケット化ストリームまたは
    /// フレックスマックスストリーム。
    pub const ISO_IEC_14496_1_IN_PES: StreamType = StreamType(0x12);
    /// ISO/IEC 14496セクションで伝送されるISO/IEC 14496-1 SLパケット化ストリームまたは
    /// フレックスマックスストリーム。
    pub const ISO_IEC_14496_1_IN_SECTIONS: StreamType = StreamType(0x13);
    /// ISO/IEC 13818-6同期ダウンロードプロトコル。
    pub const ISO_IEC_13818_6_DOWNLOAD: StreamType = StreamType(0x14);
    /// PESパケットで伝送されるメタデータ。
    pub const METADATA_IN_PES: StreamType = StreamType(0x15);
    /// メタデータセクションで伝送されるメタデータ。
    pub const METADATA_IN_SECTIONS: StreamType = StreamType(0x16);
    /// ISO/IEC 13818-6データカルーセルで伝送されるメタデータ。
    pub const METADATA_IN_DATA_CAROUSEL: StreamType = StreamType(0x17);
    /// ISO/IEC 13818-6オブジェクトカルーセルで伝送されるメタデータ。
    pub const METADATA_IN_OBJECT_CAROUSEL: StreamType = StreamType(0x18);
    /// ISO/IEC 13818-6同期ダウンロードプロトコルで伝送されるメタデータ。
    pub const METADATA_IN_DOWNLOAD_PROTOCOL: StreamType = StreamType(0x19);
    /// IPMPストリーム（ISO/IEC 13818-11で規定されるMPEG-2 IPMP）。
    pub const IPMP: StreamType = StreamType(0x1A);
    /// ITU-T勧告H.264|ISO/IEC 14496-10映像で規定されるAVC映像ストリーム。
    pub const H264: StreamType = StreamType(0x1B);
    /// HEVC映像ストリームまたはHEVC時間方向映像サブビットストリーム。
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

    /// 字幕。
    pub const CAPTION: StreamType = Self::PRIVATE_DATA;
    /// データ放送。
    pub const DATA_CARROUSEL: StreamType = Self::ISO_IEC_13818_6_TYPE_D;

    /// 定義されているストリーム種別かどうかを返す。
    pub fn is_known(&self) -> bool {
        matches!(self.0, 0x01..=0x1B | 0x24 | 0x80..=0x83 | 0x87)
    }
}

/// 偏波。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Polarization {
    /// 水平。
    LinearHorizontal,
    /// 垂直。
    LinearVertical,
    /// 左旋。
    CircularLeft,
    /// 右旋。
    CircularRight,
}
