//! MPEG2-TSのパケットを分離するためのモジュール。

use arrayvec::ArrayVec;

use crate::packet::Packet;
use crate::pes::{PesError, PesPacket};
use crate::pid::{Pid, PidTable};
use crate::psi::{PsiError, PsiSection};
use crate::utils::SliceExt;

/// パケット種別。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketType {
    /// パケットがPESであることを表す。
    Pes,

    /// パケットがPSIであることを表す。
    Psi,
}

/// [`Demuxer`]に登録するフィルター。
pub trait Filter {
    /// パケットにエラーがあった際に呼ばれる。
    ///
    /// パケットにエラーがあった場合パケットに関する処理は一切行われない。
    /// つまりフィルターの他のメソッドが呼ばれることもない。
    fn on_transport_error(&mut self) {}

    /// パケットが不正であった際に呼ばれる。
    ///
    /// パケットが不正の場合パケットに関する処理は一切行われない。
    /// つまりフィルターの他のメソッドが呼ばれることもない。
    fn on_format_error(&mut self) {}

    /// パケットを処理する前に呼ばれ、パケットをPSIとPESのどちらで分離するかを返す。。
    ///
    /// `None`を返した場合は分離処理をしない。
    /// その場合には`on_discontinued`・`on_psi_section`・`on_pes_packet`は呼ばれなくなる。
    ///
    /// `packet.is_normal()`が偽のときはパケットが不正であるため、`None`を返すことが推奨される。
    fn on_packet(&mut self, packet: &Packet) -> Option<PacketType>;

    /// パケットが連続していなかった（ドロップしていた）際に呼ばれる。
    fn on_discontinued(&mut self, pid: Pid) {
        let _ = pid;
    }

    /// PSIセクションを分離した際呼ばれる。
    fn on_psi_section(&mut self, packet: &Packet, psi: &PsiSection);

    /// PESパケットを分離した際に呼ばれる。
    fn on_pes_packet(&mut self, packet: &Packet, pes: &PesPacket);
}

impl<T: Filter + ?Sized> Filter for &mut T {
    #[inline]
    fn on_transport_error(&mut self) {
        (**self).on_transport_error()
    }

    #[inline]
    fn on_format_error(&mut self) {
        (**self).on_format_error()
    }

    #[inline]
    fn on_discontinued(&mut self, pid: Pid) {
        (**self).on_discontinued(pid)
    }

    #[inline]
    fn on_packet(&mut self, packet: &Packet) -> Option<PacketType> {
        (**self).on_packet(packet)
    }

    #[inline]
    fn on_psi_section(&mut self, packet: &Packet, psi: &PsiSection) {
        (**self).on_psi_section(packet, psi)
    }

    #[inline]
    fn on_pes_packet(&mut self, packet: &Packet, pes: &PesPacket) {
        (**self).on_pes_packet(packet, pes)
    }
}

/// TSパケットを分離する。
pub struct Demuxer<T> {
    filter: T,
    table: PidTable<PidState>,
}

impl<T> Demuxer<T> {
    /// `Demuxer`を生成する。
    pub fn new(filter: T) -> Demuxer<T> {
        Demuxer {
            filter,
            table: PidTable::from_fn(|_| PidState::default()),
        }
    }

    /// 内包するフィルターを参照で返す。
    #[inline]
    pub fn get_filter(&mut self) -> &T {
        &self.filter
    }

    /// 内包するフィルターを可変参照で返す。
    #[inline]
    pub fn get_filter_mut(&mut self) -> &mut T {
        &mut self.filter
    }

    /// `Demuxer`を消費して内包するフィルターを返す。
    #[inline]
    pub fn into_filter(self) -> T {
        self.filter
    }
}

impl<T: Filter> Demuxer<T> {
    /// [`Packet`]を処理してパケットを分離する。
    pub fn feed(&mut self, packet: &Packet) {
        if packet.error_indicator() {
            self.filter.on_transport_error();
            return;
        }
        if !packet.is_normal() {
            self.filter.on_format_error();
            return;
        }

        let Some(pt) = self.filter.on_packet(packet) else {
            return;
        };

        let pid = packet.pid();
        let state = &mut self.table[pid];

        let cc = if packet.has_payload() {
            packet.continuity_counter()
        } else {
            0x10
        };
        let is_discontinuity = packet
            .adaptation_field()
            .map_or(false, |af| af.discontinuity_indicator);
        let cc_ok = pid == Pid::NULL
            || is_discontinuity
            || cc >= 0x10
            || state.last_cc >= 0x10
            || (state.last_cc + 1) & 0x0F == cc;
        state.last_cc = cc;
        if !cc_ok {
            self.filter.on_discontinued(pid);
        }

        let Some(payload) = packet.payload().filter(|p| !p.is_empty()) else {
            return;
        };

        let data = match &mut state.data {
            // パケット種別同一
            Some(data) if data.packet_type() == pt => data,

            // パケット種別が変わった
            data => data.insert(match pt {
                PacketType::Pes => PidData::pes(),
                PacketType::Psi => PidData::psi(),
            }),
        };
        match data {
            PidData::Pes(pes) => {
                pes.write(
                    &mut self.filter,
                    packet,
                    payload,
                    packet.unit_start_indicator(),
                );
            }
            PidData::Psi(psi) => {
                if packet.unit_start_indicator() {
                    let len = payload[0] as usize;
                    let Some((prev, next)) = payload[1..].split_at_checked(len) else {
                        return;
                    };

                    if !prev.is_empty() && cc_ok {
                        psi.write(&mut self.filter, packet, prev, false);
                    }
                    if !next.is_empty() {
                        psi.write(&mut self.filter, packet, next, true);
                    }
                } else {
                    if cc_ok {
                        psi.write(&mut self.filter, packet, payload, false);
                    }
                }
            }
        }
    }
}

