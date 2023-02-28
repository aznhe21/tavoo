//! PES用のモジュール。

pub mod caption;

use thiserror::Error;

use crate::utils::{BytesExt, SliceExt};

/// ストリーム識別子。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StreamId(pub u8);

impl StreamId {
    /// プログラムストリームマップ。
    pub const PROGRAM_STREAM_MAP: StreamId = StreamId(0xBC);
    /// プライベートストリーム1。
    pub const PRIVATE_STREAM_1: StreamId = StreamId(0xBD);
    /// パディングストリーム。
    pub const PADDING_STREAM: StreamId = StreamId(0xBE);
    /// プライベートストリーム2。
    pub const PRIVATE_STREAM_2: StreamId = StreamId(0xBF);
    /// ECMストリーム。
    pub const ECM_STREAM: StreamId = StreamId(0xF0);
    /// EMMストリーム。
    pub const EMM_STREAM: StreamId = StreamId(0xF1);
    /// DSMCCストリーム。
    pub const DSMCC_STREAM: StreamId = StreamId(0xF2);
    /// ISO/IEC 13522で定義されるストリーム。
    pub const ISO_IEC_13522_STREAM: StreamId = StreamId(0xF3);
    /// ITU-T勧告H.222.1 type A。
    pub const ITU_T_REC_H222_1_TYPE_A: StreamId = StreamId(0xF4);
    /// ITU-T勧告H.222.1 type B。
    pub const ITU_T_REC_H222_1_TYPE_B: StreamId = StreamId(0xF5);
    /// ITU-T勧告H.222.1 type C。
    pub const ITU_T_REC_H222_1_TYPE_C: StreamId = StreamId(0xF6);
    /// ITU-T勧告H.222.1 type D。
    pub const ITU_T_REC_H222_1_TYPE_D: StreamId = StreamId(0xF7);
    /// ITU-T勧告H.222.1 type E。
    pub const ITU_T_REC_H222_1_TYPE_E: StreamId = StreamId(0xF8);
    /// 補助ストリーム。
    pub const ANCILLARY_STREAM: StreamId = StreamId(0xF9);
    /// SLパケット化ストリーム。
    pub const ISO_IEC_14496_1_SL_PACKETIZED_STREAM: StreamId = StreamId(0xFA);
    /// フレックスマックスストリーム。
    pub const ISO_IEC_14496_1_FLEXMUX_STREAM: StreamId = StreamId(0xFB);
    /// プログラムストリームディレクトリ。
    pub const PROGRAM_STREAM_DIRECTORY: StreamId = StreamId(0xFF);

    fn has_additional_header(self) -> bool {
        !matches!(
            self,
            StreamId::PROGRAM_STREAM_MAP
                | StreamId::PADDING_STREAM
                | StreamId::PRIVATE_STREAM_2
                | StreamId::ECM_STREAM
                | StreamId::EMM_STREAM
                | StreamId::PROGRAM_STREAM_DIRECTORY
                | StreamId::DSMCC_STREAM
                | StreamId::ITU_T_REC_H222_1_TYPE_E
        )
    }
}

/// [`PesPacket::parse`]で発生するエラー。
#[derive(Debug, Error)]
pub enum PesError {
    /// PESパケットの長さが足りない。
    #[error("insufficient length of a PES packet")]
    InsufficientLength,

    /// PESパケットのが不正。
    #[error("invalid start code")]
    InvalidStartCode,

    /// PESパケットに最低限必要なバイト数がなく、壊れたパケットである。
    #[error("corrupt section")]
    Corrupted,

    /// PESパケットのCRC16が一致しない。
    #[error("crc16 error")]
    Crc16,
}

/// PESのヘッダ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PesHeader {
    /// ストリーム識別子。
    pub stream_id: StreamId,

    /// PESヘッダオプション。
    pub option: Option<PesHeaderOption>,
}

/// PESのパケット。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PesPacket<'a> {
    /// PESのヘッダ。
    pub header: PesHeader,

    /// PESのヘッダデータ。
    pub header_data: &'a [u8],

    /// PESのデータ。
    pub data: &'a [u8],
}

