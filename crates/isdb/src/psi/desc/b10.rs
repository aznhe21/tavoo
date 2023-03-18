//! ARIB STD-B10で規定される記述子と関連する型の定義。

use std::fmt;

use crate::eight::str::AribStr;
use crate::lang::LangCode;
use crate::pid::Pid;
use crate::time::{DateTime, MjdDate};
use crate::utils::{BytesExt, SliceExt};

use super::super::table::{EventId, NetworkId, ServiceId, TransportStreamId};
use super::base::Descriptor;
use super::iso::ServiceType;

/// ストリーム形式種別。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
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

    /// ストリーム形式が映像を示す場合に`true`を返す。
    pub fn is_video(&self) -> bool {
        matches!(
            *self,
            StreamType::MPEG1_VIDEO
                | StreamType::MPEG2_VIDEO
                | StreamType::MPEG4_VISUAL
                | StreamType::H264
                | StreamType::H265
        )
    }

    /// ストリーム形式が音声を示す場合に`true`を返す。
    pub fn is_audio(&self) -> bool {
        matches!(
            *self,
            StreamType::MPEG1_AUDIO
                | StreamType::MPEG2_AUDIO
                | StreamType::AAC
                | StreamType::MPEG4_AUDIO
                | StreamType::AC3
                | StreamType::DTS
                | StreamType::TRUEHD
                | StreamType::DOLBY_DIGITAL_PLUS
        )
    }
}

impl fmt::Debug for StreamType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "StreamType(0x{:02X})", self.0)
    }
}

/// ネットワーク名記述子。
#[derive(Debug, PartialEq, Eq)]
pub struct NetworkNameDescriptor<'a> {
    /// ネットワーク名
    pub network_name: &'a AribStr,
}

impl<'a> Descriptor<'a> for NetworkNameDescriptor<'a> {
    const TAG: u8 = 0x40;

    fn read(data: &'a [u8]) -> Option<NetworkNameDescriptor<'a>> {
        Some(NetworkNameDescriptor {
            network_name: AribStr::from_bytes(data),
        })
    }
}

/// 有線分配システム記述子。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CableDeliverySystemDescriptor {
    /// 周波数（単位は100Hz）。
    pub frequency: u32,
    /// 多重フレーム形式番号（4ビット）。
    pub frame_type: u8,
    /// FEC（外側、4ビット）。
    pub fec_outer: u8,
    /// 変調。
    pub modulation: u8,
    /// シンボルレート（28ビット）。
    pub symbol_rate: u32,
    /// FEC（内側、4ビット）。
    pub fec_inner: u8,
}

impl Descriptor<'_> for CableDeliverySystemDescriptor {
    const TAG: u8 = 0x44;

    fn read(data: &[u8]) -> Option<CableDeliverySystemDescriptor> {
        if data.len() != 11 {
            log::debug!("invalid CableDeliverySystemDescriptor");
            return None;
        }

        let frequency = data[0..=3].read_bcd(8);
        let frame_type = (data[5] & 0b11110000) >> 4;
        let fec_outer = data[5] & 0b00001111;
        let modulation = data[6];
        let symbol_rate = data[7..=10].read_bcd(7);
        let fec_inner = data[10] & 0b00001111;

        Some(CableDeliverySystemDescriptor {
            frequency,
            frame_type,
            fec_outer,
            modulation,
            symbol_rate,
            fec_inner,
        })
    }
}

/// サービス記述子。
#[derive(Debug, PartialEq, Eq)]
pub struct ServiceDescriptor<'a> {
    /// サービス形式種別。
    pub service_type: ServiceType,
    /// 事業者名。
    pub service_provider_name: &'a AribStr,
    /// サービス名。
    pub service_name: &'a AribStr,
}

impl<'a> Descriptor<'a> for ServiceDescriptor<'a> {
    const TAG: u8 = 0x48;

    fn read(data: &'a [u8]) -> Option<ServiceDescriptor<'a>> {
        let [service_type, service_provider_name_length, ref data @ ..] = *data else {
            log::debug!("invalid ServiceDescriptor");
            return None;
        };
        let Some((service_provider_name, data)) = data
            .split_at_checked(service_provider_name_length as usize)
        else {
            log::debug!("invalid ServiceDescriptor::service_provider_name");
            return None;
        };
        let [service_name_length, ref service_name @ ..] = *data else {
            log::debug!("invalid ServiceDescriptor::service_name_length");
            return None;
        };
        if service_name.len() != service_name_length as usize {
            log::debug!("invalid ServiceDescriptor::service_name");
            return None;
        }

        Some(ServiceDescriptor {
            service_type: ServiceType(service_type),
            service_provider_name: AribStr::from_bytes(service_provider_name),
            service_name: AribStr::from_bytes(service_name),
        })
    }
}

/// リンク記述子。
#[derive(Debug, PartialEq, Eq)]
pub struct LinkageDescriptor<'a> {
    /// トランスポートストリーム識別。
    pub transport_stream_id: TransportStreamId,
    /// オリジナルネットワーク識別。
    pub original_network_id: NetworkId,
    /// サービス識別。
    pub service_id: ServiceId,
    /// リンク種別（4ビット）。
    pub linkage_type: u8,
    /// プライベートデータ。
    pub private_data: &'a [u8],
}

impl<'a> Descriptor<'a> for LinkageDescriptor<'a> {
    const TAG: u8 = 0x4A;

    fn read(data: &'a [u8]) -> Option<LinkageDescriptor<'a>> {
        if data.len() < 7 {
            log::debug!("invalid LinkageDescriptor");
            return None;
        }

        let Some(transport_stream_id) = TransportStreamId::new(data[0..=1].read_be_16()) else {
            log::debug!("invalid LinkageDescriptor::transport_stream_id");
            return None;
        };
        let Some(original_network_id) = NetworkId::new(data[2..=3].read_be_16()) else {
            log::debug!("invalid LinkageDescriptor::original_network_id");
            return None;
        };
        let Some(service_id) = ServiceId::new(data[4..=5].read_be_16()) else {
            log::debug!("invalid LinkageDescriptor::service_id");
            return None;
        };
        let linkage_type = data[6];
        let private_data = &data[7..];

        Some(LinkageDescriptor {
            transport_stream_id,
            original_network_id,
            service_id,
            linkage_type,
            private_data,
        })
    }
}

/// 短形式イベント記述子。
#[derive(Debug, PartialEq, Eq)]
pub struct ShortEventDescriptor<'a> {
    /// 言語コード。
    pub lang_code: LangCode,
    /// 番組名。
    pub event_name: &'a AribStr,
    /// 番組記述。
    pub text: &'a AribStr,
}

impl<'a> Descriptor<'a> for ShortEventDescriptor<'a> {
    const TAG: u8 = 0x4D;

    fn read(data: &'a [u8]) -> Option<ShortEventDescriptor<'a>> {
        if data.len() < 4 {
            log::debug!("invalid ShortEventDescriptor");
            return None;
        }

        let lang_code = LangCode(data[0..=2].try_into().unwrap());
        let event_name_length = data[3];
        let Some((event_name, data)) = data[4..].split_at_checked(event_name_length as usize) else {
            log::debug!("invalid ShortEventDescriptor::event_name");
            return None;
        };
        let event_name = AribStr::from_bytes(event_name);
        let [text_length, ref text @ ..] = *data else {
            log::debug!("invalid ShortEventDescriptor::text_length");
            return None;
        };
        if text.len() != text_length as usize {
            log::debug!("invalid ShortEventDescriptor::text");
            return None;
        }
        let text = AribStr::from_bytes(text);

        Some(ShortEventDescriptor {
            lang_code,
            event_name,
            text,
        })
    }
}

/// 拡張形式イベント記述子における項目。
#[derive(Debug, PartialEq, Eq)]
pub struct ExtendedEventItem<'a> {
    /// 項目名。
    pub item_description: &'a AribStr,
    /// 項目記述。
    pub item: &'a AribStr,
}

/// 拡張形式イベント記述子。
#[derive(Debug, PartialEq, Eq)]
pub struct ExtendedEventDescriptor<'a> {
    /// 記述子番号（4ビット）。
    pub descriptor_number: u8,
    /// 最終記述子番号（4ビット）。
    pub last_descriptor_number: u8,
    /// 言語コード。
    pub lang_code: LangCode,
    /// 項目を格納する配列。
    pub items: Vec<ExtendedEventItem<'a>>,
    /// 拡張記述。
    pub text: &'a AribStr,
}

