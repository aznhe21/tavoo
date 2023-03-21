//! ARIB STD-B10で規定されるテーブルと関連する型の定義。

use std::num::NonZeroU16;
use std::ops::RangeInclusive;

use crate::psi::desc::DescriptorBlock;
use crate::psi::{PsiSection, PsiTable};
use crate::time::DateTime;
use crate::utils::{BytesExt, SliceExt};

use super::iso::{NetworkId, ServiceId, TransportStreamConfig, TransportStreamId};

/// イベント識別。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EventId(pub NonZeroU16);

impl_id!(EventId);

/// SDT進行状態。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RunningStatus {
    /// 未定義。
    Undefined,
    /// 非実行中。
    NotRunning,
    /// 数秒以内に開始（例：映像記録用）。
    StartsSoon,
    /// 停止中。
    Pausing,
    /// 実行中。
    Running,
    /// 予約。
    Reserved,
}

impl From<u8> for RunningStatus {
    #[inline]
    fn from(value: u8) -> RunningStatus {
        match value {
            0 => RunningStatus::Undefined,
            1 => RunningStatus::NotRunning,
            2 => RunningStatus::StartsSoon,
            3 => RunningStatus::Pausing,
            4 => RunningStatus::Running,
            _ => RunningStatus::Reserved,
        }
    }
}

/// バージョン指示。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum VersionIndicator {
    /// 全バージョンが対象（コンテンツバージョンの指定は無効）。
    Whole,
    /// 指定されたバージョン以降が対象。
    After,
    /// 指定されたバージョン以前が対象。
    Before,
    /// 指定されたバージョンのみが対象。
    Only,
}

impl VersionIndicator {
    /// `value`から`VersionIndicator`を生成する。
    ///
    /// # パニック
    ///
    /// 値が範囲外の場合、このメソッドはパニックする。
    #[inline]
    pub fn new(value: u8) -> Self {
        match value {
            0x00 => VersionIndicator::Whole,
            0x01 => VersionIndicator::After,
            0x02 => VersionIndicator::Before,
            0x03 => VersionIndicator::Only,
            _ => unreachable!(),
        }
    }
}

/// 特定のトランスポートストリームに含まれるサービス。
// h_eit_flag等の定義はARIB TR-B14による。
#[derive(Debug, PartialEq, Eq)]
pub struct SdtService<'a> {
    /// サービス識別。
    pub service_id: ServiceId,
    /// 当該サービスに対するH-EITが存在するかどうか。
    pub h_eit_flag: bool,
    /// 当該サービスに対するM-EITが存在するかどうか。
    pub m_eit_flag: bool,
    /// 当該サービスに対するL-EITが存在するかどうか。
    pub l_eit_flag: bool,
    /// EIT［スケジュール］フラグ。
    pub eit_schedule_flag: bool,
    /// EIT［現在／次］フラグ。
    pub eit_present_following_flag: bool,
    /// 進行状態。
    pub running_status: RunningStatus,
    /// スクランブル。
    pub free_ca_mode: bool,
    /// 記述子の塊。
    pub descriptors: DescriptorBlock<'a>,
}

/// SDT（Service Description Table）の共通データ。
#[derive(Debug, PartialEq, Eq)]
pub struct SdtCommon<'a> {
    /// トランスポートストリーム識別。
    pub transport_stream_id: TransportStreamId,
    /// オリジナルネットワーク識別。
    pub original_network_id: NetworkId,
    /// TSのサービスを格納する配列。
    pub services: Vec<SdtService<'a>>,
}

