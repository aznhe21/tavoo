//! MPEG-2 Systemsや告示（平成26年総務省告示第233号）で規定されるテーブルおよび関連する型の定義。

use std::num::NonZeroU16;

use crate::pid::Pid;
use crate::psi::desc::{DescriptorBlock, StreamType};
use crate::psi::{PsiSection, PsiTable};
use crate::utils::BytesExt;

/// トランスポートストリーム識別。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TransportStreamId(pub NonZeroU16);

impl_id!(TransportStreamId);

/// ネットワーク識別。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NetworkId(pub NonZeroU16);

impl_id!(NetworkId);

/// サービス識別。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ServiceId(pub NonZeroU16);

impl_id!(ServiceId);

/// トランスポートストリームの物理的構成に関する情報。
#[derive(Debug, PartialEq, Eq)]
pub struct TransportStreamConfig<'a> {
    /// トランスポートストリーム識別。
    pub transport_stream_id: TransportStreamId,
    /// オリジナルネットワーク識別。
    pub original_network_id: NetworkId,
    /// トランスポート記述子の塊。
    pub transport_descriptors: DescriptorBlock<'a>,
}

/// PMTのあるPIDの定義。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatProgram {
    /// 放送番組番号識別。
    pub program_number: ServiceId,
    /// PMTのPID。
    pub program_map_pid: Pid,
}

/// PAT（Program Association Table）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pat {
    /// トランスポートストリーム識別。
    pub transport_stream_id: TransportStreamId,

    /// NITのPID。
    pub network_pid: Pid,

    /// PMTのPIDを格納する配列。
    pub pmts: Vec<PatProgram>,
}

impl Pat {
    /// PATのテーブルID。
    pub const TABLE_ID: u8 = 0x00;
}

impl PsiTable<'_> for Pat {
    fn read(psi: &PsiSection) -> Option<Pat> {
        if psi.table_id != Self::TABLE_ID {
            log::debug!("invalid Pat::table_id");
            return None;
        }
        let Some(syntax) = psi.syntax.as_ref() else {
            log::debug!("invalid Pat::syntax");
            return None;
        };

        let Some(transport_stream_id) = TransportStreamId::new(syntax.table_id_extension) else {
            log::debug!("invalid Pat::table_id_extension");
            return None;
        };

        let mut network_pid = Pid::default();
        let mut pmts = Vec::new();
        for chunk in psi.data.chunks_exact(4) {
            let program_number = chunk[0..=1].read_be_16();
            let pid = Pid::read(&chunk[2..=3]);

            if let Some(program_number) = ServiceId::new(program_number) {
                // PMT
                pmts.push(PatProgram {
                    program_number,
                    program_map_pid: pid,
                });
            } else {
                // NIT
                network_pid = pid;
            }
        }

        Some(Pat {
            transport_stream_id,
            network_pid,
            pmts,
        })
    }
}

/// CAT（Conditional Access Table）。
#[derive(Debug, PartialEq, Eq)]
pub struct Cat<'a> {
    /// 記述子の塊。
    pub descriptors: DescriptorBlock<'a>,
}

impl<'a> Cat<'a> {
    /// CATのテーブルID。
    const TABLE_ID: u8 = 0x01;
}

impl<'a> PsiTable<'a> for Cat<'a> {
    fn read(psi: &PsiSection<'a>) -> Option<Cat<'a>> {
        if psi.table_id != Self::TABLE_ID {
            log::debug!("invalid Cat::table_id");
            return None;
        }

        let (descriptors, _) =
            DescriptorBlock::read_with_len(psi.data, psi.data.len() as u16).unwrap();

        Some(Cat { descriptors })
    }
}

/// 各サービスを構成するストリームのPIDの定義。
#[derive(Debug, PartialEq, Eq)]
pub struct PmtStream<'a> {
    /// ストリーム形式種別。
    pub stream_type: StreamType,
    /// エレメンタリーPID。
    pub elementary_pid: Pid,
    /// 記述子の塊。
    pub descriptors: DescriptorBlock<'a>,
}

/// PMT（Program Map Table）。
#[derive(Debug, PartialEq, Eq)]
pub struct Pmt<'a> {
    /// 放送番組番号識別。
    pub program_number: ServiceId,
    /// PCRのPID。
    pub pcr_pid: Pid,
    /// 記述子の塊。
    pub descriptors: DescriptorBlock<'a>,
    /// ストリームのPIDを格納する配列。
    pub streams: Vec<PmtStream<'a>>,
}

