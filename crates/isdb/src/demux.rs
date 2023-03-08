//! MPEG2-TSのパケットを分離するためのモジュール。
//!
//! 分離には[`Demuxer`]を使用し、PIDごとの処理方法は[`Table`]を使用する。

use crate::packet::Packet;
use crate::pes::{PesError, PesPacket, PesPacketLength};
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
        self.0[pid] = Some(PacketState::new(
            PacketStore::Psi(PartialPsiSection::new()),
            tag,
        ));
    }

    /// `pid`のパケットをPESとして分離するよう設定する。
    ///
    /// `tag`により、PMTなど動的に変わるPIDの代わりに定数でパケット種別を区別することができる。
    #[inline]
    pub fn set_as_pes(&mut self, pid: Pid, tag: T) {
        self.0[pid] = Some(PacketState::new(
            PacketStore::Pes(PartialPesPacket::new()),
            tag,
        ));
    }

    /// `pid`のパケットをユーザー独自に処理するよう設定する。
    ///
    /// `tag`により、PMTなど動的に変わるPIDの代わりに定数でパケット種別を区別することができる。
    #[inline]
    pub fn set_as_custom(&mut self, pid: Pid, tag: T) {
        self.0[pid] = Some(PacketState::new(PacketStore::Custom, tag));
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

    /// フィルター初期化時に呼ばれ、`table`に対し各PIDにおける処理方法を設定する。
    fn on_setup(&mut self, table: &mut Table<Self::Tag>);

    /// パケットが連続していなかった（ドロップしていた）際に呼ばれる。
    ///
    /// このメソッドは処理方法の設定されていないPIDにおいても呼ばれるため`Context`は渡されない。
    fn on_discontinued(&mut self, packet: &Packet) {
        let _ = packet;
    }

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
    fn on_setup(&mut self, table: &mut Table<Self::Tag>) {
        (**self).on_setup(table)
    }

    #[inline]
    fn on_discontinued(&mut self, packet: &Packet) {
        (**self).on_discontinued(packet)
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
///
/// [`Filter`]を実装した型を渡して`Demuxer`を生成し、
/// 受信したパケットを順次`Demuxer::feed`に渡すことでPSIやPESといった塊単位でパケットを分離する。
///
/// # フィルターのタグ
///
/// `Filter`の関連型である`Tag`にはパケットを分離した際に呼ばれるメソッドに渡すタグの型を指定する。
/// タグを活用することで、PMTなど動的に変化するPIDであっても、処理時には静的な値を使って分岐することができ、
/// 高速化や可読性の向上が望める。
///
/// また、`Tag`にタグ付き列挙型を使うことで柔軟に処理を行うことができる。
///
/// `Tag`に指定する型には`Copy`トレイトが要求されることに注意されたい。
///
/// ## タグの例
///
/// `Tag`にタグ付き列挙型を指定する例。
///
/// ```
/// // タグには`Copy`の実装が必須
/// #[derive(Debug, Clone, Copy)]
/// enum Tag {
///     // PSIにおいてPATを表すタグ
///     Pat,
///     // PSIにおいてPMTを表すタグ
///     Pmt,
///     // PESにおいて動画を表すタグで、サービス識別を内包する
///     Video(u16),
///     // PESにおいて音声を表すタグで、サービス識別を内包する
///     Audio(u16),
/// }
/// ```
///
/// # フィルターのメソッド
///
/// `Filter`に実装するメソッドは以下の通りである。
///
/// - `on_setup`：初期化時に一度だけ呼ばれ、PIDごとに処理する内容を設定した[`Table`]を返す。
///   `Table`ではPIDごとにPSIかPES、あるいは独自に処理するかを指定すると共に、
///   そのPIDのパケットが分離された際に渡されるタグも指定する。
/// - `on_discontinued`：パケットのドロップが検知された際に呼ばれる。
///   このメソッドの実装は任意であるが、実装することでドロップ数を計測することができる。
/// - `on_psi_section`：[`Table`]でPSIと指定されたPIDのパケットを分離した際に呼ばれる。
///   PATやPMTなど、TSにおけるメタデータのようなものがPSIとして送られるため、このメソッドは頻繁に呼び出される。
///   またメソッドに渡される[`Context`]を通して`Table`を取得することで、
///   PSI処理中にPIDの処理内容を設定・設定解除することもできる。
/// - `on_pes_packet`：[`Table`]でPESと指定されたPIDのパケットを分離した際に呼ばれる。
///   動画や音声のデータ、さらには字幕データなどがPESとして送られる。
///   このメソッドにも[`Context`]が渡される。
/// - `on_custom_packet`：[`Table`]で独自に処理すると指定されたPIDのパケットで呼ばれる。
///   この場合、パケットは分離されることなくそのままメソッドに渡される。
///   このメソッドにも他のメソッドと同じ[`Context`]に加え、`cc_ok`という引数も渡される。
///   これはパケットがドロップしていないかどうかを示す真偽値で、パケットが不連続の場合には
///   処理を行わないといったことが可能である。
///   このメソッドの実装は任意である。
///
/// # サンプル
///
/// フィルターの実装例。
///
/// ```
/// use isdb::demux::Demuxer;
///
/// #[derive(Default)]
/// struct Filter {
///     repo: isdb::psi::Repository,
/// }
///
/// // タグには`Copy`の実装が必須
/// #[derive(Clone, Copy)]
/// enum Tag {
///     Pat,
///     Pmt,
/// }
///
/// impl isdb::demux::Filter for Filter {
///     type Tag = Tag;
///
///     // 初期化時に一度だけ呼ばれる。
///     fn on_setup(&mut self, table: &mut isdb::demux::Table<Tag>) {
///         // PATのPIDをPSIとして処理すると設定する。タグにはPATであることを示す値を指定する
///         table.set_as_psi(isdb::pid::Pid::PAT, Tag::Pat);
///     }
///
///     // PSIセクションを分離した際に呼ばれる
///     fn on_psi_section(
///         &mut self,
///         ctx: &mut isdb::demux::Context<Tag>,
///         psi: &isdb::psi::PsiSection,
///     ) {
///         // タグは`ctx.tag()`で取得する
///         // enumで定義したタグを使用しているため網羅的マッチができる
///         match ctx.tag() {
///             Tag::Pat => {
///                 // PATのパケットに対する処理をする
///                 let Some(pat) = self.repo.read::<isdb::psi::table::Pat>(psi) else {
///                     return;
///                 };
///
///                 // ここでPMTのPIDが手に入るため、PMTとして処理するよう`Table`に設定する
///                 // 本来はこれまでPMTとして処理していたPIDの設定を解除する必要があるが、
///                 // ここでは割愛する
///                 for program in pat.pmts {
///                     // PIDは動的だがタグは静的な値である`Tag::Pmt`を設定できる
///                     ctx.table().set_as_psi(program.program_map_pid, Tag::Pmt);
///                 }
///             }
///             // PMTのPIDは動的だが静的な値である`Tag::Pmt`でマッチができる
///             Tag::Pmt => {
///                 // PATで取得したPMTのPIDに対する処理をする
///                 let Some(pmt) = self.repo.read::<isdb::psi::table::Pmt>(psi) else {
///                     return;
///                 };
///
///                 // ...
///             }
///         }
///     }
///
///     // 今回はPESを処理しないため何も実装しない
///     fn on_pes_packet(
///         &mut self,
///         _: &mut isdb::demux::Context<Tag>,
///         _: &isdb::pes::PesPacket,
///     ) {}
/// }
///
/// # fn main() -> std::io::Result<()> {
/// // フィルターを引数に`Demuxer`を生成する
/// let mut demuxer = Demuxer::new(Filter::default());
/// // パケットを`Demuxer::feed`に与え続けることで処理・分離を行う
/// # let file = &mut (&[] as &[u8]);
/// for packet in isdb::Packet::iter(file) {
///     demuxer.feed(&packet?);
/// }
/// # Ok(())
/// # }
/// ```
pub struct Demuxer<T: Filter> {
    filter: T,
    cc: PidTable<u8>,
    table: Table<T::Tag>,
}

impl<T: Filter> Demuxer<T> {
    /// `Demuxer`を生成する。
    pub fn new(mut filter: T) -> Demuxer<T> {
        let cc = PidTable::from_fn(|_| 0x10);
        let mut table = Table::new();

        filter.on_setup(&mut table);
        Demuxer { filter, cc, table }
    }

    /// `Demuxer`で処理しているパケットの状態をリセットする。
    ///
    /// ストリームをシークする際にこのメソッドを使うことで、
    /// パケットの処理状態が不正になることを防ぐ。
    pub fn reset_packets(&mut self) {
        self.cc.fill(0x10);

        for state in self.table.0.iter_mut().flatten() {
            match &mut state.store {
                PacketStore::Pes(pes) => pes.reset(),
                PacketStore::Psi(psi) => psi.reset(),
                _ => {}
            }
        }
    }

    /// 内包するフィルターを参照で返す。
    #[inline]
    pub fn filter(&self) -> &T {
        &self.filter
    }

    /// 内包するフィルターを可変参照で返す。
    #[inline]
    pub fn filter_mut(&mut self) -> &mut T {
        &mut self.filter
    }

    /// `Demuxer`を消費して内包するフィルターを返す。
    #[inline]
    pub fn into_filter(self) -> T {
        self.filter
    }

    fn handle_store(&mut self, packet: &Packet, tag: T::Tag, cc_ok: bool, store: &mut PacketStore) {
        let mut ctx = Context {
            packet,
            tag,
            table: &mut self.table,
        };

        match store {
            PacketStore::Pes(pes) => {
                let Some(payload) = packet.payload().filter(|p| !p.is_empty()) else {
                    return;
                };

                pes.write(
                    &mut self.filter,
                    &mut ctx,
                    payload,
                    packet.unit_start_indicator(),
                );
            }
            PacketStore::Psi(psi) => {
                let Some(payload) = packet.payload().filter(|p| !p.is_empty()) else {
                    return;
                };

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
            PacketStore::Custom => self.filter.on_custom_packet(&mut ctx, cc_ok),
            PacketStore::Temp => unreachable!(),
        }
    }

    /// [`Packet`]を処理してパケットを分離する。
    pub fn feed(&mut self, packet: &Packet) {
        if !packet.is_normal() {
            return;
        }

        let pid = packet.pid();
        let cc_ok = packet.validate_cc(&mut self.cc[pid]);
        if !cc_ok {
            self.filter.on_discontinued(packet);
        }

        let Some(state) = self.table.0[pid].as_mut() else {
            return;
        };
        let tag = state.tag;

        // 所有権を切り離すためにパケット処理中はTempを設定
        let mut store = std::mem::replace(&mut state.store, PacketStore::Temp);
        drop(state);

        self.handle_store(packet, tag, cc_ok, &mut store);

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
    tag: T,
    store: PacketStore,
}

impl<T> PacketState<T> {
    #[inline]
    pub fn new(store: PacketStore, tag: T) -> PacketState<T> {
        PacketState { tag, store }
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

#[derive(Debug, Clone, Copy)]
enum PesState {
    /// ヘッダ受信中。
    Header,
    /// 長さ未規定のペイロード。
    Unbounded,
    /// 長さ指定のペイロード。
    Bounded(u32),
    /// ペイロード受信済み。
    Completed,
}

#[derive(Clone)]
struct PartialPesPacket {
    buffer: Vec<u8>,
    state: PesState,
}

#[derive(Clone)]
struct PartialPsiSection {
    buffer: Vec<u8>,
}

impl PartialPesPacket {
    #[inline]
    pub fn new() -> PartialPesPacket {
        PartialPesPacket {
            // バッファサイズはLibISDBから
            buffer: Vec::with_capacity(0x10005),
            // unit_start_indicatorが真になるまでのパケットは処理しない
            state: PesState::Completed,
        }
    }

    #[inline]
    pub fn reset(&mut self) {
        self.buffer.clear();
        self.state = PesState::Completed;
    }

    fn parse_complete<T: Filter>(filter: &mut T, ctx: &mut Context<T::Tag>, data: &[u8]) {
        match PesPacket::parse_complete(data) {
            // ヘッダは受信済みなのでこれらのエラーが来ることはない
            Err(PesError::InsufficientLength | PesError::InvalidStartCode) => unreachable!(),
            Err(PesError::Corrupted) => {
                log::debug!("pes packet corrupted: {:?}", ctx.packet.pid());
            }
            Err(PesError::Crc16) => {
                log::debug!("pes packet crc16 error: {:?}", ctx.packet.pid());
            }
            Ok(pes) => filter.on_pes_packet(ctx, &pes),
        };
    }

    pub fn write<T: Filter>(
        &mut self,
        filter: &mut T,
        ctx: &mut Context<T::Tag>,
        data: &[u8],
        is_start: bool,
    ) {
        if is_start {
            if matches!(self.state, PesState::Unbounded) {
                Self::parse_complete(filter, ctx, &*self.buffer);
            }
            self.buffer.clear();
            self.state = PesState::Header;
        } else if matches!(self.state, PesState::Completed) {
            return;
        }

        self.buffer.extend_from_slice(data);

        let length = match self.state {
            PesState::Completed => unreachable!(),

            PesState::Header => match PesPacket::parse_length(&*self.buffer) {
                Err(PesError::InsufficientLength) => return,
                Err(PesError::InvalidStartCode) => {
                    log::debug!("pes packet invalid start code: {:?}", ctx.packet.pid());
                    self.state = PesState::Completed;
                    return;
                }
                Err(_) => unreachable!(),

                Ok(PesPacketLength::Unbounded) => {
                    // 長さ未規定のパケットはunit_start_indicatorが真になるまでパースできない
                    self.state = PesState::Unbounded;
                    return;
                }
                Ok(PesPacketLength::Bounded(length)) => {
                    let length = length.get();
                    self.state = PesState::Bounded(length);
                    length
                }
            },
            PesState::Bounded(length) => length,
            PesState::Unbounded => return,
        };

        let Some(data) = self.buffer.get(..length as usize) else {
            return;
        };
        Self::parse_complete(filter, ctx, data);
        self.state = PesState::Completed;
    }
}

impl PartialPsiSection {
    #[inline]
    pub fn new() -> PartialPsiSection {
        PartialPsiSection {
            // バッファサイズはLibISDBから
            buffer: Vec::with_capacity(3 + 4093),
        }
    }

    #[inline]
    pub fn reset(&mut self) {
        self.buffer.clear();
    }

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

        self.buffer.extend_from_slice(data);

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