impl<'a> SdtCommon<'a> {
    /// `psi`から`SdtCommon`を読み取る。
    pub fn read(psi: &PsiSection<'a>) -> Option<SdtCommon<'a>> {
        let data = psi.data;
        if data.len() < 3 {
            log::debug!("invalid SdtCommon");
            return None;
        }
        let Some(syntax) = psi.syntax.as_ref() else {
            log::debug!("invalid SdtCommon::syntax");
            return None;
        };

        let Some(transport_stream_id) = TransportStreamId::new(syntax.table_id_extension) else {
            log::debug!("invalid SdtCommon::table_id_extension");
            return None;
        };
        let Some(original_network_id) = NetworkId::new(data[0..=1].read_be_16()) else {
            log::debug!("invalid SdtCommon::original_network_id");
            return None;
        };

        let mut data = &data[3..];
        let mut services = Vec::new();
        while !data.is_empty() {
            if data.len() < 5 {
                log::debug!("invalid SdtService");
                return None;
            }

            let Some(service_id) = ServiceId::new(data[0..=1].read_be_16()) else {
                log::debug!("invalid SdtService::service_id");
                return None;
            };
            let h_eit_flag = data[2] & 0b00010000 != 0;
            let m_eit_flag = data[2] & 0b00001000 != 0;
            let l_eit_flag = data[2] & 0b00000100 != 0;
            let eit_schedule_flag = data[2] & 0b00000010 != 0;
            let eit_present_following_flag = data[2] & 0b00000001 != 0;
            let running_status = ((data[3] & 0b11100000) >> 5).into();
            let free_ca_mode = data[3] & 0b00010000 != 0;
            let Some((descriptors, rem)) = DescriptorBlock::read(&data[3..]) else {
                log::debug!("invalid SdtService::descriptors");
                return None;
            };
            data = rem;

            services.push(SdtService {
                service_id,
                h_eit_flag,
                m_eit_flag,
                l_eit_flag,
                eit_schedule_flag,
                eit_present_following_flag,
                running_status,
                free_ca_mode,
                descriptors,
            });
        }

        Some(SdtCommon {
            transport_stream_id,
            original_network_id,
            services,
        })
    }
}

/// SDT（Service Description Table）。
#[derive(Debug, PartialEq, Eq)]
pub enum Sdt<'a> {
    /// 現在のTSにおけるSDT。
    Actual(SdtCommon<'a>),
    /// 他のTSにおけるSDT。
    Other(SdtCommon<'a>),
}

impl<'a> Sdt<'a> {
    /// 現在のTSにおけるSDTのテーブルID。
    pub const TABLE_ID_ACTUAL: u8 = 0x42;
    /// 他のTSにおけるSDTのテーブルID。
    pub const TABLE_ID_OTHER: u8 = 0x46;
}

impl<'a> PsiTable<'a> for Sdt<'a> {
    fn read(psi: &PsiSection<'a>) -> Option<Self> {
        match psi.table_id {
            Self::TABLE_ID_ACTUAL => Some(Sdt::Actual(SdtCommon::read(psi)?)),
            Self::TABLE_ID_OTHER => Some(Sdt::Other(SdtCommon::read(psi)?)),
            _ => {
                log::debug!("invalid Sdt");
                None
            }
        }
    }
}

/// BAT（Bouquet Association Table）。
#[derive(Debug, PartialEq, Eq)]
pub struct Bat<'a> {
    /// ブーケ識別。
    pub bouquet_id: u16,
    /// ブーケ記述子の塊。
    pub bouquet_descriptors: DescriptorBlock<'a>,
    /// TSの物理的構成を格納する配列。
    pub transport_streams: Vec<TransportStreamConfig<'a>>,
}

impl<'a> Bat<'a> {
    /// BATのテーブルID。
    pub const TABLE_ID: u8 = 0x4A;
}

impl<'a> PsiTable<'a> for Bat<'a> {
    fn read(psi: &PsiSection<'a>) -> Option<Bat<'a>> {
        if psi.table_id != Self::TABLE_ID {
            log::debug!("invalid Bat::table_id");
            return None;
        }
        let Some(syntax) = psi.syntax.as_ref() else {
            log::debug!("invalid Bat::syntax");
            return None;
        };

        let data = psi.data;
        if data.len() < 2 {
            log::debug!("invalid Bat");
            return None;
        }

        let bouquet_id = syntax.table_id_extension;
        let Some((bouquet_descriptors, data)) = DescriptorBlock::read(&data[0..]) else {
            log::debug!("invalid Bat::descriptors");
            return None;
        };

        if data.len() < 2 {
            log::debug!("invalid Bat::transport_stream_loop_length");
            return None;
        }
        let transport_stream_loop_length = data[0..=1].read_be_16() & 0b0000_1111_1111_1111;
        let Some(mut data) = data[2..].get(..transport_stream_loop_length as usize) else {
            log::debug!("invalid Bat::transport_streams");
            return None;
        };

        let mut transport_streams = Vec::new();
        while !data.is_empty() {
            if data.len() < 6 {
                log::debug!("invalid BatTransportStream");
                return None;
            }

            let Some(transport_stream_id) = TransportStreamId::new(data[0..=1].read_be_16()) else {
                log::debug!("invalid BatTransportStream::transport_stream_id");
                return None;
            };
            let Some(original_network_id) = NetworkId::new(data[2..=3].read_be_16()) else {
                log::debug!("invalid BatTransportStream::original_network_id");
                return None;
            };
            let Some((transport_descriptors, rem)) = DescriptorBlock::read(&data[4..]) else {
                log::debug!("invalid BatTransportStream::descriptors");
                return None;
            };
            data = rem;

            transport_streams.push(TransportStreamConfig {
                transport_stream_id,
                original_network_id,
                transport_descriptors,
            });
        }

        Some(Bat {
            bouquet_id,
            bouquet_descriptors,
            transport_streams,
        })
    }
}

/// 各サービスに含まれるイベント。
#[derive(Debug, PartialEq, Eq)]
pub struct EitEvent<'a> {
    /// イベント識別。
    pub event_id: EventId,
    /// 開始時間。
    pub start_time: DateTime,
    /// 継続時間（単位は秒）。
    pub duration: u32,
    /// 進行状態。
    pub running_status: RunningStatus,
    /// スクランブル。
    pub free_ca_mode: bool,
    /// 記述子の塊。
    pub descriptors: DescriptorBlock<'a>,
}