struct PidState {
    last_cc: u8,
    data: Option<PidData>,
}

impl Default for PidState {
    fn default() -> Self {
        PidState {
            last_cc: 0x10,
            data: None,
        }
    }
}

enum PidData {
    Pes(PesPacketData),
    Psi(PsiSectionData),
}

impl PidData {
    #[inline]
    pub fn pes() -> PidData {
        PidData::Pes(PesPacketData {
            buffer: Box::new(ArrayVec::new()),
            finished: false,
        })
    }

    #[inline]
    pub fn psi() -> PidData {
        PidData::Psi(PsiSectionData {
            buffer: Box::new(ArrayVec::new()),
        })
    }

    #[inline]
    pub fn packet_type(&self) -> PacketType {
        match self {
            PidData::Pes(_) => PacketType::Pes,
            PidData::Psi(_) => PacketType::Psi,
        }
    }
}

struct PesPacketData {
    buffer: Box<ArrayVec<u8, 0x10005>>,
    finished: bool,
}

impl PesPacketData {
    pub fn write<T: Filter>(
        &mut self,
        filter: &mut T,
        packet: &Packet,
        data: &[u8],
        is_start: bool,
    ) {
        match (is_start, self.finished) {
            (false, true) => return,
            (false, false) => {}
            (true, _) => {
                self.buffer.clear();
                self.finished = false;
            }
        }

        // バッファに収まる形でdataを追記
        let len = std::cmp::min(self.buffer.remaining_capacity(), data.len());
        let _result = self.buffer.try_extend_from_slice(&data[..len]);
        debug_assert!(_result.is_ok());

        match PesPacket::parse(&**self.buffer) {
            Err(PesError::InsufficientLength) => return,
            Err(PesError::InvalidStartCode) => {
                log::debug!("pes packet invalid start code: {:?}", packet.pid());
            }
            Err(PesError::Corrupted) => {
                log::debug!("pes packet corrupted: {:?}", packet.pid());
            }
            Err(PesError::Crc16) => {
                log::debug!("pes packet crc16 error: {:?}", packet.pid());
            }
            Ok(pes) => filter.on_pes_packet(packet, &pes),
        };
        self.finished = true;
    }
}

struct PsiSectionData {
    buffer: Box<ArrayVec<u8, 4096>>,
}

impl PsiSectionData {
    pub fn write<T: Filter>(
        &mut self,
        filter: &mut T,
        packet: &Packet,
        data: &[u8],
        is_start: bool,
    ) {
        if is_start {
            self.buffer.clear();
        }

        // バッファに収まる形でdataを追記
        let len = std::cmp::min(self.buffer.remaining_capacity(), data.len());
        let _result = self.buffer.try_extend_from_slice(&data[..len]);
        debug_assert!(_result.is_ok());

        let mut buf = self.buffer.as_slice();
        loop {
            let psi_len = match PsiSection::parse(buf) {
                Err(PsiError::InsufficientLength | PsiError::EndOfPsi) => break,
                Err(PsiError::Corrupted(psi_len)) => {
                    log::debug!("psi section corrupted: {:?}", packet.pid());
                    psi_len
                }
                Err(PsiError::Crc32(psi_len)) => {
                    log::debug!("psi section crc32 error: {:?}", packet.pid());
                    psi_len
                }
                Ok((psi, psi_len)) => {
                    filter.on_psi_section(packet, &psi);
                    psi_len
                }
            };

            // 読み込んだPSIセクションの分バッファを進める
            buf = &buf[psi_len..];
        }

        if buf.len() < self.buffer.len() {
            // 処理した部分を捨てる
            let remaining = buf.len();
            let offset = self.buffer.len() - remaining;
            self.buffer.copy_within(offset.., 0);
            self.buffer.truncate(remaining);
        }
    }
}