impl<'a> Descriptor<'a> for ExtendedEventDescriptor<'a> {
    const TAG: u8 = 0x4E;

    fn read(data: &'a [u8]) -> Option<ExtendedEventDescriptor<'a>> {
        if data.len() < 5 {
            log::debug!("invalid ExtendedEventDescriptor");
            return None;
        }

        let descriptor_number = (data[0] & 0b11110000) >> 4;
        let last_descriptor_number = data[0] & 0b00001111;
        let lang_code = LangCode(data[1..=3].try_into().unwrap());
        let length_of_items = data[4];
        let Some((mut data, rem)) = data[5..].split_at_checked(length_of_items as usize) else {
            log::debug!("invalid ExtendedEventDescriptor::length_of_items");
            return None;
        };

        let mut items = Vec::new();
        while !data.is_empty() {
            let [item_description_length, ref rem @ ..] = *data else {
                log::debug!("invalid ExtendedEventDescriptor::item_description_length");
                return None;
            };
            let Some((item_description, rem)) = rem
                .split_at_checked(item_description_length as usize)
            else {
                log::debug!("invalid ExtendedEventDescriptor::item_description");
                return None;
            };
            let item_description = AribStr::from_bytes(item_description);

            let [item_length, ref rem @ ..] = *rem else {
                log::debug!("invalid ExtendedEventDescriptor::item_length");
                return None;
            };
            let Some((item, rem)) = rem.split_at_checked(item_length as usize) else {
                log::debug!("invalid ExtendedEventDescriptor::item");
                return None;
            };
            let item = AribStr::from_bytes(item);
            data = rem;

            items.push(ExtendedEventItem {
                item_description,
                item,
            });
        }

        let [text_length, ref text @ ..] = *rem else {
            log::debug!("invalid ExtendedEventDescriptor::text_length");
            return None;
        };
        if text.len() != text_length as usize {
            log::debug!("invalid ExtendedEventDescriptor::text");
            return None;
        }
        let text = AribStr::from_bytes(text);

        Some(ExtendedEventDescriptor {
            descriptor_number,
            last_descriptor_number,
            lang_code,
            items,
            text,
        })
    }
}

/// コンポーネント記述子。
#[derive(Debug, PartialEq, Eq)]
pub struct ComponentDescriptor<'a> {
    /// コンポーネント内容（4ビット）。
    pub stream_content: u8,
    /// コンポーネント種別。
    pub component_type: u8,
    /// コンポーネントタグ。
    pub component_tag: u8,
    /// 言語コード。
    pub lang_code: LangCode,
    /// コンポーネント記述。
    pub text: &'a AribStr,
}

impl<'a> Descriptor<'a> for ComponentDescriptor<'a> {
    const TAG: u8 = 0x50;

    fn read(data: &'a [u8]) -> Option<ComponentDescriptor<'a>> {
        if data.len() < 6 {
            log::debug!("invalid ComponentDescriptor");
            return None;
        }

        let stream_content = data[0] & 0b00001111;
        let component_type = data[1];
        let component_tag = data[2];
        let lang_code = LangCode(data[3..=5].try_into().unwrap());
        let text = AribStr::from_bytes(&data[6..]);

        Some(ComponentDescriptor {
            stream_content,
            component_type,
            component_tag,
            lang_code,
            text,
        })
    }
}

/// ストリーム識別記述子。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamIdDescriptor {
    /// コンポーネントタグ。
    pub component_tag: u8,
}

impl Descriptor<'_> for StreamIdDescriptor {
    const TAG: u8 = 0x52;

    fn read(data: &[u8]) -> Option<StreamIdDescriptor> {
        let [component_tag] = *data else {
            log::debug!("invalid StreamIdDescriptor");
            return None;
        };

        Some(StreamIdDescriptor { component_tag })
    }
}

/// コンテント分類。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentGenre {
    /// ジャンル1（4ビット）。
    pub large_genre_classification: u8,
    /// ジャンル2（4ビット）。
    pub middle_genre_classification: u8,
    /// ユーザジャンル（4ビット）。
    pub user_genre_1: u8,
    /// ユーザジャンル（4ビット）。
    pub user_genre_2: u8,
}

/// コンテント記述子。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentDescriptor {
    /// [`ContentGenre`]の配列。
    pub genres: Vec<ContentGenre>,
}

impl Descriptor<'_> for ContentDescriptor {
    const TAG: u8 = 0x54;

    fn read(data: &[u8]) -> Option<ContentDescriptor> {
        // genresは7要素以下
        if data.len() > 2 * 7 {
            log::debug!("invalid ContentDescriptor");
            return None;
        }

        let genres = data
            .chunks_exact(2)
            .map(|chunk| {
                let large_genre_classification = (chunk[0] & 0b11110000) >> 4;
                let middle_genre_classification = chunk[0] & 0b00001111;
                let user_genre_1 = (chunk[1] & 0b11110000) >> 4;
                let user_genre_2 = chunk[1] & 0b00001111;

                ContentGenre {
                    large_genre_classification,
                    middle_genre_classification,
                    user_genre_1,
                    user_genre_2,
                }
            })
            .collect();

        Some(ContentDescriptor { genres })
    }
}

/// ローカル時間オフセット。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalTimeOffsetEntry {
    /// 言語コード。
    pub country_code: LangCode,
    /// 国地域識別（6ビット）。
    pub country_region_id: u8,
    /// ローカル時間オフセット極性。
    pub local_time_offset_polarity: bool,
    /// ローカル時間オフセット。
    pub local_time_offset: u16,
    /// 変更時刻。
    pub time_of_change: DateTime,
    /// 変更後時間オフセット。
    pub next_time_offset: u16,
}

/// ローカル時間オフセット記述子。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalTimeOffsetDescriptor {
    /// ローカル時間オフセットを格納する配列。
    pub time_offsets: Vec<LocalTimeOffsetEntry>,
}

impl Descriptor<'_> for LocalTimeOffsetDescriptor {
    const TAG: u8 = 0x58;

    fn read(data: &[u8]) -> Option<LocalTimeOffsetDescriptor> {
        let time_offsets = data
            .chunks_exact(13)
            .map(|chunk| {
                let country_code = LangCode(chunk[0..=2].try_into().unwrap());
                let country_region_id = (chunk[3] & 0b11111100) >> 2;
                let local_time_offset_polarity = (chunk[3] & 0b00000001) != 0;
                let local_time_offset = chunk[4..=5].read_be_16();
                let time_of_change = DateTime::read(chunk[6..=10].try_into().unwrap());
                let next_time_offset = chunk[11..=12].read_be_16();

                LocalTimeOffsetEntry {
                    country_code,
                    country_region_id,
                    local_time_offset_polarity,
                    local_time_offset,
                    time_of_change,
                    next_time_offset,
                }
            })
            .collect();

        Some(LocalTimeOffsetDescriptor { time_offsets })
    }
}

/// 階層伝送記述子。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HierarchicalTransmissionDescriptor {
    /// 階層レベル。
    pub high_quality: bool,
    /// 参照先PID。
    pub reference_pid: Pid,
}

impl Descriptor<'_> for HierarchicalTransmissionDescriptor {
    const TAG: u8 = 0xC0;

    fn read(data: &[u8]) -> Option<HierarchicalTransmissionDescriptor> {
        if data.len() != 3 {
            log::debug!("invalid HierarchicalTransmissionDescriptor");
            return None;
        }

        let high_quality = data[0] & 0b00000001 != 0;
        let reference_pid = Pid::read(&data[1..=2]);

        Some(HierarchicalTransmissionDescriptor {
            high_quality,
            reference_pid,
        })
    }
}

/// デジタルコピー制御記述子におけるコンポーネント制御情報。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentControlEntry {
    /// コンポーネントタグ。
    pub component_tag: u8,
    /// デジタルコピー制御情報（2ビット）。
    pub digital_recording_control_data: u8,
    /// コピー制御形式情報（2ビット）。
    pub copy_control_type: u8,
    /// アナログ出力コピー制御情報（2ビット）。
    pub aps_control_data: Option<u8>,
    /// 最大伝送レート。
    pub maximum_bitrate: Option<u8>,
}

/// デジタルコピー制御記述子。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DigitalCopyControlDescriptor {
    /// デジタルコピー制御情報（2ビット）。
    pub digital_recording_control_data: u8,
    /// コピー制御形式情報（2ビット）。
    pub copy_control_type: u8,
    /// アナログ出力コピー制御情報（2ビット）。
    pub aps_control_data: Option<u8>,
    /// 最大伝送レート。
    pub maximum_bitrate: Option<u8>,
    /// 番組を構成するコンポーネントごとのデジタルコピー制御情報。
    pub component_controls: Option<Vec<ComponentControlEntry>>,
}

