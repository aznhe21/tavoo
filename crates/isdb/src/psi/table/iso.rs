//! MPEG-2 Systemsや告示（平成26年総務省告示第233号）で規定されるテーブルおよび関連する型の定義。

use std::num::NonZeroU16;

use crate::pid::Pid;
use crate::psi::desc::{DescriptorBlock, StreamType};
use crate::psi::PsiSection;
use crate::utils::BytesExt;

/// トランスポートストリームの物理的構成に関する情報。
#[derive(Debug)]
pub struct TransportStreamConfig<'a> {
    /// トランスポートストリーム識別。
    pub transport_stream_id: u16,
    /// オリジナルネットワーク識別。
    pub original_network_id: u16,
    /// トランスポート記述子の塊。
    pub transport_descriptors: DescriptorBlock<'a>,
}

/// PMTのあるPIDの定義。
#[derive(Debug)]
pub struct PatProgram {
    /// 放送番組番号識別。
    pub program_number: NonZeroU16,
    /// PMTのPID。
    pub program_map_pid: Pid,
}

/// PAT（Program Association Table）。
#[derive(Debug)]
pub struct Pat {
    /// トランスポートストリーム識別。
    pub transport_stream_id: u16,

    /// NITのPID。
    pub network_pid: Pid,

    /// PMTのPIDを格納する配列。
    pub pmts: Vec<PatProgram>,
}

impl Pat {
    /// PATのテーブルID。
    pub const TABLE_ID: u8 = 0x00;

    /// `psi`から`Pat`を読み取る。
    pub fn read(psi: &PsiSection) -> Option<Pat> {
        if psi.table_id != Self::TABLE_ID {
            log::debug!("invalid Pat::table_id");
            return None;
        }
        let Some(syntax) = psi.syntax.as_ref() else {
            log::debug!("invalid Pat::syntax");
            return None;
        };

        let transport_stream_id = syntax.table_id_extension;

        let mut network_pid = Pid::default();
        let mut pmts = Vec::new();
        for chunk in psi.data.chunks_exact(4) {
            let program_number = chunk[0..=1].read_be_16();
            let pid = Pid::read(&chunk[2..=3]);

            if let Some(program_number) = NonZeroU16::new(program_number) {
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
#[derive(Debug)]
pub struct Cat<'a> {
    /// 記述子の塊。
    pub descriptors: DescriptorBlock<'a>,
}

impl<'a> Cat<'a> {
    /// CATのテーブルID。
    const TABLE_ID: u8 = 0x01;

    /// `psi`から`Cat`を読み取る。
    pub fn read(psi: &PsiSection<'a>) -> Option<Cat<'a>> {
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
#[derive(Debug)]
pub struct PmtStream<'a> {
    /// ストリーム形式種別。
    pub stream_type: StreamType,
    /// エレメンタリーPID。
    pub elementary_pid: Pid,
    /// 記述子の塊。
    pub descriptors: DescriptorBlock<'a>,
}

/// PMT（Program Map Table）。
#[derive(Debug)]
pub struct Pmt<'a> {
    /// 放送番組番号識別。
    pub program_number: u16,
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

    /// `psi`から`Pmt`を読み取る。
    pub fn read(psi: &PsiSection<'a>) -> Option<Pmt<'a>> {
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

        let program_number = syntax.table_id_extension;
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
#[derive(Debug)]
pub struct Nit<'a> {
    /// ネットワーク識別。
    pub network_id: u16,
    /// ネットワーク記述子の塊。
    pub network_descriptors: DescriptorBlock<'a>,
    /// TSの物理的構成を格納する配列。
    pub transport_streams: Vec<TransportStreamConfig<'a>>,
}

impl<'a> Nit<'a> {
    /// NITのテーブルID。
    pub const TABLE_ID: u8 = 0x40;

    /// `psi`から`Nit`を読み取る。
    pub fn read(psi: &PsiSection<'a>) -> Option<Nit<'a>> {
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

        let network_id = syntax.table_id_extension;
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

            let transport_stream_id = data[0..=1].read_be_16();
            let original_network_id = data[2..=3].read_be_16();
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
