//! MPEG2-TSのパケットを分離するためのモジュール。

use arrayvec::ArrayVec;

use crate::packet::Packet;
use crate::pes::{PesError, PesPacket};
use crate::pid::{Pid, PidTable};
use crate::psi::{PsiError, PsiSection};
use crate::utils::SliceExt;

/// 各PIDにおける処理方法を設定するテーブル。
///
/// 型パラメータ`T`には`Filter::Tag`を指定する。
#[derive(Clone)]
pub struct Table<T>(PidTable<Option<PacketState<T>>>);

impl<T: Copy> Table<T> {
    /// 何も設定されていない空のテーブルを生成する。
    #[inline]
    pub fn new() -> Table<T> {
        Table(PidTable::from_fn(|_| None))
    }

    /// `pid`のパケットに処理が設定されているかどうかを返す。
    #[inline]
    pub fn is_set(&self, pid: Pid) -> bool {
        self.0[pid].is_some()
    }

    /// `pid`のパケットをPSIとして分離するよう設定されているかどうかを返す。
    #[inline]
    pub fn is_psi(&self, pid: Pid) -> bool {
        matches!(
            self.0[pid],
            Some(PacketState {
                store: PacketStore::Psi(_),
                ..
            })
        )
    }

    /// `pid`のパケットをPESとして分離するよう設定されているかどうかを返す。
    #[inline]
    pub fn is_pes(&self, pid: Pid) -> bool {
        matches!(
            self.0[pid],
            Some(PacketState {
                store: PacketStore::Pes(_),
                ..
            })
        )
    }

    /// `pid`のパケットをユーザー独自に処理するよう設定されているかどうかを返す。
    #[inline]
    pub fn is_custom(&self, pid: Pid) -> bool {
        matches!(
            self.0[pid],
            Some(PacketState {
                store: PacketStore::Custom,
                ..
            })
        )
    }

    /// `pid`のパケットに設定されたタグを返す。
    #[inline]
    pub fn get_tag(&self, pid: Pid) -> Option<T> {
        self.0[pid].as_ref().map(|s| s.tag)
    }

    /// `pid`のパケットをPSIとして分離するよう設定する。
    ///
    /// `tag`により、PMTなど動的に変わるPIDの代わりに定数でパケット種別を区別することができる。
    #[inline]
    pub fn set_as_psi(&mut self, pid: Pid, tag: T) {
        self.0[pid] = Some(PacketState::new(PacketStore::psi(), tag));
    }

    /// `pid`のパケットをPESとして分離するよう設定する。
    ///
    /// `tag`により、PMTなど動的に変わるPIDの代わりに定数でパケット種別を区別することができる。
    #[inline]
    pub fn set_as_pes(&mut self, pid: Pid, tag: T) {
        self.0[pid] = Some(PacketState::new(PacketStore::pes(), tag));
    }

    /// `pid`のパケットをユーザー独自に処理するよう設定する。
    ///
    /// `tag`により、PMTなど動的に変わるPIDの代わりに定数でパケット種別を区別することができる。
    #[inline]
    pub fn set_as_custom(&mut self, pid: Pid, tag: T) {
        self.0[pid] = Some(PacketState::new(PacketStore::custom(), tag));
    }

    /// `pid`のパケットで何も処理しないよう設定を解除する。
    #[inline]
    pub fn unset(&mut self, pid: Pid) {
        self.0[pid] = None;
    }
}

impl<T: Copy> Default for Table<T> {
    #[inline]
    fn default() -> Table<T> {
        Table::new()
    }
}

/// パケットの分離における状況。
pub struct Context<'a, T> {
    packet: &'a Packet,
    tag: T,
    table: &'a mut Table<T>,
}

impl<'a, T: Copy> Context<'a, T> {
    /// 分離対象のパケットを返す。
    #[inline]
    pub fn packet(&self) -> &Packet {
        self.packet
    }

    /// 現在のPIDに設定されたタグを返す。
    #[inline]
    pub fn tag(&self) -> T {
        self.tag
    }

    /// 各PIDにおける処理方法を設定するテーブルを返す。
    #[inline]
    pub fn table(&mut self) -> &mut Table<T> {
        self.table
    }
}

/// [`Demuxer`]に渡すフィルターで、パケットを処理するために各メソッドが呼ばれる。
pub trait Filter {
    /// パケットの種類を識別するためのタグに使う型。
    type Tag: Copy;

    /// フィルター初期化時に呼ばれ、各PIDにおける処理方法を設定するテーブルを返す。
    fn on_setup(&mut self) -> Table<Self::Tag>;

    /// PSIセクションを分離した際に呼ばれる。
    fn on_psi_section(&mut self, ctx: &mut Context<Self::Tag>, psi: &PsiSection);

    /// PESパケットを分離した際に呼ばれる。
    fn on_pes_packet(&mut self, ctx: &mut Context<Self::Tag>, pes: &PesPacket);

    /// 独自に処理するよう設定されたパケットを処理する際に呼ばれる。
    ///
    /// `cc_ok`は連続性指標が正常であるかどうかを示す。
    fn on_custom_packet(&mut self, ctx: &mut Context<Self::Tag>, cc_ok: bool) {
        let _ = (ctx, cc_ok);
    }
}

impl<T: Filter + ?Sized> Filter for &mut T {
    type Tag = T::Tag;

    #[inline]
    fn on_setup(&mut self) -> Table<Self::Tag> {
        (**self).on_setup()
    }

    #[inline]
    fn on_psi_section(&mut self, ctx: &mut Context<Self::Tag>, psi: &PsiSection) {
        (**self).on_psi_section(ctx, psi)
    }