impl Descriptor<'_> for DigitalCopyControlDescriptor {
    const TAG: u8 = 0xC1;

    fn read(data: &[u8]) -> Option<DigitalCopyControlDescriptor> {
        if data.len() < 1 {
            log::debug!("invalid DigitalCopyControlDescriptor");
            return None;
        }

        let digital_recording_control_data = (data[0] & 0b11000000) >> 6;
        let maximum_bitrate_flag = data[0] & 0b00100000 != 0;
        let component_control_flag = data[0] & 0b00010000 != 0;
        let copy_control_type = (data[0] & 0b00001100) >> 2;
        let aps_control_data = if copy_control_type == 0b01 {
            Some(data[0] & 0b00000011)
        } else {
            None
        };

        let mut data = &data[1..];
        let maximum_bitrate = if maximum_bitrate_flag {
            let [maximum_bitrate, ref rem @ ..] = *data else {
                log::debug!("invalid DigitalCopyControlDescriptor::maximum_bitrate_flag");
                return None;
            };
            data = rem;

            Some(maximum_bitrate)
        } else {
            None
        };

        let component_controls = if component_control_flag {
            let [component_control_length, ref data @ ..] = *data else {
                log::debug!("invalid DigitalCopyControlDescriptor::component_control_length");
                return None;
            };
            let Some(mut data) = data.get(..component_control_length as usize) else {
                log::debug!("invalid DigitalCopyControlDescriptor::component_controls");
                return None;
            };

            let mut component_controls = Vec::new();
            while data.len() >= 2 {
                let component_tag = data[0];
                let digital_recording_control_data = (data[1] & 0b11000000) >> 6;
                let maximum_bitrate_flag = data[1] & 0b00100000 != 0;
                let copy_control_type = (data[1] & 0b00001100) >> 2;
                let aps_control_data = if copy_control_type == 0b01 {
                    Some(data[1] & 0b00000011)
                } else {
                    None
                };

                data = &data[2..];
                let maximum_bitrate = if maximum_bitrate_flag {
                    let [maximum_bitrate, ref rem @ ..] = *data else {
                        break;
                    };
                    data = rem;

                    Some(maximum_bitrate)
                } else {
                    None
                };

                component_controls.push(ComponentControlEntry {
                    component_tag,
                    digital_recording_control_data,
                    copy_control_type,
                    aps_control_data,
                    maximum_bitrate,
                });
            }

            Some(component_controls)
        } else {
            None
        };

        Some(DigitalCopyControlDescriptor {
            digital_recording_control_data,
            copy_control_type,
            aps_control_data,
            maximum_bitrate,
            component_controls,
        })
    }
}

/// 音声コンポーネント記述子における音質表示。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum QualityIndicator {
    /// Reserved for future use
    Reserved,
    /// Mode 1
    Mode1,
    /// Mode 2
    Mode2,
    /// Mode 3
    Mode3,
}

/// 音声コンポーネント記述子におけるサンプリング周波数。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SamplingFrequency {
    /// 予約
    Reserved,
    /// 16kHZ
    SF16k,
    /// 22.05kHZ
    SF22_05k,
    /// 24kHZ
    SF24k,
    /// 32kHZ
    SF32k,
    /// 44.1kHZ
    SF44_1k,
    /// 48kHZ
    SF48k,
}

/// 音声コンポーネント記述子。
#[derive(Debug, PartialEq, Eq)]
pub struct AudioComponentDescriptor<'a> {
    /// コンポーネント内容（4ビット）。
    pub stream_content: u8,
    /// コンポーネント種別。
    pub component_type: u8,
    /// コンポーネントタグ。
    pub component_tag: u8,
    /// ストリーム形式種別。
    pub stream_type: StreamType,
    /// サイマルキャストグループ識別。
    pub simulcast_group_tag: u8,
    /// 主コンポーネントフラグ。
    pub main_component_flag: bool,
    /// 音質表示。
    pub quality_indicator: QualityIndicator,
    /// サンプリング周波数。
    pub sampling_rate: SamplingFrequency,
    /// 言語コード。
    pub lang_code: LangCode,
    /// 言語コードその2。
    pub lang_code_2: Option<LangCode>,
    /// コンポーネント記述。
    pub text: &'a AribStr,
}

impl<'a> Descriptor<'a> for AudioComponentDescriptor<'a> {
    const TAG: u8 = 0xC4;

    fn read(data: &'a [u8]) -> Option<AudioComponentDescriptor<'a>> {
        if data.len() < 9 {
            log::debug!("invalid AudioComponentDescriptor");
            return None;
        }

        let stream_content = data[0] & 0b00001111;
        let component_type = data[1];
        let component_tag = data[2];
        let stream_type = StreamType(data[3]);
        let simulcast_group_tag = data[4];
        let es_multi_lingual_flag = (data[5] & 0b10000000) != 0;
        let main_component_flag = (data[5] & 0b01000000) != 0;
        let quality_indicator = match (data[5] & 0b00110000) >> 4 {
            0b00 => QualityIndicator::Reserved,
            0b01 => QualityIndicator::Mode1,
            0b10 => QualityIndicator::Mode2,
            0b11 => QualityIndicator::Mode3,
            _ => unreachable!(),
        };
        let sampling_rate = match (data[5] & 0b00001110) >> 1 {
            0b000 | 0b100 => SamplingFrequency::Reserved,
            0b001 => SamplingFrequency::SF16k,
            0b010 => SamplingFrequency::SF22_05k,
            0b011 => SamplingFrequency::SF24k,
            0b101 => SamplingFrequency::SF32k,
            0b110 => SamplingFrequency::SF44_1k,
            0b111 => SamplingFrequency::SF48k,
            _ => unreachable!(),
        };
        let lang_code = LangCode(data[6..=8].try_into().unwrap());

        let mut data = &data[9..];
        let lang_code_2 = if es_multi_lingual_flag {
            let Some((lang_code, rem)) = data.split_at_checked(3) else {
                log::debug!("invalid AudioComponentDescriptor::ISO_639_language_code_2");
                return None;
            };
            let lang_code = LangCode(lang_code.try_into().unwrap());
            data = rem;

            Some(lang_code)
        } else {
            None
        };

        let text = AribStr::from_bytes(data);

        Some(AudioComponentDescriptor {
            stream_content,
            component_type,
            component_tag,
            stream_type,
            simulcast_group_tag,
            main_component_flag,
            quality_indicator,
            sampling_rate,
            lang_code,
            lang_code_2,
            text,
        })
    }
}

/// ハイパーリンク記述子におけるリンク先。
#[derive(Debug, PartialEq, Eq)]
pub enum SelectorInfo<'a> {
    /// サービス。
    LinkServiceInfo(LinkServiceInfo),
    /// イベント。
    LinkEventInfo(LinkEventInfo),
    /// イベントの特定モジュール。
    LinkModuleInfo(LinkModuleInfo),
    /// コンテント。
    LinkContentInfo(LinkContentInfo),
    /// コンテントの特定モジュール。
    LinkContentModuleInfo(LinkContentModuleInfo),
    /// イベント関係テーブルのノード。
    LinkErtNodeInfo(LinkErtNodeInfo),
    /// 蓄積コンテント。
    LinkStoredContentInfo(LinkStoredContentInfo<'a>),
    /// 不明。
    Unknown(LinkUnknown<'a>),
}

/// サービス。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkServiceInfo {
    /// オリジナルネットワーク識別。
    pub original_network_id: NetworkId,
    /// トランスポートストリーム識別。
    pub transport_stream_id: TransportStreamId,
    /// サービス識別。
    pub service_id: ServiceId,
}

/// イベント。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkEventInfo {
    /// オリジナルネットワーク識別。
    pub original_network_id: NetworkId,
    /// トランスポートストリーム識別。
    pub transport_stream_id: TransportStreamId,
    /// サービス識別。
    pub service_id: ServiceId,
    /// イベント識別。
    pub event_id: EventId,
}

/// イベントの特定モジュール。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkModuleInfo {
    /// オリジナルネットワーク識別。
    pub original_network_id: NetworkId,
    /// トランスポートストリーム識別。
    pub transport_stream_id: TransportStreamId,
    /// サービス識別。
    pub service_id: ServiceId,
    /// イベント識別。
    pub event_id: EventId,
    /// コンポーネントタグ。
    pub component_tag: u8,
    /// モジュール識別。
    pub module_id: u16,
}