/// EIT（Event Information Table）の共通データ。
#[derive(Debug, PartialEq, Eq)]
pub struct EitCommon<'a> {
    /// サービス識別。
    pub service_id: ServiceId,
    /// セクション番号。
    pub section_number: u8,
    /// トランスポートストリーム識別。
    pub transport_stream_id: TransportStreamId,
    /// オリジナルネットワーク識別。
    pub original_network_id: NetworkId,
    /// セグメント最終セクション番号。
    pub segment_last_section_number: u8,
    /// 最終テーブル識別。
    pub last_table_id: u8,
    /// イベントを格納する配列。
    pub events: Vec<EitEvent<'a>>,
}

impl<'a> EitCommon<'a> {
    /// `psi`から`EitCommon`を読み取る。
    pub fn read(psi: &PsiSection<'a>) -> Option<EitCommon<'a>> {
        let Some(syntax) = psi.syntax.as_ref() else {
            log::debug!("invalid EitCommon::syntax");
            return None;
        };

        let data = psi.data;
        if data.len() < 6 {
            log::debug!("invalid EitCommon");
            return None;
        }

        let Some(service_id) = ServiceId::new(syntax.table_id_extension) else {
            log::debug!("invalid EitCommon::table_id_extension");
            return None;
        };
        let section_number = syntax.section_number;
        let Some(transport_stream_id) = TransportStreamId::new(data[0..=1].read_be_16()) else {
            log::debug!("invalid EitCommon::transport_stream_id");
            return None;
        };
        let Some(original_network_id) = NetworkId::new(data[2..=3].read_be_16()) else {
            log::debug!("invalid EitCommon::original_network_id");
            return None;
        };
        let segment_last_section_number = data[4];
        let last_table_id = data[5];

        let mut data = &data[6..];
        let mut events = Vec::new();
        while !data.is_empty() {
            if data.len() < 12 {
                log::debug!("invalid EitEvent");
                return None;
            }

            let Some(event_id) = EventId::new(data[0..=1].read_be_16()) else {
                log::debug!("invalid EitEvent::event_id");
                return None;
            };
            let start_time = DateTime::read(data[2..=6].try_into().unwrap());
            let duration = data[7..=9].read_bcd_second();
            let running_status = ((data[10] & 0b11100000) >> 5).into();
            let free_ca_mode = data[10] & 0b00010000 != 0;
            let Some((descriptors, rem)) = DescriptorBlock::read(&data[10..]) else {
                log::debug!("invalid EitEvent::descriptors");
                return None;
            };
            data = rem;

            events.push(EitEvent {
                event_id,
                start_time,
                duration,
                running_status,
                free_ca_mode,
                descriptors,
            });
        }

        Some(EitCommon {
            service_id,
            section_number,
            transport_stream_id,
            original_network_id,
            segment_last_section_number,
            last_table_id,
            events,
        })
    }
}

/// EIT（Event Information Table）。
#[derive(Debug, PartialEq, Eq)]
pub enum Eit<'a> {
    /// 自TSにおけるイベント［現在／次］。
    ActualPf(EitCommon<'a>),
    /// 他TSにおけるイベント［現在／次］。
    OtherPf(EitCommon<'a>),
    /// 自TSにおけるイベント［スケジュール］。
    ActualSchedule(EitCommon<'a>),
    /// 他TSにおけるイベント［スケジュール］。
    OtherSchedule(EitCommon<'a>),
}

impl<'a> Eit<'a> {
    /// 自TSにおけるイベント［現在／次］を格納するEITのテーブルID。
    pub const TABLE_ID_PF_ACTUAL: u8 = 0x4E;
    /// 他TSにおけるイベント［現在／次］を格納するEITのテーブルID。
    pub const TABLE_ID_PF_OTHER: u8 = 0x4F;
    /// 自TSにおけるイベント［スケジュール］を格納するEITのテーブルID。
    pub const TABLE_ID_SCHEDULE_ACTUAL: RangeInclusive<u8> = 0x50..=0x5F;
    /// 他TSにおけるイベント［スケジュール］を格納するEITのテーブルID。
    pub const TABLE_ID_SCHEDULE_OTHER: RangeInclusive<u8> = 0x60..=0x6F;
}

impl<'a> PsiTable<'a> for Eit<'a> {
    fn read(psi: &PsiSection<'a>) -> Option<Eit<'a>> {
        match psi.table_id {
            Self::TABLE_ID_PF_ACTUAL => Some(Eit::ActualPf(EitCommon::read(psi)?)),
            Self::TABLE_ID_PF_OTHER => Some(Eit::OtherPf(EitCommon::read(psi)?)),
            0x50..=0x5F => Some(Eit::ActualSchedule(EitCommon::read(psi)?)),
            0x60..=0x6F => Some(Eit::OtherSchedule(EitCommon::read(psi)?)),
            _ => {
                log::debug!("invalid Eit");
                None
            }
        }
    }
}

/// Tdt（Time and Date Table）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tdt {
    /// 現在日付、現在時刻。
    pub jst_time: DateTime,
}

impl Tdt {
    /// TDTのテーブルID。
    pub const TABLE_ID: u8 = 0x70;
}

impl PsiTable<'_> for Tdt {
    fn read(psi: &PsiSection) -> Option<Tdt> {
        if psi.table_id != Self::TABLE_ID {
            log::debug!("invalid Tdt::table_id");
            return None;
        }

        let data = psi.data;
        if data.len() != 5 {
            log::debug!("invalid Tdt");
            return None;
        }

        let jst_time = DateTime::read(&data[0..=4].try_into().unwrap());

        Some(Tdt { jst_time })
    }
}

/// イベントの進行状態。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RstStatus {
    /// トランスポートストリーム識別。
    pub transport_stream_id: TransportStreamId,
    /// オリジナルネットワーク識別。
    pub original_network_id: NetworkId,
    /// サービス識別。
    pub service_id: ServiceId,
    /// イベント識別。
    pub event_id: EventId,
    /// 進行状態。
    pub running_status: RunningStatus,
}

/// RST（Running Status Table）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rst {
    /// イベントの進行状態を格納する配列。
    pub statuses: Vec<RstStatus>,
}