    #[inline]
    fn on_pes_packet(&mut self, ctx: &mut Context<Self::Tag>, pes: &PesPacket) {
        (**self).on_pes_packet(ctx, pes)
    }

    #[inline]
    fn on_custom_packet(&mut self, ctx: &mut Context<Self::Tag>, cc_ok: bool) {
        (**self).on_custom_packet(ctx, cc_ok);
    }
}

/// TSパケットを分離する。
pub struct Demuxer<T: Filter> {
    filter: T,
    table: Table<T::Tag>,
}

impl<T: Filter> Demuxer<T> {
    /// `Demuxer`を生成する。
    pub fn new(mut filter: T) -> Demuxer<T> {
        let table = filter.on_setup();
        Demuxer { filter, table }
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

    /// [`Packet`]を処理してパケットを分離する。
    pub fn feed(&mut self, packet: &Packet) {
        if !packet.is_normal() {
            return;
        }

        let pid = packet.pid();
        let Some(state) = self.table.0[pid].as_mut() else {
            return;
        };
        let tag = state.tag;

        let cc_ok = packet.validate_cc(&mut state.last_cc);

        // 所有権を切り離すためにパケット処理中はTempを設定
        let mut store = std::mem::replace(&mut state.store, PacketStore::Temp);
        drop(state);

        let mut ctx = Context {
            packet,
            tag,
            table: &mut self.table,
        };
        match &mut store {
            PacketStore::Pes(pes) => match packet.payload() {
                Some(payload) if !payload.is_empty() => {
                    pes.write(
                        &mut self.filter,
                        &mut ctx,
                        payload,
                        packet.unit_start_indicator(),
                    );
                }
                _ => {}
            },
            PacketStore::Psi(psi) => match packet.payload() {
                Some(payload) if !payload.is_empty() => {
                    if packet.unit_start_indicator() {
                        let len = payload[0] as usize;
                        let Some((prev, next)) = payload[1..].split_at_checked(len) else {
                            return;
                        };

                        if !prev.is_empty() && cc_ok {
                            psi.write(&mut self.filter, &mut ctx, prev, false);
                        }
                        if !next.is_empty() {
                            psi.write(&mut self.filter, &mut ctx, next, true);
                        }
                    } else {
                        if cc_ok {
                            psi.write(&mut self.filter, &mut ctx, payload, false);
                        }
                    }
                }
                _ => {}
            },
            PacketStore::Custom => self.filter.on_custom_packet(&mut ctx, cc_ok),
            PacketStore::Temp => unreachable!(),
        }

        // フィルター内でテーブルの設定がされていなければ値を戻す
        if let Some(
            state @ PacketState {
                store: PacketStore::Temp,
                ..
            },
        ) = &mut self.table.0[pid]
        {
            state.store = store;
        }
    }
}

#[derive(Clone)]
struct PacketState<T> {
    last_cc: u8,
    tag: T,
    store: PacketStore,
}

impl<T> PacketState<T> {
    #[inline]
    pub fn new(store: PacketStore, tag: T) -> PacketState<T> {
        PacketState {
            last_cc: 0x10,
            tag,
            store,
        }
    }
}

#[derive(Clone)]
enum PacketStore {
    /// PESパケット用。
    Pes(PartialPesPacket),
    /// PSIセクション用。
    Psi(PartialPsiSection),
    /// ユーザーが独自に処理する用。
    Custom,
    /// パケット処理中に設定しておく一時的な値。
    Temp,
}

impl PacketStore {
    #[inline]
    pub fn pes() -> PacketStore {
        PacketStore::Pes(PartialPesPacket {
            buffer: Box::new(ArrayVec::new()),
            finished: false,
        })
    }

    #[inline]
    pub fn psi() -> PacketStore {
        PacketStore::Psi(PartialPsiSection {
            buffer: Box::new(ArrayVec::new()),
        })
    }

    #[inline]
    pub fn custom() -> PacketStore {
        PacketStore::Custom
    }
}

#[derive(Clone)]
struct PartialPesPacket {
    // バッファサイズはLibISDBから
    buffer: Box<ArrayVec<u8, 0x10005>>,
    finished: bool,
}

#[derive(Clone)]
struct PartialPsiSection {
    // バッファサイズはLibISDBから
    buffer: Box<ArrayVec<u8, { 3 + 4093 }>>,
}

impl PartialPesPacket {
    pub fn write<T: Filter>(
        &mut self,
        filter: &mut T,
        ctx: &mut Context<T::Tag>,
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
                log::debug!("pes packet invalid start code: {:?}", ctx.packet.pid());
            }
            Err(PesError::Corrupted) => {
                log::debug!("pes packet corrupted: {:?}", ctx.packet.pid());
            }
            Err(PesError::Crc16) => {
                log::debug!("pes packet crc16 error: {:?}", ctx.packet.pid());
            }
            Ok(pes) => filter.on_pes_packet(ctx, &pes),
        };
        self.finished = true;
    }
}

impl PartialPsiSection {
    pub fn write<T: Filter>(
        &mut self,
        filter: &mut T,
        ctx: &mut Context<T::Tag>,
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
                    log::debug!("psi section corrupted: {:?}", ctx.packet.pid());
                    psi_len
                }
                Err(PsiError::Crc32(psi_len)) => {
                    log::debug!("psi section crc32 error: {:?}", ctx.packet.pid());
                    psi_len
                }
                Ok((psi, psi_len)) => {
                    filter.on_psi_section(ctx, &psi);
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