/// コンテント。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkContentInfo {
    /// オリジナルネットワーク識別。
    pub original_network_id: NetworkId,
    /// トランスポートストリーム識別。
    pub transport_stream_id: TransportStreamId,
    /// サービス識別。
    pub service_id: ServiceId,
    /// コンテンツ識別。
    pub content_id: u32,
}

/// コンテントの特定モジュール。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkContentModuleInfo {
    /// オリジナルネットワーク識別。
    pub original_network_id: NetworkId,
    /// トランスポートストリーム識別。
    pub transport_stream_id: TransportStreamId,
    /// サービス識別。
    pub service_id: ServiceId,
    /// コンテンツ識別。
    pub content_id: u32,
    /// コンポーネントタグ。
    pub component_tag: u8,
    /// モジュール識別。
    pub module_id: u16,
}

/// イベント関係テーブルのノード。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkErtNodeInfo {
    /// 情報提供者識別。
    pub information_provider_id: u16,
    /// イベント関係識別。
    pub event_relation_id: u16,
    /// ノード識別。
    pub node_id: u16,
}

/// 蓄積コンテント。
#[derive(Debug, PartialEq, Eq)]
pub struct LinkStoredContentInfo<'a> {
    /// URI文字
    pub uri: &'a AribStr,
}

/// 不明。
#[derive(Debug, PartialEq, Eq)]
pub struct LinkUnknown<'a> {
    /// 不明なセレクタの種類。
    pub link_destination_type: u8,
    /// 不明なセレクタのデータ。
    pub selector: &'a [u8],
}

/// ハイパーリンク記述子。
#[derive(Debug, PartialEq, Eq)]
pub struct HyperlinkDescriptor<'a> {
    /// ハイパーリンク種別。
    pub hyper_linkage_type: u8,
    /// リンク先。
    pub selector: SelectorInfo<'a>,
    /// プライベートデータ。
    pub private_data: &'a [u8],
}

impl<'a> Descriptor<'a> for HyperlinkDescriptor<'a> {
    const TAG: u8 = 0xC5;

    fn read(data: &'a [u8]) -> Option<HyperlinkDescriptor<'a>> {
        let [hyper_linkage_type, link_destination_type, selector_length, ref data @ ..] = *data
        else {
            log::debug!("invalid HyperlinkDescriptor");
            return None;
        };
        let Some((selector, private_data)) = data.split_at_checked(selector_length as usize) else {
            log::debug!("invalid HyperlinkDescriptor::selector");
            return None;
        };
        let selector = match link_destination_type {
            0x01 => {
                if selector.len() != 6 {
                    log::debug!("invalid LinkServiceInfo");
                    return None;
                }

                let Some(original_network_id) = NetworkId::new(selector[0..=1].read_be_16()) else {
                    log::debug!("invalid LinkServiceInfo::original_network_id");
                    return None;
                };
                let Some(transport_stream_id) = TransportStreamId::new(selector[2..=3].read_be_16()) else {
                    log::debug!("invalid LinkServiceInfo::transport_stream_id");
                    return None;
                };
                let Some(service_id) = ServiceId::new(selector[4..=5].read_be_16()) else {
                    log::debug!("invalid LinkServiceInfo::service_id");
                    return None;
                };

                SelectorInfo::LinkServiceInfo(LinkServiceInfo {
                    original_network_id,
                    transport_stream_id,
                    service_id,
                })
            }
            0x02 => {
                if selector.len() != 8 {
                    log::debug!("invalid LinkEventInfo");
                    return None;
                }

                let Some(original_network_id) = NetworkId::new(selector[0..=1].read_be_16()) else {
                    log::debug!("invalid LinkEventInfo::original_network_id");
                    return None;
                };
                let Some(transport_stream_id) = TransportStreamId::new(selector[2..=3].read_be_16()) else {
                    log::debug!("invalid LinkEventInfo::transport_stream_id");
                    return None;
                };
                let Some(service_id) = ServiceId::new(selector[4..=5].read_be_16()) else {
                    log::debug!("invalid LinkEventInfo::service_id");
                    return None;
                };
                let Some(event_id) = EventId::new(selector[6..=7].read_be_16()) else {
                    log::debug!("invalid LinkEventInfo::event_id");
                    return None;
                };

                SelectorInfo::LinkEventInfo(LinkEventInfo {
                    original_network_id,
                    transport_stream_id,
                    service_id,
                    event_id,
                })
            }
            0x03 => {
                if selector.len() != 11 {
                    log::debug!("invalid LinkModuleInfo");
                    return None;
                }

                let Some(original_network_id) = NetworkId::new(selector[0..=1].read_be_16()) else {
                    log::debug!("invalid LinkModuleInfo::original_network_id");
                    return None;
                };
                let Some(transport_stream_id) = TransportStreamId::new(selector[2..=3].read_be_16()) else {
                    log::debug!("invalid LinkModuleInfo::transport_stream_id");
                    return None;
                };
                let Some(service_id) = ServiceId::new(selector[4..=5].read_be_16()) else {
                    log::debug!("invalid LinkModuleInfo::service_id");
                    return None;
                };
                let Some(event_id) = EventId::new(selector[6..=7].read_be_16()) else {
                    log::debug!("invalid LinkModuleInfo::event_id");
                    return None;
                };
                let component_tag = selector[8];
                let module_id = selector[9..=10].read_be_16();

                SelectorInfo::LinkModuleInfo(LinkModuleInfo {
                    original_network_id,
                    transport_stream_id,
                    service_id,
                    event_id,
                    component_tag,
                    module_id,
                })
            }
            0x04 => {
                if selector.len() != 10 {
                    log::debug!("invalid LinkContentInfo");
                    return None;
                }

                let Some(original_network_id) = NetworkId::new(selector[0..=1].read_be_16()) else {
                    log::debug!("invalid LinkContentInfo::original_network_id");
                    return None;
                };
                let Some(transport_stream_id) = TransportStreamId::new(selector[2..=3].read_be_16()) else {
                    log::debug!("invalid LinkContentInfo::transport_stream_id");
                    return None;
                };
                let Some(service_id) = ServiceId::new(selector[4..=5].read_be_16()) else {
                    log::debug!("invalid LinkContentInfo::service_id");
                    return None;
                };
                let content_id = selector[6..=9].read_be_32();

                SelectorInfo::LinkContentInfo(LinkContentInfo {
                    original_network_id,
                    transport_stream_id,
                    service_id,
                    content_id,
                })
            }
            0x05 => {
                if selector.len() != 13 {
                    log::debug!("invalid LinkContentModuleInfo");
                    return None;
                }

                let Some(original_network_id) = NetworkId::new(selector[0..=1].read_be_16()) else {
                    log::debug!("invalid LinkContentModuleInfo::original_network_id");
                    return None;
                };
                let Some(transport_stream_id) = TransportStreamId::new(selector[2..=3].read_be_16()) else {
                    log::debug!("invalid LinkContentModuleInfo::transport_stream_id");
                    return None;
                };
                let Some(service_id) = ServiceId::new(selector[4..=5].read_be_16()) else {
                    log::debug!("invalid LinkContentModuleInfo::service_id");
                    return None;
                };
                let content_id = selector[6..=9].read_be_32();
                let component_tag = selector[10];
                let module_id = selector[11..=12].read_be_16();

                SelectorInfo::LinkContentModuleInfo(LinkContentModuleInfo {
                    original_network_id,
                    transport_stream_id,
                    service_id,
                    content_id,
                    component_tag,
                    module_id,
                })
            }
            0x06 => {
                if selector.len() != 6 {
                    log::debug!("invalid LinkErtNodeInfo");
                    return None;
                }

                let information_provider_id = selector[0..=1].read_be_16();
                let event_relation_id = selector[2..=3].read_be_16();
                let node_id = selector[4..=5].read_be_16();

                SelectorInfo::LinkErtNodeInfo(LinkErtNodeInfo {
                    information_provider_id,
                    event_relation_id,
                    node_id,
                })
            }
            0x07 => SelectorInfo::LinkStoredContentInfo(LinkStoredContentInfo {
                uri: AribStr::from_bytes(selector),
            }),
            _ => SelectorInfo::Unknown(LinkUnknown {
                link_destination_type,
                selector,
            }),
        };

        Some(HyperlinkDescriptor {
            hyper_linkage_type,
            selector,
            private_data,
        })
    }
}

/// 対象地域記述子における地域記述方式。
#[derive(Debug, PartialEq, Eq)]
pub enum TargetRegionSpec<'a> {
    /// BSデジタル用県域指定。
    BsPrefectureSpec(BsPrefectureSpec),
    /// 不明な地域記述方式。
    Unknown(&'a [u8]),
}