impl Rst {
    /// RSTのテーブルID。
    pub const TABLE_ID: u8 = 0x71;
}

impl PsiTable<'_> for Rst {
    fn read(psi: &PsiSection) -> Option<Rst> {
        if psi.table_id != Self::TABLE_ID {
            log::debug!("invalid Rst::table_id");
            return None;
        }

        let statuses = psi
            .data
            .chunks_exact(9)
            .map(|chunk| {
                let Some(transport_stream_id) = TransportStreamId::new(chunk[0..=1].read_be_16())
                else {
                    log::debug!("invalid RstStatus::transport_stream_id");
                    return None;
                };
                let Some(original_network_id) = NetworkId::new(chunk[2..=3].read_be_16()) else {
                    log::debug!("invalid RstStatus::original_network_id");
                    return None;
                };
                let Some(service_id) = ServiceId::new(chunk[4..=5].read_be_16()) else {
                    log::debug!("invalid RstStatus::service_id");
                    return None;
                };
                let Some(event_id) = EventId::new(chunk[6..=7].read_be_16()) else {
                    log::debug!("invalid RstStatus::event_id");
                    return None;
                };
                let running_status = (chunk[8] & 0b00000111).into();

                Some(RstStatus {
                    transport_stream_id,
                    original_network_id,
                    service_id,
                    event_id,
                    running_status,
                })
            })
            .collect::<Option<_>>()?;

        Some(Rst { statuses })
    }
}

/// TOT（Time Offset Table）。
#[derive(Debug, PartialEq, Eq)]
pub struct Tot<'a> {
    /// 現在日付、現在時刻。
    pub jst_time: DateTime,
    /// 記述子の塊。
    pub descriptors: DescriptorBlock<'a>,
}

impl<'a> Tot<'a> {
    /// TOTのテーブルID。
    pub const TABLE_ID: u8 = 0x73;
}

impl<'a> PsiTable<'a> for Tot<'a> {
    fn read(psi: &PsiSection<'a>) -> Option<Tot<'a>> {
        if psi.table_id != Self::TABLE_ID {
            log::debug!("invalid Tot::table_id");
            return None;
        }

        let data = psi.data;
        if data.len() < 7 {
            log::debug!("invalid Tot");
            return None;
        }

        let jst_time = DateTime::read(&data[0..=4].try_into().unwrap());
        let Some((descriptors, _)) = DescriptorBlock::read(&data[5..]) else {
            log::debug!("invalid Tot::descriptors");
            return None;
        };

        Some(Tot {
            jst_time,
            descriptors,
        })
    }
}

/// 差分配信の開始時刻と継続時間。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PcatScheduleDescription {
    /// 開始時刻。
    pub start_time: DateTime,
    /// 継続時間（単位は秒）。
    pub duration: u32,
}

/// 差分配信のコンテンツ情報。
#[derive(Debug, PartialEq, Eq)]
pub struct PcatContent<'a> {
    /// コンテンツバージョン。
    pub content_version: u16,
    /// コンテンツマイナー場ジョン。
    pub content_minor_version: u16,
    /// バージョン指示。
    pub version_indicator: VersionIndicator,
    /// [`PcatScheduleDescription`]の配列。
    pub schedule_descriptions: Vec<PcatScheduleDescription>,
    /// 記述子群。
    pub content_descriptors: DescriptorBlock<'a>,
}

/// PCAT（Partial Content Announcement Table）。
#[derive(Debug, PartialEq, Eq)]
pub struct Pcat<'a> {
    /// サービス識別。
    pub service_id: ServiceId,
    /// トランスポートストリーム識別。
    pub transport_stream_id: TransportStreamId,
    /// オリジナルネットワーク識別。
    pub original_network_id: NetworkId,
    /// コンテンツ識別。
    pub content_id: u32,
    /// コンテンツを格納する配列。
    pub contents: Vec<PcatContent<'a>>,
}

impl<'a> Pcat<'a> {
    /// PCATのテーブルID。
    pub const TABLE_ID: u8 = 0xC2;
}