impl<'a> Pmt<'a> {
    /// PMTのテーブルID。
    pub const TABLE_ID: u8 = 0x02;
}

impl<'a> PsiTable<'a> for Pmt<'a> {
    fn read(psi: &PsiSection<'a>) -> Option<Pmt<'a>> {
        if psi.table_id != Self::TABLE_ID {
            log::debug!("invalid Pmt::table_id");
            return None;
        }
        let Some(syntax) = psi.syntax.as_ref() else {
            log::debug!("invalid Pmt::syntax");
            return None;
        };

        let data = psi.data;
        if data.len() < 4 {
            log::debug!("invalid Pmt");
            return None;
        }

        let Some(program_number) = ServiceId::new(syntax.table_id_extension) else {
            log::debug!("invalid Pmt::table_id_extension");
            return None;
        };
        let pcr_pid = Pid::read(&data[0..=1]);
        let Some((descriptors, mut data)) = DescriptorBlock::read(&data[2..]) else {
            log::debug!("invalid Pmt::descriptors");
            return None;
        };

        let mut streams = Vec::new();
        while !data.is_empty() {
            if data.len() < 5 {
                log::debug!("invalid PmtStream");
                return None;
            }

            let stream_type = StreamType(data[0]);
            let elementary_pid = Pid::read(&data[1..=2]);
            let Some((descriptors, rem)) = DescriptorBlock::read(&data[3..]) else {
                log::debug!("invalid PmtStream::descriptors");
                return None;
            };
            data = rem;

            streams.push(PmtStream {
                stream_type,
                elementary_pid,
                descriptors,
            });
        }

        Some(Pmt {
            program_number,
            pcr_pid,
            descriptors,
            streams,
        })
    }
}

/// NIT（Network Information Table）。
#[derive(Debug, PartialEq, Eq)]
pub struct Nit<'a> {
    /// ネットワーク識別。
    pub network_id: NetworkId,
    /// ネットワーク記述子の塊。
    pub network_descriptors: DescriptorBlock<'a>,
    /// TSの物理的構成を格納する配列。
    pub transport_streams: Vec<TransportStreamConfig<'a>>,
}

impl<'a> Nit<'a> {
    /// NITのテーブルID。
    pub const TABLE_ID: u8 = 0x40;
}

impl<'a> PsiTable<'a> for Nit<'a> {
    fn read(psi: &PsiSection<'a>) -> Option<Nit<'a>> {
        if psi.table_id != Self::TABLE_ID {
            log::debug!("invalid Nit::table_id");
            return None;
        }
        let Some(syntax) = psi.syntax.as_ref() else {
            log::debug!("invalid Nit::syntax");
            return None;
        };

        let data = psi.data;
        if data.len() < 2 {
            log::debug!("invalid Nit");
            return None;
        }

        let Some(network_id) = NetworkId::new(syntax.table_id_extension) else {
            log::debug!("invalid Nit::table_id_extension");
            return None;
        };
        let Some((network_descriptors, data)) = DescriptorBlock::read(&data[0..]) else {
            log::debug!("invalid Nit::descriptors");
            return None;
        };

        if data.len() < 2 {
            log::debug!("invalid Nit::transport_stream_loop_length");
            return None;
        }
        let transport_stream_loop_length = data[0..=1].read_be_16() & 0b0000_1111_1111_1111;
        let Some(mut data) = data[2..].get(..transport_stream_loop_length as usize) else {
            log::debug!("invalid Nit::transport_streams");
            return None;
        };

        let mut transport_streams = Vec::new();
        while !data.is_empty() {
            if data.len() < 6 {
                log::debug!("invalid NitTransportStream");
                return None;
            }

            let Some(transport_stream_id) = TransportStreamId::new(data[0..=1].read_be_16()) else {
                log::debug!("invalid NitTransportStream::transport_stream_id");
                return None;
            };
            let Some(original_network_id) = NetworkId::new(data[2..=3].read_be_16()) else {
                log::debug!("invalid NitTransportStream::original_network_id");
                return None;
            };
            let Some((transport_descriptors, rem)) = DescriptorBlock::read(&data[4..]) else {
                log::debug!("invalid NitTransportStream::transport_descriptors");
                return None;
            };
            data = rem;

            transport_streams.push(TransportStreamConfig {
                transport_stream_id,
                original_network_id,
                transport_descriptors,
            });
        }

        Some(Nit {
            network_id,
            network_descriptors,
            transport_streams,
        })
    }
}