/// 県域指定ビットマップ。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrefectureBitmap(pub [u8; 7]);

macro_rules! prefactures {
    ($($method:ident($bit:literal) => $jp:literal,)*) => {
        $(
            #[doc = concat!($jp, "が対象であるかどうかを返す。")]
            #[inline]
            pub fn $method(&self) -> bool {
                self.0[($bit - 1) / 8] & (1 << (($bit - 1) % 8)) != 0
            }
        )*
    };
}

impl PrefectureBitmap {
    prefactures! {
        has_east_hokkaido(1) => "東北海道",
        has_west_hokkaido(2) => "西北海道",
        has_aomori(3) => "青森県",
        has_iwate(4) => "岩手県",
        has_miyagi(5) => "宮城県",
        has_akita(6) => "秋田県",
        has_yamagata(7) => "山形県",
        has_fukushima(8) => "福島県",
        has_ibaraki(9) => "茨城県",
        has_tochigi(10) => "栃木県",
        has_gunma(11) => "群馬県",
        has_saitama(12) => "埼玉県",
        has_chiba(13) => "千葉県",
        has_tokyo(14) => "東京都（島部を除く）",
        has_kanagawa(15) => "神奈川県",
        has_niigata(16) => "新潟県",
        has_toyama(17) => "富山県",
        has_ishikawa(18) => "石川県",
        has_fukui(19) => "福井県",
        has_yamanashi(20) => "山梨県",
        has_nagano(21) => "長野県",
        has_gifu(22) => "岐阜県",
        has_shizuoka(23) => "静岡県",
        has_aichi(24) => "愛知県",
        has_mie(25) => "三重県",
        has_shiga(26) => "滋賀県",
        has_kyoto(27) => "京都府",
        has_osaka(28) => "大阪府",
        has_hyogo(29) => "兵庫県",
        has_nara(30) => "奈良県",
        has_wakayama(31) => "和歌山県",
        has_tottori(32) => "鳥取県",
        has_shimane(33) => "島根県",
        has_okayama(34) => "岡山県",
        has_hiroshima(35) => "広島県",
        has_yamaguchi(36) => "山口県",
        has_tokushima(37) => "徳島県",
        has_kagawa(38) => "香川県",
        has_ehime(39) => "愛媛県",
        has_kochi(40) => "高知県",
        has_fukuoka(41) => "福岡県",
        has_saga(42) => "佐賀県",
        has_nagasaki(43) => "長崎県",
        has_kumamoto(44) => "熊本県",
        has_oita(45) => "大分県",
        has_miyazaki(46) => "宮崎県",
        has_kagoshima(47) => "鹿児島県（南西諸島を除く）",
        has_okinawa(48) => "沖縄県",
        has_tokyo_island(49) => "東京都島部（伊豆・小笠原諸島）",
        has_kagoshima_island(50) => "鹿児島県島部（南西諸島の鹿児島県域）",
    }
}

/// BSデジタル用県域指定。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BsPrefectureSpec {
    /// 県域指定ビットマップ。
    pub prefecture_bitmap: PrefectureBitmap,
}

/// 対象地域記述子。
#[derive(Debug, PartialEq, Eq)]
pub struct TargetRegionDescriptor<'a> {
    /// 地域記述方式。
    pub target_region_spec: TargetRegionSpec<'a>,
}

impl<'a> Descriptor<'a> for TargetRegionDescriptor<'a> {
    const TAG: u8 = 0xC6;

    fn read(data: &'a [u8]) -> Option<TargetRegionDescriptor<'a>> {
        let [region_spec_type, ref data @ ..] = *data else {
            log::debug!("invalid TargetRegionDescriptor");
            return None;
        };

        let target_region_spec = match region_spec_type {
            0x01 => {
                let Ok(prefecture_bitmap) = data.try_into() else {
                    log::debug!("invalid TargetRegionDescriptor::BsPrefectureSpec");
                    return None;
                };
                let prefecture_bitmap = PrefectureBitmap(prefecture_bitmap);

                TargetRegionSpec::BsPrefectureSpec(BsPrefectureSpec { prefecture_bitmap })
            }
            _ => TargetRegionSpec::Unknown(data),
        };

        Some(TargetRegionDescriptor { target_region_spec })
    }
}

/// データコンテンツ記述子。
#[derive(Debug, PartialEq, Eq)]
pub struct DataContentDescriptor<'a> {
    /// データ符号化方式識別。
    pub data_component_id: u16,
    /// エントリコンポーネント。
    pub entry_component: u8,
    /// セレクタ。
    pub selector: &'a [u8],
    /// 参照コンポーネント。
    pub component_ref: &'a [u8],
    /// 言語コード。
    pub lang_code: LangCode,
    /// コンテンツ記述。
    pub text: &'a AribStr,
}

impl<'a> Descriptor<'a> for DataContentDescriptor<'a> {
    const TAG: u8 = 0xC7;

    fn read(data: &'a [u8]) -> Option<DataContentDescriptor<'a>> {
        if data.len() < 4 {
            log::debug!("invalid DataContentDescriptor");
            return None;
        }

        let data_component_id = data[0..=1].read_be_16();
        let entry_component = data[2];
        let selector_length = data[3];
        let Some((selector, data)) = data[4..].split_at_checked(selector_length as usize) else {
            log::debug!("invalid DataContentDescriptor::selector");
            return None;
        };

        if data.len() < 1 {
            log::debug!("invalid DataContentDescriptor::num_of_component_ref");
            return None;
        }
        let num_of_component_ref = data[0];
        let Some((component_ref, data)) = data[1..].split_at_checked(num_of_component_ref as usize)
        else {
            log::debug!("invalid DataContentDescriptor::component_ref");
            return None;
        };

        if data.len() < 4 {
            log::debug!("invalid DataContentDescriptor::ISO_639_language_code");
            return None;
        }
        let lang_code = LangCode(data[0..=2].try_into().unwrap());
        let text_length = data[3];
        let text = AribStr::from_bytes(&data[4..]);
        if text.len() != text_length as usize {
            log::debug!("invalid DataContentDescriptor::text");
            return None;
        };

        Some(DataContentDescriptor {
            data_component_id,
            entry_component,
            selector,
            component_ref,
            lang_code,
            text,
        })
    }
}

/// ビデオエンコードフォーマット。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum VideoEncodeFormat {
    /// 1080/P
    Vef1080P,
    /// 1080/I
    Vef1080I,
    /// 720/P
    Vef720P,
    /// 480/P
    Vef480P,
    /// 480/I
    Vef480I,
    /// 240/P
    Vef240P,
    /// 120/P
    Vef120P,
    /// 2160/60P
    Vef2160_60P,
    /// 180P
    Vef180P,
    /// 2160/120P
    Vef2160_120P,
    /// 4320/60P
    Vef4320_60P,
    /// 4320/120P
    Vef4320_120P,
    /// 不明（拡張用）。
    Unknown,
}

impl From<u8> for VideoEncodeFormat {
    #[inline]
    fn from(value: u8) -> VideoEncodeFormat {
        match value {
            0b0000 => VideoEncodeFormat::Vef1080P,
            0b0001 => VideoEncodeFormat::Vef1080I,
            0b0010 => VideoEncodeFormat::Vef720P,
            0b0011 => VideoEncodeFormat::Vef480P,
            0b0100 => VideoEncodeFormat::Vef480I,
            0b0101 => VideoEncodeFormat::Vef240P,
            0b0110 => VideoEncodeFormat::Vef120P,
            0b0111 => VideoEncodeFormat::Vef2160_60P,
            0b1000 => VideoEncodeFormat::Vef180P,
            0b1001 => VideoEncodeFormat::Vef2160_120P,
            0b1010 => VideoEncodeFormat::Vef4320_60P,
            0b1011 => VideoEncodeFormat::Vef2160_120P,
            _ => VideoEncodeFormat::Unknown,
        }
    }
}

/// ビデオデコードコントロール記述子。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoDecodeControlDescriptor {
    /// 静止画フラグ。
    pub still_picture_flag: bool,
    /// シーケンスエンドコードフラグ。
    pub sequence_end_code_flag: bool,
    /// ビデオエンコードフォーマット。
    pub video_encode_format: VideoEncodeFormat,
}

impl Descriptor<'_> for VideoDecodeControlDescriptor {
    const TAG: u8 = 0xC8;

    fn read(data: &[u8]) -> Option<VideoDecodeControlDescriptor> {
        if data.len() != 1 {
            log::debug!("invalid VideoDecodeControlDescriptor");
            return None;
        }

        let still_picture_flag = data[0] & 0b10000000 != 0;
        let sequence_end_code_flag = data[0] & 0b01000000 != 0;
        let video_encode_format = ((data[0] & 0b00111100) >> 2).into();

        Some(VideoDecodeControlDescriptor {
            still_picture_flag,
            sequence_end_code_flag,
            video_encode_format,
        })
    }
}