impl<'a> PsiTable<'a> for Pcat<'a> {
    fn read(psi: &PsiSection<'a>) -> Option<Pcat<'a>> {
        if psi.table_id != Self::TABLE_ID {
            log::debug!("invalid Pcat::table_id");
            return None;
        }

        let Some(syntax) = psi.syntax.as_ref() else {
            log::debug!("invalid Pcat::syntax");
            return None;
        };

        let data = psi.data;
        if data.len() < 9 {
            log::debug!("invalid Pcat");
            return None;
        }

        let Some(service_id) = ServiceId::new(syntax.table_id_extension) else {
            log::debug!("invalid Pcat::table_id_extension");
            return None;
        };
        let Some(transport_stream_id) = TransportStreamId::new(data[0..=1].read_be_16()) else {
            log::debug!("invalid Pcat::transport_stream_id");
            return None;
        };
        let Some(original_network_id) = NetworkId::new(data[2..=3].read_be_16()) else {
            log::debug!("invalid Pcat::original_network_id");
            return None;
        };
        let content_id = data[4..=7].read_be_32();
        let num_of_content_version = data[8];
        let mut data = &data[9..];

        let mut contents = Vec::with_capacity(num_of_content_version as usize);
        for _ in 0..num_of_content_version {
            if data.len() < 8 {
                log::debug!("invalid PcatContent");
                return None;
            }

            let content_version = data[0..=1].read_be_16();
            let content_minor_version = data[2..=3].read_be_16();
            let version_indicator = VersionIndicator::new((data[4] & 0b11000000) >> 6);
            let content_descriptor_length = data[4..=5].read_be_16() & 0b0000_1111_1111_1111;
            let schedule_description_length = data[6..=7].read_be_16() & 0b0000_1111_1111_1111;
            let Some((schedule_descriptions, rem)) = data[8..]
                .split_at_checked(schedule_description_length as usize)
            else {
                log::debug!("invalid PcatContent::schedule_descriptions");
                return None;
            };
            let Some((content_descriptors, rem)) =
                DescriptorBlock::read_with_len(rem, content_descriptor_length)
            else {
                log::debug!("invalid PcatContent::content_descriptors");
                return None;
            };
            let schedule_descriptions = schedule_descriptions
                .chunks_exact(8)
                .map(|chunk| {
                    let start_time = DateTime::read(chunk[0..=4].try_into().unwrap());
                    let duration = chunk[5..=7].read_bcd_second();

                    PcatScheduleDescription {
                        start_time,
                        duration,
                    }
                })
                .collect();
            data = rem;

            contents.push(PcatContent {
                content_version,
                content_minor_version,
                version_indicator,
                schedule_descriptions,
                content_descriptors,
            });
        }

        Some(Pcat {
            service_id,
            transport_stream_id,
            original_network_id,
            content_id,
            contents,
        })
    }
}

/// ブロードキャスタごとの情報。
#[derive(Debug, PartialEq, Eq)]
pub struct BitBroadcaster<'a> {
    /// ブロードキャスタ識別。
    pub broadcaster_id: u8,
    /// ブロードキャスタ記述子の塊。
    pub broadcaster_descriptors: DescriptorBlock<'a>,
}

/// BIT（Broadcaster Information Table）。
#[derive(Debug, PartialEq, Eq)]
pub struct Bit<'a> {
    /// オリジナルネットワーク識別。
    pub original_network_id: NetworkId,
    /// 事業者表示適否。
    pub broadcast_view_propriety: bool,
    /// 第1記述子の塊。
    pub first_descriptors: DescriptorBlock<'a>,
    /// ブロードキャスタごとの情報を格納する配列。
    pub broadcasters: Vec<BitBroadcaster<'a>>,
}

impl<'a> Bit<'a> {
    /// BITのテーブルID。
    pub const TABLE_ID: u8 = 0xC4;
}

impl<'a> PsiTable<'a> for Bit<'a> {
    fn read(psi: &PsiSection<'a>) -> Option<Bit<'a>> {
        if psi.table_id != Self::TABLE_ID {
            log::debug!("invalid Bit::table_id");
            return None;
        }
        let Some(syntax) = psi.syntax.as_ref() else {
            log::debug!("invalid Bit::syntax");
            return None;
        };

        let data = psi.data;
        if data.len() < 2 {
            log::debug!("invalid Bit");
            return None;
        }

        let Some(original_network_id) = NetworkId::new(syntax.table_id_extension) else {
            log::debug!("invalid Bit::table_id_extension");
            return None;
        };
        let broadcast_view_propriety = data[0] & 0b00010000 != 0;
        let Some((first_descriptors, mut data)) = DescriptorBlock::read(&data[0..]) else {
            log::debug!("invalid Bit::first_descriptors");
            return None;
        };

        let mut broadcasters = Vec::new();
        while !data.is_empty() {
            if data.len() < 3 {
                log::debug!("invalid Bit::broadcaster_id");
                return None;
            };

            let broadcaster_id = data[0];
            let Some((broadcaster_descriptors, rem)) = DescriptorBlock::read(&data[1..]) else {
                log::debug!("invalid Bit::broadcaster_descriptors");
                return None;
            };
            data = rem;

            broadcasters.push(BitBroadcaster {
                broadcaster_id,
                broadcaster_descriptors,
            });
        }

        Some(Bit {
            original_network_id,
            broadcast_view_propriety,
            first_descriptors,
            broadcasters,
        })
    }
}