impl<'a> PesPacket<'a> {
    /// PESパケットをパースして[`PesPacket`]として返す。
    pub fn parse(data: &'a [u8]) -> Result<PesPacket<'a>, PesError> {
        if data.len() < 6 {
            return Err(PesError::InsufficientLength);
        }

        if data[0..=2] != [0x00, 0x00, 0x01] {
            return Err(PesError::InvalidStartCode);
        }
        let pes_packet_length = data[4..=5].read_be_16();
        let Some(data) = data.get(..6 + pes_packet_length as usize) else {
            return Err(PesError::InsufficientLength);
        };

        let stream_id = StreamId(data[3]);

        let (option, mid) = if stream_id.has_additional_header() {
            if data.len() < 9 {
                return Err(PesError::Corrupted);
            }

            let pes_scrambling_control = (data[6] & 0b00110000) >> 4;
            let pes_priority = data[6] & 0b00001000 != 0;
            let data_alignment_indicator = data[6] & 0b00000100 != 0;
            let copyright = data[6] & 0b00000010 != 0;
            let original_or_copy = data[6] & 0b00000001 != 0;
            let pts_dts_flags = (data[7] & 0b11000000) >> 6;
            let escr_flag = data[7] & 0b00100000 != 0;
            let es_rate_flag = data[7] & 0b00010000 != 0;
            let dsm_trick_mode_flag = data[7] & 0b00001000 != 0;
            let additional_copy_info_flag = data[7] & 0b00000100 != 0;
            let pes_crc_flag = data[7] & 0b00000010 != 0;
            let pes_extension_flag = data[7] & 0b00000001 != 0;
            let pes_header_data_length = data[8];

            let mid = 9 + pes_header_data_length as usize;
            if data.len() < mid {
                return Err(PesError::Corrupted);
            }

            let option = PesHeaderOption {
                pes_scrambling_control,
                pes_priority,
                data_alignment_indicator,
                copyright,
                original_or_copy,
                pts_dts_flags,
                escr_flag,
                es_rate_flag,
                dsm_trick_mode_flag,
                additional_copy_info_flag,
                pes_crc_flag,
                pes_extension_flag,
                pes_header_data_length,
            };

            (Some(option), mid)
        } else {
            (None, 6)
        };

        // `data.len() < mid`であることは確認済み
        let (header_data, data) = data.split_at(mid);

        // TODO: crc16をチェック

        let header = PesHeader { stream_id, option };

        Ok(PesPacket {
            header,
            header_data,
            data,
        })
    }
}

/// PESヘッダオプション。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PesHeaderOption {
    /// スクランブリングモード（2ビット）。
    pub pes_scrambling_control: u8,
    /// PESパケットのペイロードの優先度。
    pub pes_priority: bool,
    /// PESヘッダの直後に同期語が続くかどうか。
    pub data_alignment_indicator: bool,
    /// PESパケットのペイロードが著作権で保護されていることが定義されているかどうか。
    pub copyright: bool,
    /// PESパケットのペイロードの内容がオリジナルか、コピーかを示す。
    pub original_or_copy: bool,
    /// PESパケットヘッダにPTSフィールド、DTSフィールドが存在するかどうかを示す（2ビット）。
    pub pts_dts_flags: u8,
    /// ESCR基本及び拡張フィールドがPESパケットヘッダに存在するかどうか。
    pub escr_flag: bool,
    /// ES_rateフィールドがPESパケットヘッダに存在するかどうか。
    pub es_rate_flag: bool,
    /// trick_mode_controlフィールドがPESパケットヘッダに存在するかどうか。
    pub dsm_trick_mode_flag: bool,
    /// additional_copy_infoフィールドがPESパケットヘッダに存在するかどうか。
    pub additional_copy_info_flag: bool,
    /// CRCフィールドがPESパケットヘッダに存在するかどうか。
    pub pes_crc_flag: bool,
    /// 拡張フィールドがPESパケットヘッダに存在するかどうか。
    pub pes_extension_flag: bool,
    /// PESパケットヘッダに含まれるオプションフィールド及びスタッフィングバイトの全バイト数。
    pub pes_header_data_length: u8,
}

/// 同期型PES・非同期型に共通するPESデータ。
#[derive(Debug)]
pub struct PesData<'a> {
    /// サービスに依存する領域。
    ///
    /// PES_data_private_data_byte。
    pub private_data: &'a [u8],
    /// 伝送するデータ。
    ///
    /// Synchronized_PES_data_byte / Asynchronous_PES_data_byte。
    pub pes_data: &'a [u8],
}

/// ARIB STD-B24第三編で規定される独立PES。
#[derive(Debug)]
pub enum IndependentPes<'a> {
    /// 同期型PES。
    Sync(PesData<'a>),

    /// 非同期型PES。
    Async(PesData<'a>),
}

impl<'a> IndependentPes<'a> {
    /// `data`から`IndependentPes`を読み取る。
    pub fn read(data: &'a [u8]) -> Option<IndependentPes<'a>> {
        if data.len() < 3 {
            log::debug!("invalid IndependentPes");
            return None;
        }

        let is_sync = match data[0] {
            0x80 => true,
            0x81 => false,
            _ => {
                log::debug!("invalid IndependentPes::data_identifier");
                return None;
            }
        };
        if data[1] != 0xFF {
            log::debug!("invalid IndependentPes::private_stream_id");
            return None;
        }

        // PES_data_packet_header_length
        let header_length = data[2] & 0b00001111;
        let Some((private_data, pes_data)) = data[3..].split_at_checked(header_length as usize)
        else {
            log::debug!("invalid IndependentPes::private_data");
            return None;
        };

        let data = PesData {
            private_data,
            pes_data,
        };

        Some(if is_sync {
            IndependentPes::Sync(data)
        } else {
            IndependentPes::Async(data)
        })
    }

    /// 同期・非同期共通のPESデータを取得する。
    #[inline]
    pub fn data(&self) -> &PesData<'a> {
        match self {
            IndependentPes::Sync(data) => data,
            IndependentPes::Async(data) => data,
        }
    }
}