/// TS情報記述子における伝送種別。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TsInformationTransmissionType {
    /// 伝承種別情報。
    pub transmission_type_info: u8,
    /// サービス識別。
    pub service_ids: Vec<u16>,
}

/// TS情報記述子。
#[derive(Debug, PartialEq, Eq)]
pub struct TsInformationDescriptor<'a> {
    /// リモコンキー識別。
    pub remote_control_key_id: u8,
    /// TS名記述。
    pub ts_name: &'a AribStr,
    /// 伝送種別を格納する配列。
    pub transmission_types: Vec<TsInformationTransmissionType>,
}

impl<'a> Descriptor<'a> for TsInformationDescriptor<'a> {
    const TAG: u8 = 0xCD;

    fn read(data: &'a [u8]) -> Option<TsInformationDescriptor<'a>> {
        if data.len() < 3 {
            log::debug!("invalid TsInformationDescriptor");
            return None;
        }

        let remote_control_key_id = data[0];
        let length_of_ts_name = (data[1] & 0b11111100) >> 2;
        let transmission_type_count = data[1] & 0b00000011;
        let Some((ts_name, mut data)) = data[2..].split_at_checked(length_of_ts_name as usize)
        else {
            log::debug!("invalid TsInformationDescriptor::ts_name");
            return None;
        };
        let ts_name = AribStr::from_bytes(ts_name);

        let mut transmission_types = Vec::with_capacity(transmission_type_count as usize);
        for _ in 0..transmission_type_count {
            if data.len() < 2 {
                log::debug!("invalid TsInformationTransmission");
                return None;
            }

            let transmission_type_info = data[0];
            let num_of_service = data[1];
            let Some((service_ids, rem)) = data[2..].split_at_checked(num_of_service as usize)
            else {
                log::debug!("invalid TsInformationTransmission::service_ids");
                return None;
            };
            let service_ids = service_ids
                .chunks_exact(2)
                .map(<[u8]>::read_be_16)
                .collect();
            data = rem;

            transmission_types.push(TsInformationTransmissionType {
                transmission_type_info,
                service_ids,
            });
        }

        Some(TsInformationDescriptor {
            remote_control_key_id,
            ts_name,
            transmission_types,
        })
    }
}

/// 拡張ブロードキャスタ識別におけるブロードキャスタ識別。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BroadcasterId {
    /// オリジナルネットワーク識別。
    pub original_network_id: NetworkId,
    /// ブロードキャスタ識別。
    pub broadcaster_id: u8,
}

/// 地上デジタルテレビジョン放送。
#[derive(Debug, PartialEq, Eq)]
pub struct DigitalTerrestrialTelevisionBroadcast<'a> {
    /// 地上ブロードキャスタ識別。
    pub terrestrial_broadcaster_id: u16,
    /// 系列識別。
    pub affiliation_id: &'a [u8],
    /// ブロードキャスタ識別を格納する配列。
    pub broadcaster_ids: Vec<BroadcasterId>,
    /// プライベートデータ。
    pub private_data: &'a [u8],
}

/// 地上デジタル音声放送。
#[derive(Debug, PartialEq, Eq)]
pub struct DigitalTerrestrialSoundBroadcast<'a> {
    /// 地上音声ブロードキャスタ識別。
    pub terrestrial_sound_broadcaster_id: u16,
    /// 音声放送系列識別。
    pub sound_broadcast_affiliation_id: &'a [u8],
    /// ブロードキャスタ識別を格納する配列。
    pub broadcaster_ids: Vec<BroadcasterId>,
    /// プライベートデータ。
    pub private_data: &'a [u8],
}

/// 拡張ブロードキャスタ記述子。
#[derive(Debug, PartialEq, Eq)]
pub enum ExtendedBroadcasterDescriptor<'a> {
    /// 地上デジタルテレビジョン放送。
    DigitalTerrestrialTelevisionBroadcast(DigitalTerrestrialTelevisionBroadcast<'a>),
    /// 地上デジタル音声放送。
    DigitalTerrestrialSoundBroadcast(DigitalTerrestrialSoundBroadcast<'a>),
    /// 未定義。
    Unknown(&'a [u8]),
}

impl<'a> Descriptor<'a> for ExtendedBroadcasterDescriptor<'a> {
    const TAG: u8 = 0xCE;

    fn read(data: &'a [u8]) -> Option<ExtendedBroadcasterDescriptor<'a>> {
        fn read_broadcaster_ids(broadcaster_ids: &[u8]) -> Option<Vec<BroadcasterId>> {
            broadcaster_ids
                .chunks_exact(3)
                .map(|chunk| {
                    let Some(original_network_id) = NetworkId::new(chunk[0..=1].read_be_16()) else {
                        log::debug!("invalid BroadcasterId::original_network_id");
                        return None;
                    };
                    let broadcaster_id = chunk[2];

                    Some(BroadcasterId {
                        original_network_id,
                        broadcaster_id,
                    })
                })
                .collect()
        }

        if data.len() < 1 {
            log::debug!("invalid ExtendedBroadcasterDescriptor");
            return None;
        }

        let broadcaster_type = (data[0] & 0b11110000) >> 4;
        let data = &data[1..];

        let descriptor = match broadcaster_type {
            0x1 => {
                if data.len() < 3 {
                    log::debug!("invalid DigitalTerrestrialTelevisionBroadcast");
                    return None;
                }

                let terrestrial_broadcaster_id = data[0..=1].read_be_16();
                let number_of_affiliation_id_loop = (data[2] & 0b11110000) >> 4;
                let number_of_broadcaster_id_loop = data[2] & 0b00001111;

                let Some((affiliation_id, data)) = data[3..]
                    .split_at_checked(number_of_affiliation_id_loop as usize)
                else {
                    log::debug!("invalid DigitalTerrestrialTelevisionBroadcast::affiliation_id");
                    return None;
                };
                let Some((broadcaster_ids, data)) = data
                    .split_at_checked(3 * number_of_broadcaster_id_loop as usize)
                else {
                    log::debug!("invalid DigitalTerrestrialTelevisionBroadcast::broadcaster_ids");
                    return None;
                };
                let broadcaster_ids = read_broadcaster_ids(broadcaster_ids)?;
                let private_data = data;

                ExtendedBroadcasterDescriptor::DigitalTerrestrialTelevisionBroadcast(
                    DigitalTerrestrialTelevisionBroadcast {
                        terrestrial_broadcaster_id,
                        affiliation_id,
                        broadcaster_ids,
                        private_data,
                    },
                )
            }
            0x2 => {
                if data.len() < 3 {
                    log::debug!("invalid DigitalTerrestrialSoundBroadcast");
                    return None;
                }

                let terrestrial_sound_broadcaster_id = data[0..=1].read_be_16();
                let number_of_sound_broadcast_affiliation_id_loop = (data[2] & 0b11110000) >> 4;
                let number_of_broadcaster_id_loop = data[2] & 0b00001111;

                let Some((sound_broadcast_affiliation_id, data)) = data[3..]
                    .split_at_checked(number_of_sound_broadcast_affiliation_id_loop as usize)
                else {
                    log::debug!("invalid DigitalTerrestrialSoundBroadcast::sound_broadcast_affiliation_id");
                    return None;
                };
                let Some((broadcaster_ids, data)) = data
                    .split_at_checked(3 * number_of_broadcaster_id_loop as usize)
                else {
                    log::debug!("invalid DigitalTerrestrialSoundBroadcast::broadcaster_ids");
                    return None;
                };
                let broadcaster_ids = read_broadcaster_ids(broadcaster_ids)?;
                let private_data = data;

                ExtendedBroadcasterDescriptor::DigitalTerrestrialSoundBroadcast(
                    DigitalTerrestrialSoundBroadcast {
                        terrestrial_sound_broadcaster_id,
                        sound_broadcast_affiliation_id,
                        broadcaster_ids,
                        private_data,
                    },
                )
            }
            _ => ExtendedBroadcasterDescriptor::Unknown(data),
        };
        Some(descriptor)
    }
}

/// CDT伝送方式1。
///
/// CDTをダウンロードデータ識別で直接参照する場合。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogoTransmissionCdt1 {
    /// ロゴ識別（9ビット）。
    pub logo_id: u16,
    /// ロゴバージョン番号（12ビット）。
    pub logo_version: u16,
    /// ダウンロードデータ識別。
    pub download_data_id: u16,
}

/// CDT伝送方式2．
///
/// CDTをロゴ識別を用いてダウンロードデータ識別を間接的に参照する場合。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogoTransmissionCdt2 {
    /// ロゴ識別（9ビット）。
    pub logo_id: u16,
}