/// [`NbitCommon`]における情報種別。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum NbitInformationType {
    /// 未定義。
    Undefined,
    /// 掲示情報。
    Information,
    /// サービス識別付き掲示情報。
    WithServiceId,
    /// ジャンル付き掲示情報。
    WithGenre,
    /// 予約。
    Reserved,
}

/// [`NbitCommon`]における記述本体位置。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum NbitDescriptionBodyLocation {
    /// 未定義。
    Undefined,
    /// 詳細情報は自TSのテーブルに記述。
    InActualTsTable,
    /// 詳細情報はSIプライムTSのテーブルに記述。
    InSiPrimeTsTable,
    /// 予約。
    Reserved,
}

/// 案内・お知らせ等の掲示情報。
#[derive(Debug, PartialEq, Eq)]
pub struct NbitInformation<'a> {
    /// 情報識別。
    pub information_id: u16,
    /// 情報種別。
    pub information_type: NbitInformationType,
    /// 記述本体位置。
    pub description_body_location: NbitDescriptionBodyLocation,
    /// 事業者定義ビット。
    pub user_defined: u8,
    /// キー識別を格納する配列。
    pub keys: Vec<u16>,
    /// 記述子の塊。
    pub descriptors: DescriptorBlock<'a>,
}

/// NBIT（Network Board Information）の共通データ。
#[derive(Debug, PartialEq, Eq)]
pub struct NbitCommon<'a> {
    /// オリジナルネットワーク識別。
    pub original_network_id: NetworkId,
    /// 掲示情報を格納する配列。
    pub informations: Vec<NbitInformation<'a>>,
}

impl<'a> NbitCommon<'a> {
    /// `Nbit`を読み取る。
    pub fn read(psi: &PsiSection<'a>) -> Option<NbitCommon<'a>> {
        let Some(syntax) = psi.syntax.as_ref() else {
            log::debug!("invalid NbitCommon::syntax");
            return None;
        };

        let mut data = psi.data;

        let Some(original_network_id) = NetworkId::new(syntax.table_id_extension) else {
            log::debug!("invalid NbitCommon::table_id_extension");
            return None;
        };

        let mut informations = Vec::new();
        while !data.is_empty() {
            if data.len() < 5 {
                log::debug!("invalid NbitInformation");
                return None;
            }

            let information_id = data[0..=1].read_be_16();
            let information_type = match (data[2] & 0b11110000) >> 4 {
                0x0 => NbitInformationType::Undefined,
                0x1 => NbitInformationType::Information,
                0x2 => NbitInformationType::WithServiceId,
                0x3 => NbitInformationType::WithGenre,
                _ => NbitInformationType::Reserved,
            };
            let description_body_location = match (data[2] & 0b00001100) >> 2 {
                0b00 => NbitDescriptionBodyLocation::Undefined,
                0b01 => NbitDescriptionBodyLocation::InActualTsTable,
                0b10 => NbitDescriptionBodyLocation::InSiPrimeTsTable,
                0b11 => NbitDescriptionBodyLocation::Reserved,
                _ => unreachable!(),
            };
            let user_defined = data[3];
            let number_of_keys = data[4];
            let Some((keys, rem)) = data[5..].split_at_checked(number_of_keys as usize * 2) else {
                log::debug!("invalid NbitInformation::keys");
                return None;
            };
            let Some((descriptors, rem)) = DescriptorBlock::read(&rem[0..]) else {
                log::debug!("invalid NbitInformation::descriptors");
                return None;
            };
            let keys = keys.chunks_exact(2).map(<[u8]>::read_be_16).collect();
            data = rem;

            informations.push(NbitInformation {
                information_id,
                information_type,
                description_body_location,
                user_defined,
                keys,
                descriptors,
            });
        }

        Some(NbitCommon {
            original_network_id,
            informations,
        })
    }
}

/// NBIT（Network Board Information Table）。
#[derive(Debug, PartialEq, Eq)]
pub enum Nbit<'a> {
    /// 掲示板情報本体。
    Body(NbitCommon<'a>),
    /// 掲示板情報取得のための参照情報。
    Ref(NbitCommon<'a>),
}

impl<'a> Nbit<'a> {
    /// NBIT本体のテーブルID。
    pub const TABLE_ID_BODY: u8 = 0xC5;
    /// 参照情報のテーブルID。
    pub const TABLE_ID_REF: u8 = 0xC6;
}

impl<'a> PsiTable<'a> for Nbit<'a> {
    fn read(psi: &PsiSection<'a>) -> Option<Nbit<'a>> {
        match psi.table_id {
            Self::TABLE_ID_BODY => Some(Nbit::Body(NbitCommon::read(psi)?)),
            Self::TABLE_ID_REF => Some(Nbit::Ref(NbitCommon::read(psi)?)),
            _ => {
                log::debug!("invalid Nbit");
                None
            }
        }
    }
}

/// [`Ldt`]における記述。
#[derive(Debug, PartialEq, Eq)]
pub struct LdtDescription<'a> {
    /// 記述識別。
    pub description_id: u16,
    /// 記述子の塊。
    pub descriptors: DescriptorBlock<'a>,
}

/// LDT（Linked Description Table）。
#[derive(Debug, PartialEq, Eq)]
pub struct Ldt<'a> {
    /// オリジナルサービス識別。
    pub original_service_id: ServiceId,
    /// トランスポートストリーム識別。
    pub transport_stream_id: TransportStreamId,
    /// オリジナルネットワーク識別。
    pub original_network_id: NetworkId,
    /// 記述を格納する配列。
    pub descriptions: Vec<LdtDescription<'a>>,
}