/// ロゴ伝送記述子。
#[derive(Debug, PartialEq, Eq)]
pub enum LogoTransmissionDescriptor<'a> {
    /// CDT伝送方式1。
    Cdt1(LogoTransmissionCdt1),
    /// CDT伝送方式2．
    Cdt2(LogoTransmissionCdt2),
    /// 簡易ロゴ方式。
    Simple(&'a [u8]),
    /// 予約。
    Unknown(&'a [u8]),
}

impl<'a> Descriptor<'a> for LogoTransmissionDescriptor<'a> {
    const TAG: u8 = 0xCF;

    fn read(data: &'a [u8]) -> Option<LogoTransmissionDescriptor<'a>> {
        let [logo_transmission_type, ref data @ ..] = *data else {
            log::debug!("invalid LogoTransmissionDescriptor");
            return None;
        };

        let descriptor = match logo_transmission_type {
            0x01 => {
                if data.len() != 6 {
                    log::debug!("invalid LogoTransmissionCdt1");
                    return None;
                }

                let logo_id = data[0..=1].read_be_16() & 0b0000_0001_1111_1111;
                let logo_version = data[2..=3].read_be_16() & 0b0000_1111_1111_1111;
                let download_data_id = data[4..=5].read_be_16();

                LogoTransmissionDescriptor::Cdt1(LogoTransmissionCdt1 {
                    logo_id,
                    logo_version,
                    download_data_id,
                })
            }
            0x02 => {
                if data.len() != 2 {
                    log::debug!("invalid LogoTransmissionCdt2");
                    return None;
                }

                let logo_id = data[0..=1].read_be_16() & 0b0000_0001_1111_1111;

                LogoTransmissionDescriptor::Cdt2(LogoTransmissionCdt2 { logo_id })
            }
            0x03 => LogoTransmissionDescriptor::Simple(data),
            _ => LogoTransmissionDescriptor::Unknown(data),
        };
        Some(descriptor)
    }
}

/// シリーズ記述子における編成パターン。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProgramPattern {
    /// 不定期。
    Nonscheduled,
    /// 帯番組（毎日、平日のみ毎日、土・日のみなど）、週に複数回の編成。
    Regular,
    /// 週に1回程度の編成。
    OnceAWeek,
    /// 月に1回程度の編成。
    OnceAMonth,
    /// 同日内に複数話数の編成。
    SeveralEventsInADay,
    /// 長時間番組の分割。
    Division,
    /// 定期または不定期の蓄積用の編成。
    Accumulation,
    /// 未定義。
    Undefined,
}

/// シリーズ記述子。
#[derive(Debug, PartialEq, Eq)]
pub struct SeriesDescriptor<'a> {
    /// シリーズ識別。
    pub series_id: u16,
    /// 再放送ラベル（4ビット）。
    pub repeat_label: u8,
    /// 編成パターン。
    pub program_pattern: ProgramPattern,
    /// 有効期限。
    pub expire_date: Option<MjdDate>,
    /// 話数（12ビット）。
    pub episode_number: u16,
    /// 番組総数（12ビット）。
    pub last_episode_number: u16,
    /// シリーズ名。
    pub series_name: &'a AribStr,
}

impl<'a> Descriptor<'a> for SeriesDescriptor<'a> {
    const TAG: u8 = 0xD5;

    fn read(data: &'a [u8]) -> Option<SeriesDescriptor<'a>> {
        if data.len() < 8 {
            log::debug!("invalid SeriesDescriptor");
            return None;
        }

        let series_id = data[0..=1].read_be_16();
        let repeat_label = (data[2] & 0b11110000) >> 4;
        let program_pattern = match (data[2] & 0b00001110) >> 1 {
            0x0 => ProgramPattern::Nonscheduled,
            0x1 => ProgramPattern::Regular,
            0x2 => ProgramPattern::OnceAWeek,
            0x3 => ProgramPattern::OnceAMonth,
            0x4 => ProgramPattern::SeveralEventsInADay,
            0x5 => ProgramPattern::Division,
            0x6 => ProgramPattern::Accumulation,
            0x7 => ProgramPattern::Undefined,
            _ => unreachable!(),
        };
        let expire_date_valid_flag = data[2] & 0b00000001 != 0;
        let expire_date =
            expire_date_valid_flag.then(|| MjdDate::read(&data[3..=4].try_into().unwrap()));
        let episode_number = data[5..=6].read_be_16() >> 4; // 12bit
        let last_episode_number = data[6..=7].read_be_16() & 0b0000_1111_1111_1111; // 12bit
        let series_name = AribStr::from_bytes(&data[8..]);

        Some(SeriesDescriptor {
            series_id,
            repeat_label,
            program_pattern,
            expire_date,
            episode_number,
            last_episode_number,
            series_name,
        })
    }
}

/// イベントグループ記述子におけるイベント。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActualEvent {
    /// サービス識別。
    pub service_id: ServiceId,
    /// イベント識別。
    pub event_id: EventId,
}

/// イベントグループ記述子における`RelayToOtherNetworks`か`MovementFromOtherNetworks`に入る値。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtherNetwork {
    /// オリジナルネットワーク識別。
    pub original_network_id: NetworkId,
    /// トランスポートストリーム識別。
    pub transport_stream_id: TransportStreamId,
    /// サービス識別。
    pub service_id: ServiceId,
    /// イベント識別。
    pub event_id: EventId,
}

/// イベントグループ記述子におけるグループ。
#[derive(Debug, PartialEq, Eq)]
pub enum EventGroup<'a> {
    /// イベント共有。
    Common(&'a [u8]),
    /// イベントリレー。
    Relay(&'a [u8]),
    /// イベント移動。
    Movement(&'a [u8]),
    /// 他ネットワークへのイベントリレー。
    RelayToOtherNetworks(Vec<OtherNetwork>),
    /// 他ネットワークからのイベント移動。
    MovementFromOtherNetworks(Vec<OtherNetwork>),
    /// 未定義。
    Undefined(&'a [u8]),
}

/// イベントグループ記述子。
#[derive(Debug, PartialEq, Eq)]
pub struct EventGroupDescriptor<'a> {
    /// イベントを格納する配列。
    pub events: Vec<ActualEvent>,
    /// group_type + ...
    pub group: EventGroup<'a>,
}

impl<'a> Descriptor<'a> for EventGroupDescriptor<'a> {
    const TAG: u8 = 0xD6;

    fn read(data: &'a [u8]) -> Option<EventGroupDescriptor<'a>> {
        fn read_other_networks(data: &[u8]) -> Option<Vec<OtherNetwork>> {
            data.chunks_exact(8)
                .map(|chunk| {
                    let Some(original_network_id) = NetworkId::new(chunk[0..=1].read_be_16())
                    else {
                        log::debug!("invalid OtherNetwork::original_network_id");
                        return None;
                    };
                    let Some(transport_stream_id) =
                        TransportStreamId::new(chunk[2..=3].read_be_16())
                    else {
                        log::debug!("invalid OtherNetwork::transport_stream_id");
                        return None;
                    };
                    let Some(service_id) = ServiceId::new(chunk[4..=5].read_be_16()) else {
                        log::debug!("invalid OtherNetwork::service_id");
                        return None;
                    };
                    let Some(event_id) = EventId::new(chunk[6..=7].read_be_16()) else {
                        log::debug!("invalid OtherNetwork::event_id");
                        return None;
                    };

                    Some(OtherNetwork {
                        original_network_id,
                        transport_stream_id,
                        service_id,
                        event_id,
                    })
                })
                .collect()
        }

        if data.len() < 1 {
            log::debug!("invalid EventGroupDescriptor");
            return None;
        }

        let group_type = (data[0] & 0b11110000) >> 4;
        let event_count = data[0] & 0b00001111;
        let Some((events, data)) = data[1..].split_at_checked(4 * event_count as usize) else {
            log::debug!("invalid EventGroupDescriptor::events");
            return None;
        };
        let events = events
            .chunks_exact(4)
            .map(|chunk| {
                let Some(service_id) = ServiceId::new(chunk[0..=1].read_be_16()) else {
                    log::debug!("invalid ActualEvent::service_id");
                    return None;
                };
                let Some(event_id) = EventId::new(chunk[2..=3].read_be_16()) else {
                    log::debug!("invalid ActualEvent::event_id");
                    return None;
                };

                Some(ActualEvent {
                    service_id,
                    event_id,
                })
            })
            .collect::<Option<_>>()?;

        let group = match group_type {
            0x1 => EventGroup::Common(data),
            0x2 => EventGroup::Relay(data),
            0x3 => EventGroup::Movement(data),
            0x4 => EventGroup::RelayToOtherNetworks(read_other_networks(data)?),
            0x5 => EventGroup::MovementFromOtherNetworks(read_other_networks(data)?),
            _ => EventGroup::Undefined(data),
        };

        Some(EventGroupDescriptor { events, group })
    }
}

/// SI伝送パラメータ記述子におけるテーブル。
#[derive(Debug, PartialEq, Eq)]
pub struct SiParameterTable<'a> {
    /// テーブル識別。
    pub table_id: u8,
    /// テーブル記述。
    pub table_description: &'a [u8],
}

/// SI伝送パラメータ記述子。
#[derive(Debug, PartialEq, Eq)]
pub struct SiParameterDescriptor<'a> {
    /// パラメータバージョン。
    pub parameter_version: u8,
    /// 更新日。
    pub update_time: MjdDate,
    /// テーブルを格納する配列。
    pub tables: Vec<SiParameterTable<'a>>,
}