impl<'a> Ldt<'a> {
    /// LDTのテーブルID。
    pub const TABLE_ID: u8 = 0xC7;
}

impl<'a> PsiTable<'a> for Ldt<'a> {
    fn read(psi: &PsiSection<'a>) -> Option<Ldt<'a>> {
        if psi.table_id != Self::TABLE_ID {
            log::debug!("invalid Ldt::table_id");
            return None;
        }
        let Some(syntax) = psi.syntax.as_ref() else {
            log::debug!("invalid Ldt::syntax");
            return None;
        };

        let data = psi.data;
        if data.len() < 4 {
            log::debug!("invalid Ldt");
            return None;
        }

        let Some(original_service_id) = ServiceId::new(syntax.table_id_extension) else {
            log::debug!("invalid Ldt::table_id_extension");
            return None;
        };
        let Some(transport_stream_id) = TransportStreamId::new(data[0..=1].read_be_16()) else {
            log::debug!("invalid Ldt::transport_stream_id");
            return None;
        };
        let Some(original_network_id) = NetworkId::new(data[2..=3].read_be_16()) else {
            log::debug!("invalid Ldt::original_network_id");
            return None;
        };
        let mut data = &data[4..];

        let mut descriptions = Vec::new();
        while !data.is_empty() {
            if data.len() < 5 {
                log::debug!("invalid LdtDescription");
                return None;
            }

            let description_id = data[0..=1].read_be_16();
            let Some((descriptors, rem)) = DescriptorBlock::read(&data[3..]) else {
                log::debug!("invalid LdtDescription::descriptors");
                return None;
            };
            data = rem;

            descriptions.push(LdtDescription {
                description_id,
                descriptors,
            });
        }

        Some(Ldt {
            original_network_id,
            transport_stream_id,
            original_service_id,
            descriptions,
        })
    }
}

/// [`Lit`]における番組内イベントに関する情報。
#[derive(Debug, PartialEq, Eq)]
pub struct LitLocalEvent<'a> {
    /// 番組内イベント識別。
    pub local_event_id: u16,
    /// 記述子の塊。
    pub descriptors: DescriptorBlock<'a>,
}

/// LIT（Local Event Information Table）。
#[derive(Debug, PartialEq, Eq)]
pub struct Lit<'a> {
    /// イベント識別。
    pub event_id: EventId,
    /// サービス識別。
    pub service_id: ServiceId,
    /// トランスポートストリーム識別。
    pub transport_stream_id: TransportStreamId,
    /// オリジナルネットワーク識別。
    pub original_network_id: NetworkId,
    /// 番組内イベントの情報を格納する配列。
    pub local_events: Vec<LitLocalEvent<'a>>,
}

impl<'a> Lit<'a> {
    /// LITのテーブルID。
    pub const TABLE_ID: u8 = 0xD0;
}

impl<'a> PsiTable<'a> for Lit<'a> {
    fn read(psi: &PsiSection<'a>) -> Option<Lit<'a>> {
        if psi.table_id != Self::TABLE_ID {
            log::debug!("invalid Lit::table_id");
            return None;
        }
        let Some(syntax) = psi.syntax.as_ref() else {
            log::debug!("invalid Lit::syntax");
            return None;
        };

        let data = psi.data;
        if data.len() < 6 {
            log::debug!("invalid Lit");
            return None;
        }

        let Some(event_id) = EventId::new(syntax.table_id_extension) else {
            log::debug!("invalid Lit::table_id_extension");
            return None;
        };
        let Some(service_id) = ServiceId::new(data[0..=1].read_be_16()) else {
            log::debug!("invalid Lit::service_id");
            return None;
        };
        let Some(transport_stream_id) = TransportStreamId::new(data[2..=3].read_be_16()) else {
            log::debug!("invalid Lit::transport_stream_id");
            return None;
        };
        let Some(original_network_id) = NetworkId::new(data[4..=5].read_be_16()) else {
            log::debug!("invalid Lit::original_network_id");
            return None;
        };
        let mut data = &data[6..];

        let mut local_events = Vec::new();
        while !data.is_empty() {
            if data.len() < 4 {
                log::debug!("invalid LitLocalEvent");
                return None;
            }

            let local_event_id = data[0..=1].read_be_16();
            let Some((descriptors, rem)) = DescriptorBlock::read(&data[2..]) else {
                log::debug!("invalid LitLocalEvent::descriptors");
                return None;
            };
            data = rem;

            local_events.push(LitLocalEvent {
                local_event_id,
                descriptors,
            });
        }

        Some(Lit {
            event_id,
            service_id,
            transport_stream_id,
            original_network_id,
            local_events,
        })
    }
}