impl<'a> Descriptor<'a> for SiParameterDescriptor<'a> {
    const TAG: u8 = 0xD7;

    fn read(data: &'a [u8]) -> Option<SiParameterDescriptor<'a>> {
        if data.len() < 3 {
            log::debug!("invalid SiParameterDescriptor");
            return None;
        }

        let parameter_version = data[0];
        let update_time = MjdDate::read(&data[1..=2].try_into().unwrap());
        let mut data = &data[3..];

        let mut tables = Vec::new();
        while !data.is_empty() {
            let [table_id, table_description_length, ref rem @ ..] = *data else {
                log::debug!("invalid SiParameterDescriptor::table_id");
                return None;
            };
            let Some((table_description, rem)) = rem
                .split_at_checked(table_description_length as usize)
            else {
                log::debug!("invalid SiParameterDescriptor::table_description");
                return None;
            };
            data = rem;

            tables.push(SiParameterTable {
                table_id,
                table_description,
            });
        }

        Some(SiParameterDescriptor {
            parameter_version,
            update_time,
            tables,
        })
    }
}

/// ブロードキャスタ名記述子。
#[derive(Debug, PartialEq, Eq)]
pub struct BroadcasterNameDescriptor<'a> {
    /// ブロードキャスタ名。
    pub broadcaster_name: &'a AribStr,
}

impl<'a> Descriptor<'a> for BroadcasterNameDescriptor<'a> {
    const TAG: u8 = 0xD8;

    fn read(data: &'a [u8]) -> Option<BroadcasterNameDescriptor<'a>> {
        Some(BroadcasterNameDescriptor {
            broadcaster_name: AribStr::from_bytes(data),
        })
    }
}

/// コンポーネントグループ記述子における課金単位。
#[derive(Debug, PartialEq, Eq)]
pub struct CaUnit<'a> {
    /// 課金単位識別。
    pub ca_unit_id: u8,
    /// コンポーネントタグ。
    pub component_tag: &'a [u8],
}

/// コンポーネントグループ記述子におけるグループ。
#[derive(Debug, PartialEq, Eq)]
pub struct ComponentGroup<'a> {
    /// コンポーネントグループ識別（4ビット）。
    pub component_group_id: u8,
    /// 課金単位を格納する配列。
    pub ca_units: Vec<CaUnit<'a>>,
    /// トータルビットレート。
    pub total_bit_rate: Option<u8>,
    /// コンポーネントグループ記述。
    pub text: &'a AribStr,
}

/// コンポーネントグループ記述子。
#[derive(Debug, PartialEq, Eq)]
pub struct ComponentGroupDescriptor<'a> {
    /// コンポーネントグループ種別（3ビット）。
    pub component_group_type: u8,
    /// コンポーネントグループを格納する配列。
    pub groups: Vec<ComponentGroup<'a>>,
}

impl<'a> Descriptor<'a> for ComponentGroupDescriptor<'a> {
    const TAG: u8 = 0xD9;

    fn read(data: &'a [u8]) -> Option<ComponentGroupDescriptor<'a>> {
        if data.len() < 1 {
            log::debug!("invalid ComponentGroupDescriptor");
            return None;
        }

        let component_group_type = (data[0] & 0b11100000) >> 5;
        let total_bit_rate_flag = data[0] & 0b00010000 != 0;
        let num_of_group = data[0] & 0b00001111;
        let mut data = &data[1..];

        let mut groups = Vec::with_capacity(num_of_group as usize);
        for _ in 0..num_of_group {
            if data.len() < 1 {
                log::debug!("invalid ComponentGroup");
                return None;
            }

            let component_group_id = (data[0] & 0b11110000) >> 4;
            let num_of_ca_unit = data[0] & 0b00001111;
            data = &data[1..];

            let mut ca_units = Vec::with_capacity(num_of_ca_unit as usize);
            for _ in 0..num_of_ca_unit {
                if data.len() < 1 {
                    log::debug!("invalid CaUnit");
                    return None;
                }

                let ca_unit_id = (data[0] & 0b11110000) >> 4;
                let num_of_component = data[0] & 0b00001111;
                let Some((component_tag, rem)) = data.split_at_checked(num_of_component as usize)
                else {
                    log::debug!("invalid CaUnit::component_tag");
                    return None;
                };
                data = rem;

                ca_units.push(CaUnit {
                    ca_unit_id,
                    component_tag,
                });
            }

            let total_bit_rate = if total_bit_rate_flag {
                let [total_bit_rate, ref rem @ ..] = *data else {
                    log::debug!("invalid ComponentGroup::total_bit_rate");
                    return None;
                };
                data = rem;

                Some(total_bit_rate)
            } else {
                None
            };

            let [text_length, ref rem @ ..] = *data else {
                log::debug!("invalid ComponentGroup::text_length");
                return None;
            };
            let Some((text, rem)) = rem.split_at_checked(text_length as usize) else {
                log::debug!("invalid ComponentGroup::text");
                return None
            };
            let text = AribStr::from_bytes(text);
            data = rem;

            groups.push(ComponentGroup {
                component_group_id,
                ca_units,
                total_bit_rate,
                text,
            });
        }

        Some(ComponentGroupDescriptor {
            component_group_type,
            groups,
        })
    }
}

/// LDTリンク記述子におけるリンク先の記述に関する情報。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LdtLinkedDescriptor {
    /// 記述識別。
    pub description_id: u16,
    /// 記述形式識別（4ビット）。
    pub description_type: u8,
    /// 事業者定義ビット。
    pub user_defined: u8,
}

/// LDTリンク記述子。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LdtLinkageDescriptor {
    /// オリジナルサービス識別。
    pub original_service_id: ServiceId,
    /// トランスポートストリーム識別。
    pub transport_stream_id: TransportStreamId,
    /// オリジナルネットワーク識別。
    pub original_network_id: NetworkId,
    /// リンク先の記述を格納する配列。
    pub descriptors: Vec<LdtLinkedDescriptor>,
}

impl Descriptor<'_> for LdtLinkageDescriptor {
    const TAG: u8 = 0xDC;

    fn read(data: &[u8]) -> Option<LdtLinkageDescriptor> {
        if data.len() < 6 {
            log::debug!("invalid LdtLinkageDescriptor");
            return None;
        }

        let Some(original_service_id) = ServiceId::new(data[0..=1].read_be_16()) else {
            log::debug!("invalid LdtLinkageDescriptor::original_network_id");
            return None;
        };
        let Some(transport_stream_id) = TransportStreamId::new(data[2..=3].read_be_16()) else {
            log::debug!("invalid LdtLinkageDescriptor::transport_stream_id");
            return None;
        };
        let Some(original_network_id) = NetworkId::new(data[4..=5].read_be_16()) else {
            log::debug!("invalid LdtLinkageDescriptor::original_network_id");
            return None;
        };
        let descriptors = data[6..]
            .chunks_exact(4)
            .map(|chunk| {
                let description_id = chunk[0..=1].read_be_16();
                let description_type = chunk[2] & 0b00001111;
                let user_defined = chunk[3];

                LdtLinkedDescriptor {
                    description_id,
                    description_type,
                    user_defined,
                }
            })
            .collect();

        Some(LdtLinkageDescriptor {
            original_service_id,
            transport_stream_id,
            original_network_id,
            descriptors,
        })
    }
}