/// [`ErtNode`]におけるコレクションモード。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ErtCollectionMode {
    /// 集合（bag）。
    Group,
    /// 連結（sequential）。
    Concatenation,
    /// 選択（alternate）。
    Selection,
    /// 並列（parallel）。
    Parallel,
    /// 予約。
    Reserved,
}

/// [`Ert`]における関係識別。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ErtRelationType {
    /// 内容記述（内容記述を行うための木構造を表す）。
    ContentsDescription,
    /// ナビゲーション（表示。選択を行わせるための木構造を表す）。
    Navigation,
    /// 予約。
    Reserved,
}

/// [`Ert`]におけるノード。
#[derive(Debug, PartialEq, Eq)]
pub struct ErtNode<'a> {
    /// ノード識別。
    pub node_id: u16,
    /// コレクションモード。
    pub collection_mode: ErtCollectionMode,
    /// 親ノード識別。
    pub parent_node_id: u16,
    /// 参照番号。
    pub reference_number: u8,
    /// 記述子の塊。
    pub descriptors: DescriptorBlock<'a>,
}

/// ERT（Event Relation Table）。
#[derive(Debug, PartialEq, Eq)]
pub struct Ert<'a> {
    /// イベント関係識別。
    pub event_relation_id: u16,
    /// 情報提供者識別。
    pub information_provider_id: u16,
    /// 関係識別。
    pub relation_type: ErtRelationType,
    /// ノードを格納する配列。
    pub nodes: Vec<ErtNode<'a>>,
}

impl<'a> Ert<'a> {
    /// ERTのテーブルID。
    pub const TABLE_ID: u8 = 0xD1;
}

impl<'a> PsiTable<'a> for Ert<'a> {
    fn read(psi: &PsiSection<'a>) -> Option<Ert<'a>> {
        if psi.table_id != Self::TABLE_ID {
            log::debug!("invalid Ert::table_id");
            return None;
        }
        let Some(syntax) = psi.syntax.as_ref() else {
            log::debug!("invalid Ert::syntax");
            return None;
        };

        let data = psi.data;
        if data.len() < 3 {
            log::debug!("invalid Ert");
            return None;
        }

        let event_relation_id = syntax.table_id_extension;
        let information_provider_id = data[0..=1].read_be_16();
        let relation_type = match (data[2] & 0b11110000) >> 4 {
            0x1 => ErtRelationType::ContentsDescription,
            0x2 => ErtRelationType::Navigation,
            _ => ErtRelationType::Reserved,
        };
        let mut data = &data[3..];

        let mut nodes = Vec::new();
        while !data.is_empty() {
            if data.len() < 8 {
                log::debug!("invalid ErtNode");
                return None;
            }

            let node_id = data[0..=1].read_be_16();
            let collection_mode = match (data[2] & 0b11110000) >> 4 {
                0x0 => ErtCollectionMode::Group,
                0x1 => ErtCollectionMode::Concatenation,
                0x2 => ErtCollectionMode::Selection,
                0x3 => ErtCollectionMode::Parallel,
                _ => ErtCollectionMode::Reserved,
            };
            let parent_node_id = data[3..=4].read_be_16();
            let reference_number = data[5];
            let Some((descriptors, rem)) = DescriptorBlock::read(&data[6..]) else {
                log::debug!("invalid ErtNode::descriptors");
                return None;
            };
            data = rem;

            nodes.push(ErtNode {
                node_id,
                collection_mode,
                parent_node_id,
                reference_number,
                descriptors,
            });
        }

        Some(Ert {
            event_relation_id,
            information_provider_id,
            relation_type,
            nodes,
        })
    }
}

/// ITT（Index Transmission Table）。
#[derive(Debug, PartialEq, Eq)]
pub struct Itt<'a> {
    /// イベント識別。
    pub event_id: EventId,
    /// 記述子の塊。
    pub descriptors: DescriptorBlock<'a>,
}

impl<'a> Itt<'a> {
    /// ITTのテーブルID。
    pub const TABLE_ID: u8 = 0xD2;
}

impl<'a> PsiTable<'a> for Itt<'a> {
    fn read(psi: &PsiSection<'a>) -> Option<Itt<'a>> {
        if psi.table_id != Self::TABLE_ID {
            log::debug!("invalid Itt::table_id");
            return None;
        }
        let Some(syntax) = psi.syntax.as_ref() else {
            log::debug!("invalid Itt::syntax");
            return None;
        };

        let data = psi.data;

        let Some(event_id) = EventId::new(syntax.table_id_extension) else {
            log::debug!("invalid Itt::event_id");
            return None;
        };
        let Some((descriptors, _)) = DescriptorBlock::read(&data[0..]) else {
            log::debug!("invalid Itt::descriptors");
            return None;
        };

        Some(Itt {
            event_id,
            descriptors,
        })
    }
}
