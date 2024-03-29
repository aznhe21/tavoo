//! パケットを仕分けるためのフィルター。

use fxhash::{FxHashMap, FxHashSet};
use smallvec::SmallVec;

use crate::demux;
use crate::eight::char::TimeControlMode;
use crate::lang;
use crate::pes;
use crate::pid::Pid;
use crate::psi;
use crate::psi::table::{EventId, ServiceId};
use crate::time;
use crate::AribStr;
use crate::AribString;

/// [`Sorter`]から仕分けた結果を受け取るためのトレイト。
///
/// 各メソッドには現在[`Sorter`]の保持するサービス一覧が与えられる。
pub trait Shooter {
    /// パケットが連続していなかった（ドロップしていた）際に呼ばれる。
    fn on_packet_discontinued(&mut self, services: &ServiceMap, pid: Pid) {
        let _ = (services, pid);
    }

    /// PATが更新された際に呼ばれる。
    ///
    /// パケットの順序によっては既にイベント情報が存在する場合がある。
    fn on_pat_updated(&mut self, services: &ServiceMap);

    /// `service`のPMTが更新された際に呼ばれる。
    fn on_pmt_updated(&mut self, services: &ServiceMap, service: &Service);

    /// `service`のEITが更新された際に呼ばれる。
    ///
    /// このメソッドが呼ばれた後に`Service::present_event`等が`None`を返す場合、
    /// EIT未受信ではないためイベントが存在しないことを表す。
    fn on_eit_updated(&mut self, services: &ServiceMap, service: &Service, is_present: bool);

    /// 映像パケットを受信した際に呼ばれる。
    fn on_video_packet(
        &mut self,
        services: &ServiceMap,
        pid: Pid,
        pts: Option<time::Timestamp>,
        dts: Option<time::Timestamp>,
        payload: &[u8],
    );

    /// 音声パケットを受信した際に呼ばれる。
    fn on_audio_packet(
        &mut self,
        services: &ServiceMap,
        pid: Pid,
        pts: Option<time::Timestamp>,
        dts: Option<time::Timestamp>,
        payload: &[u8],
    );

    /// 字幕パケットを受信した際に呼ばれる。
    fn on_caption(
        &mut self,
        services: &ServiceMap,
        pid: Pid,
        pts: Option<time::Timestamp>,
        caption: &Caption,
    );

    /// 文字スーパーのパケットを受信した際に呼ばれる。
    fn on_superimpose(
        &mut self,
        services: &ServiceMap,
        pid: Pid,
        pts: Option<time::Timestamp>,
        caption: &Caption,
    );

    /// PCRが更新された際に呼ばれる。
    ///
    /// PCRが更新された全サービス識別が`service_ids`で渡される。
    fn on_pcr(&mut self, services: &ServiceMap, service_ids: &[ServiceId]) {
        let _ = (services, service_ids);
    }

    /// TOTが更新された際に呼ばれる。
    fn on_tot(
        &mut self,
        services: &ServiceMap,
        datetime: time::DateTime,
        time_offset: Option<psi::desc::LocalTimeOffsetEntry>,
    ) {
        let _ = (services, datetime, time_offset);
    }
}

/// PMTで送出されるストリーム情報。
#[derive(Debug, Clone)]
pub struct Stream {
    pid: Pid,
    stream_type: psi::desc::StreamType,
    component_tag: Option<u8>,
    video_encode_format: Option<psi::desc::VideoEncodeFormat>,
}

impl Stream {
    /// 無効な`Stream`を返す。
    #[inline]
    pub const fn invalid() -> Stream {
        Stream {
            pid: Pid::NULL,
            stream_type: psi::desc::StreamType::INVALID,
            component_tag: None,
            video_encode_format: None,
        }
    }

    /// ストリームのPID。
    #[inline]
    pub fn pid(&self) -> Pid {
        self.pid
    }

    /// ストリーム形式種別。
    #[inline]
    pub fn stream_type(&self) -> psi::desc::StreamType {
        self.stream_type
    }

    /// ストリームのコンポーネントタグ。
    #[inline]
    pub fn component_tag(&self) -> Option<u8> {
        self.component_tag
    }

    /// ストリームのビデオエンコードフォーマット。
    #[inline]
    pub fn video_encode_format(&self) -> Option<psi::desc::VideoEncodeFormat> {
        self.video_encode_format
    }
}

/// PAT・PMT・EITから送出されるサービス情報。
#[derive(Debug, Clone)]
pub struct Service {
    service_id: ServiceId,
    pmt_pid: Pid,
    pcr_pid: Pid,
    pcr: Option<time::Timestamp>,
    pmt_filled: bool,
    /// 映像ストリーム一覧。component_tagにより昇順に並ぶ
    video_streams: Vec<Stream>,
    /// 音声ストリーム一覧。component_tagにより昇順に並ぶ
    audio_streams: Vec<Stream>,
    /// 字幕ストリーム
    caption_stream: Option<Stream>,
    /// 文字スーパーのストリーム
    superimpose_stream: Option<Stream>,

    provider_name: AribString,
    service_name: AribString,
    present_event: Option<EventInfo>,
    following_event: Option<EventInfo>,
}

impl Service {
    /// サービスのサービス識別。
    #[inline]
    pub fn service_id(&self) -> ServiceId {
        self.service_id
    }

    /// このサービスがワンセグかどうかを返す。
    #[inline]
    pub fn is_oneseg(&self) -> bool {
        self.pmt_pid.is_oneseg_pmt()
    }

    /// サービスにおけるPCRのPID。
    #[inline]
    pub fn pcr_pid(&self) -> Pid {
        self.pcr_pid
    }

    /// サービスにおける現在のPCR。
    #[inline]
    pub fn pcr(&self) -> Option<time::Timestamp> {
        self.pcr
    }

    /// このサービスのPMTが受信済みかどうか。
    #[inline]
    pub fn pmt_filled(&self) -> bool {
        self.pmt_filled
    }

    /// 映像ストリーム一覧。
    #[inline]
    pub fn video_streams(&self) -> &[Stream] {
        &*self.video_streams
    }

    /// 音声ストリーム一覧。
    #[inline]
    pub fn audio_streams(&self) -> &[Stream] {
        &*self.audio_streams
    }

    /// 字幕ストリーム。
    #[inline]
    pub fn caption_stream(&self) -> Option<&Stream> {
        self.caption_stream.as_ref()
    }

    /// 文字スーパーのストリーム。
    #[inline]
    pub fn superimpose_stream(&self) -> Option<&Stream> {
        self.superimpose_stream.as_ref()
    }

    #[inline]
    fn all_streams(&self) -> impl Iterator<Item = &Stream> {
        std::iter::empty()
            .chain(&*self.video_streams)
            .chain(&*self.audio_streams)
            .chain(&self.caption_stream)
            .chain(&self.superimpose_stream)
    }

    /// 事業者名。
    #[inline]
    pub fn provider_name(&self) -> &AribStr {
        &*self.provider_name
    }

    /// サービス名。
    #[inline]
    pub fn service_name(&self) -> &AribStr {
        &*self.service_name
    }

    /// 現在のイベント情報。
    ///
    /// EIT未受信、またはイベントが存在しない場合に`None`を返す。
    #[inline]
    pub fn present_event(&self) -> Option<&EventInfo> {
        self.present_event.as_ref()
    }

    /// 次のイベント情報。
    ///
    /// EIT未受信、またはイベントが存在しない場合に`None`を返す。
    #[inline]
    pub fn following_event(&self) -> Option<&EventInfo> {
        self.following_event.as_ref()
    }

    fn find_stream(streams: &[Stream], component_tag: Option<u8>) -> Option<&Stream> {
        component_tag
            .and_then(|component_tag| {
                streams
                    .iter()
                    .find(|s| s.component_tag == Some(component_tag))
            })
            // 同一コンポーネントタグがないのでデフォルトESに切り替え
            .or_else(|| streams.first())
    }

    /// `video_tag`と一致するコンポーネントタグの映像ストリームを検索する。
    ///
    /// `video_tag`に`None`を指定した場合、または指定されたコンポーネントタグと一致する映像ストリームがない場合、
    /// デフォルトESを返す。
    ///
    /// ARIBの仕様上デフォルトESが必ず存在するが、
    /// データ上はストリームが存在しないこともあり得るため、
    /// このメソッドは`Option`を返すようにしている。
    pub fn find_video_stream(&self, video_tag: Option<u8>) -> Option<&Stream> {
        Self::find_stream(&*self.video_streams, video_tag)
    }

    /// `audio_tag`と一致するコンポーネントタグの音声ストリームを検索する。
    ///
    /// `audio_tag`に`None`を指定した場合、または指定されたコンポーネントタグと一致する音声ストリームがない場合、
    /// デフォルトESを返す。
    ///
    /// ARIBの仕様上デフォルトESが必ず存在するが、
    /// データ上はストリームが存在しないこともあり得るため、
    /// このメソッドは`Option`を返すようにしている。
    pub fn find_audio_stream(&self, audio_tag: Option<u8>) -> Option<&Stream> {
        Self::find_stream(&*self.audio_streams, audio_tag)
    }
}

/// 番組に関する情報。
#[derive(Debug, Clone)]
pub struct EventInfo {
    /// 番組ID。
    pub event_id: EventId,
    /// 番組開始時刻。
    pub start_time: time::DateTime,
    /// 番組の継続時間（秒単位）。
    pub duration: u32,
    /// 番組名。
    pub name: Option<AribString>,
    /// 番組情報。
    pub text: Option<AribString>,
    /// 拡張番組情報。
    pub extended_items: Vec<ExtendedEventItem>,
    /// 映像に関する情報。
    pub video_components: Vec<VideoComponent>,
    /// 音声に関する情報。
    pub audio_components: Vec<AudioComponent>,
    /// 分類。
    pub genres: Option<SmallVec<[psi::desc::ContentGenre; 7]>>,
}

/// 拡張番組情報の要素。
#[derive(Debug, Clone)]
pub struct ExtendedEventItem {
    /// 項目名。
    pub item: AribString,
    /// 概要。
    pub description: AribString,
}

/// 番組の映像に関する情報。
#[derive(Debug, Clone)]
pub struct VideoComponent {
    /// コンポーネント内容（4ビット）。
    pub stream_content: u8,
    /// コンポーネント種別。
    pub component_type: u8,
    /// コンポーネントタグ。
    pub component_tag: u8,
    /// 言語コード。
    pub lang_code: lang::LangCode,
    /// コンポーネント記述。
    pub text: AribString,
}

/// 番組の音声に関する情報。
#[derive(Debug, Clone)]
pub struct AudioComponent {
    /// コンポーネント内容（4ビット）。
    pub stream_content: u8,
    /// コンポーネント種別。
    pub component_type: u8,
    /// コンポーネントタグ。
    pub component_tag: u8,
    /// ストリーム形式種別。
    pub stream_type: psi::desc::StreamType,
    /// サイマルキャストグループ識別。
    pub simulcast_group_tag: u8,
    /// 主コンポーネントフラグ。
    pub main_component_flag: bool,
    /// 音質表示。
    pub quality_indicator: psi::desc::QualityIndicator,
    /// サンプリング周波数。
    pub sampling_rate: psi::desc::SamplingFrequency,
    /// 言語コード。
    pub lang_code: lang::LangCode,
    /// 言語コードその2。
    pub lang_code_2: Option<lang::LangCode>,
    /// コンポーネント記述。
    pub text: AribString,
}

/// 字幕のデータグループ。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CaptionGroup {
    /// 組A。
    GroupA,
    /// 組B。
    GroupB,
}

/// 字幕管理データ。
#[derive(Debug)]
pub struct CaptionManagementData<'a> {
    /// データグループ。
    pub group: CaptionGroup,
    /// 時刻制御モード。
    pub tmd: TimeControlMode,
    /// オフセット時刻（単位はミリ秒）。
    pub otm: Option<u32>,
    /// 言語。
    pub languages: Vec<pes::caption::CaptionLanguage>,
    /// データユニット。
    pub data_units: Vec<pes::caption::DataUnit<'a>>,
}

/// 字幕文データ。
#[derive(Debug)]
pub struct CaptionData<'a> {
    /// データグループ。
    pub group: CaptionGroup,
    /// 言語識別。
    pub language_tag: pes::caption::LanguageTag,
    /// 時刻制御モード。
    pub tmd: TimeControlMode,
    /// 提示開始時刻（単位はミリ秒）。
    pub stm: Option<u32>,
    /// データユニット。
    pub data_units: Vec<pes::caption::DataUnit<'a>>,
}

/// 字幕データ。
#[derive(Debug)]
pub enum Caption<'a> {
    /// 字幕管理データ。
    ManagementData(CaptionManagementData<'a>),
    /// 字幕文データ。
    Data(CaptionData<'a>),
}

impl<'a> Caption<'a> {
    /// 字幕のデータユニットを返す。
    pub fn data_units(&self) -> &[pes::caption::DataUnit] {
        match self {
            Caption::ManagementData(management) => &*management.data_units,
            Caption::Data(data) => &*data.data_units,
        }
    }
}

/// サービス識別からサービス情報を得るための、順序を保持する連想配列。
pub type ServiceMap = indexmap::IndexMap<ServiceId, Service, fxhash::FxBuildHasher>;

/// 仕分け用フィルター。
pub struct Sorter<T> {
    shooter: T,
    repo: psi::Repository,

    services: ServiceMap,
    // PAT前のEIT
    events: FxHashMap<ServiceId, (Option<EventInfo>, Option<EventInfo>)>,
}

impl<T> Sorter<T> {
    /// `Sorter`を生成する。
    pub fn new(shooter: T) -> Sorter<T> {
        Sorter {
            shooter,
            repo: psi::Repository::new(),

            services: ServiceMap::default(),
            events: FxHashMap::default(),
        }
    }

    /// 内包する`Shooter`を参照で返す。
    #[inline]
    pub fn shooter(&self) -> &T {
        &self.shooter
    }

    /// 内包する`Shooter`を可変参照で返す。
    #[inline]
    pub fn shooter_mut(&mut self) -> &mut T {
        &mut self.shooter
    }

    /// 現在のTSにおけるすべてのサービスを返す。
    ///
    /// 戻り値はPATで記述された順に並ぶ[`IndexMap`]で、キーはサービス識別である。
    ///
    /// [`IndexMap`]: indexmap::IndexMap
    #[inline]
    pub fn services(&self) -> &ServiceMap {
        &self.services
    }

    /// すべてのサービス及び内包する`Shooter`を、前者は参照で、後者は可変参照で返す。
    pub fn pair(&mut self) -> (&ServiceMap, &mut T) {
        (&self.services, &mut self.shooter)
    }
}

mod sealed {
    // モジュール直下に定義するとE0446で怒られるので封印
    #[derive(Debug, Clone, Copy)]
    pub enum Tag {
        // PSI
        Pat,
        Pmt,
        Sdt,
        Eit,
        Tot,

        // PES
        Video,
        Audio,
        Caption,
        Superimpose,

        // on_packet_storing用
        Pcr,
    }
}

use sealed::Tag;

impl<T: Shooter> demux::Filter for Sorter<T> {
    type Tag = Tag;

    fn on_setup(&mut self, table: &mut demux::Table<Self::Tag>) {
        table.set_as_psi(Pid::PAT, Tag::Pat);
        table.set_as_psi(Pid::SDT, Tag::Sdt);
        table.set_as_psi(Pid::H_EIT, Tag::Eit);
        table.set_as_psi(Pid::L_EIT, Tag::Eit);
        table.set_as_psi(Pid::TOT, Tag::Tot);
    }

    fn on_discontinued(&mut self, packet: &crate::Packet) {
        self.shooter
            .on_packet_discontinued(&self.services, packet.pid());
    }

    fn on_packet_storing(&mut self, ctx: &mut demux::Context<Self::Tag>) {
        // PMTのpcr_pidとESのPIDが共有される場合があるのでon_custom_packetではなくこちらで処理する
        let Some(pcr) = ctx.packet().adaptation_field().and_then(|af| af.pcr()) else {
            return;
        };

        let pid = ctx.packet().pid();
        // 大抵の局でサービス数は4個も無さげ
        let mut service_ids = SmallVec::<[_; 4]>::new_const();
        for service in self.services.values_mut() {
            if service.pcr_pid == pid {
                service.pcr = Some(pcr);
                service_ids.push(service.service_id);
            }
        }
        if !service_ids.is_empty() {
            self.shooter.on_pcr(&self.services, &*service_ids);
        }
    }

    fn on_psi_section(&mut self, ctx: &mut demux::Context<Self::Tag>, psi: &psi::PsiSection) {
        match ctx.tag() {
            Tag::Pat => {
                let Some(pat) = self.repo.read::<psi::table::Pat>(psi) else {
                    return;
                };

                // メモリを解放するためself.eventsを置き換える
                let mut events = std::mem::take(&mut self.events);

                for (i, program) in pat.pmts.iter().enumerate() {
                    let service_id = program.program_number;

                    let entry = self.services.entry(service_id);
                    let index = entry.index();
                    entry.or_insert_with(|| {
                        let (present_event, following_event) =
                            events.remove(&service_id).unwrap_or_else(|| (None, None));

                        Service {
                            service_id,
                            pmt_pid: program.program_map_pid,
                            pcr_pid: Pid::NULL,
                            pcr: None,
                            pmt_filled: false,
                            video_streams: Vec::new(),
                            audio_streams: Vec::new(),
                            caption_stream: None,
                            superimpose_stream: None,
                            provider_name: AribString::new(),
                            service_name: AribString::new(),
                            present_event,
                            following_event,
                        }
                    });

                    // pat.pmtsの順に並べる
                    self.services.move_index(index, i);

                    ctx.table().set_as_psi(program.program_map_pid, Tag::Pmt);
                }

                // PATからいなくなったサービスを消す
                for (_, service) in self.services.drain(pat.pmts.len()..) {
                    ctx.table().unset(service.pmt_pid);
                    ctx.table().unset(service.pcr_pid);

                    for stream in service.all_streams() {
                        ctx.table().unset(stream.pid);
                    }
                }

                self.shooter.on_pat_updated(&self.services);
            }
            Tag::Pmt => {
                let Some(pmt) = self.repo.read::<psi::table::Pmt>(psi) else {
                    return;
                };
                let Some(service) = self.services.get_mut(&pmt.program_number) else {
                    self.repo.unset(psi);
                    return;
                };

                if service.pcr_pid != pmt.pcr_pid {
                    if pmt.pcr_pid != Pid::NULL {
                        if !ctx.table().is_custom(pmt.pcr_pid) {
                            ctx.table().set_as_custom(pmt.pcr_pid, Tag::Pcr);
                        }
                    } else {
                        ctx.table().unset(service.pcr_pid);
                    }
                    service.pcr_pid = pmt.pcr_pid;
                }

                let mut lost_pids: FxHashSet<Pid> = service.all_streams().map(|s| s.pid).collect();

                service.video_streams.clear();
                service.audio_streams.clear();
                service.caption_stream = None;
                service.superimpose_stream = None;

                for stream in &*pmt.streams {
                    let video_encode_format = stream
                        .descriptors
                        .get::<psi::desc::VideoDecodeControlDescriptor>()
                        .map(|vdcd| vdcd.video_encode_format);
                    let component_tag = stream
                        .descriptors
                        .get::<psi::desc::StreamIdDescriptor>()
                        .map(|sid| sid.component_tag);
                    let make_stream = || Stream {
                        pid: stream.elementary_pid,
                        stream_type: stream.stream_type,
                        component_tag,
                        video_encode_format,
                    };

                    let tag = match (stream.stream_type, component_tag) {
                        (t, _) if t.is_video() => {
                            service.video_streams.push(make_stream());
                            Tag::Video
                        }

                        (t, _) if t.is_audio() => {
                            service.audio_streams.push(make_stream());
                            Tag::Audio
                        }

                        (psi::desc::StreamType::CAPTION, Some(0x30 | 0x87))
                            if service.caption_stream.is_none() =>
                        {
                            // 字幕のデフォルトES
                            service.caption_stream = Some(make_stream());
                            Tag::Caption
                        }

                        (psi::desc::StreamType::CAPTION, Some(0x38 | 0x88))
                            if service.superimpose_stream.is_none() =>
                        {
                            // 文字スーパーのデフォルトES
                            service.superimpose_stream = Some(make_stream());
                            Tag::Superimpose
                        }

                        _ => continue,
                    };

                    if !ctx.table().is_pes(stream.elementary_pid) {
                        ctx.table().set_as_pes(stream.elementary_pid, tag);
                    }
                    lost_pids.remove(&stream.elementary_pid);
                }

                // コンポーネントタグの昇順でソート
                let f = |s: &Stream| s.component_tag;
                service.video_streams.sort_unstable_by_key(f);
                service.audio_streams.sort_unstable_by_key(f);
                service.pmt_filled = true;

                // 消えたPIDを設定解除
                for &lost_pid in &lost_pids {
                    ctx.table().unset(lost_pid);
                }

                self.shooter.on_pmt_updated(
                    &self.services,
                    self.services.get(&pmt.program_number).unwrap(),
                );
            }
            Tag::Sdt => {
                let Some(psi::table::Sdt::Actual(sdt)) = self.repo.read(psi) else {
                    return;
                };

                for svc in &*sdt.services {
                    let Some(service) = self.services.get_mut(&svc.service_id) else {
                        continue;
                    };
                    let Some(sd) = svc.descriptors.get::<psi::desc::ServiceDescriptor>() else {
                        continue;
                    };

                    sd.service_provider_name
                        .clone_into(&mut service.provider_name);
                    sd.service_name.clone_into(&mut service.service_name);
                }
            }
            Tag::Eit => {
                let Some(psi::table::Eit::ActualPf(eit)) = self.repo.read(psi) else {
                    return;
                };
                // TODO: transport_stream_idやoriginal_network_idをチェックすべき？
                let is_present = match eit.section_number {
                    0 => true,
                    1 => false,
                    _ => return,
                };

                let event = eit.events.get(0).map(|event| {
                    let (name, text) = if let Some(sed) =
                        event.descriptors.get::<psi::desc::ShortEventDescriptor>()
                    {
                        (Some(sed.event_name.to_owned()), Some(sed.text.to_owned()))
                    } else {
                        (None, None)
                    };

                    let mut extended_items = Vec::new();
                    for item in event
                        .descriptors
                        .get_all::<psi::desc::ExtendedEventDescriptor>()
                        .flat_map(|eed| eed.items)
                    {
                        match (item.item_description.is_empty(), extended_items.last_mut()) {
                            (false, _) | (true, None) => {
                                // 項目名がある、または最初の項目なので新規追加
                                extended_items.push(ExtendedEventItem {
                                    description: item.item_description.to_owned(),
                                    item: item.item.to_owned(),
                                });
                            }
                            (true, Some(last_item)) => {
                                // 項目名がないので項目継続
                                last_item.item.push_str(item.item);
                            }
                        }
                    }

                    let video_components = event
                        .descriptors
                        .get_all::<psi::desc::ComponentDescriptor>()
                        .map(|cd| VideoComponent {
                            stream_content: cd.stream_content,
                            component_type: cd.component_type,
                            component_tag: cd.component_tag,
                            lang_code: cd.lang_code,
                            text: cd.text.to_owned(),
                        })
                        .collect();

                    let audio_components = event
                        .descriptors
                        .get_all::<psi::desc::AudioComponentDescriptor>()
                        .map(|acd| AudioComponent {
                            stream_content: acd.stream_content,
                            component_type: acd.component_type,
                            component_tag: acd.component_tag,
                            stream_type: acd.stream_type,
                            simulcast_group_tag: acd.simulcast_group_tag,
                            main_component_flag: acd.main_component_flag,
                            quality_indicator: acd.quality_indicator,
                            sampling_rate: acd.sampling_rate,
                            lang_code: acd.lang_code,
                            lang_code_2: acd.lang_code_2,
                            text: acd.text.to_owned(),
                        })
                        .collect();

                    let genres = event
                        .descriptors
                        .get::<psi::desc::ContentDescriptor>()
                        .map(|cd| cd.genres.into());

                    EventInfo {
                        event_id: event.event_id,
                        start_time: event.start_time.clone(),
                        duration: event.duration,
                        name,
                        text,
                        extended_items,
                        video_components,
                        audio_components,
                        genres,
                    }
                });

                if let Some(service) = self.services.get_mut(&eit.service_id) {
                    if is_present {
                        service.present_event = event;
                    } else {
                        service.following_event = event;
                    }

                    self.shooter.on_eit_updated(
                        &self.services,
                        self.services.get(&eit.service_id).unwrap(),
                        is_present,
                    );
                } else {
                    let events = self
                        .events
                        .entry(eit.service_id)
                        .or_insert_with(Default::default);
                    if is_present {
                        events.0 = event;
                    } else {
                        events.1 = event;
                    }
                }
            }
            Tag::Tot => {
                let Some(tot) = self.repo.read::<psi::table::Tot>(psi) else {
                    return;
                };

                // ARIB TR-B14によりローカル時間オフセット記述子は1個のみである
                let time_offset = tot
                    .descriptors
                    .get::<psi::desc::LocalTimeOffsetDescriptor>()
                    .and_then(|ltod| ltod.time_offsets.get(0).cloned());

                self.shooter
                    .on_tot(&self.services, tot.jst_time, time_offset);
            }
            tag @ _ => {
                log::error!("invalid tag: {:?}", tag);
            }
        }
    }

    fn on_pes_packet(&mut self, ctx: &mut demux::Context<Self::Tag>, pes: &pes::PesPacket) {
        match ctx.tag() {
            Tag::Video => {
                let Some(option) = &pes.header.option else {
                    return;
                };

                self.shooter.on_video_packet(
                    &self.services,
                    ctx.packet().pid(),
                    option.pts,
                    option.dts,
                    pes.data,
                );
            }
            Tag::Audio => {
                let Some(option) = &pes.header.option else {
                    return;
                };

                self.shooter.on_audio_packet(
                    &self.services,
                    ctx.packet().pid(),
                    option.pts,
                    option.dts,
                    pes.data,
                );
            }
            tag @ (Tag::Caption | Tag::Superimpose) => {
                let Some(ipes) = pes::IndependentPes::read(pes.data) else {
                    return;
                };
                let Some(data_group) = pes::caption::DataGroup::read(ipes.data().pes_data) else {
                    return;
                };

                let group = match data_group.data_group_id & 0xF0 {
                    0x00 => CaptionGroup::GroupA,
                    0x20 => CaptionGroup::GroupB,
                    _ => return,
                };
                let caption = match data_group.data_group_id & 0x0F {
                    0 => {
                        let Some(data) =
                            pes::caption::CaptionManagementData::read(data_group.data_group_data)
                        else {
                            return;
                        };

                        Caption::ManagementData(CaptionManagementData {
                            group,
                            tmd: data.tmd,
                            otm: data.otm,
                            languages: data.languages,
                            data_units: data.data_units,
                        })
                    }
                    tag @ 1..=8 => {
                        let Some(data) =
                            pes::caption::CaptionData::read(data_group.data_group_data)
                        else {
                            return;
                        };

                        Caption::Data(CaptionData {
                            group,
                            language_tag: pes::caption::LanguageTag(tag - 1),
                            tmd: data.tmd,
                            stm: data.stm,
                            data_units: data.data_units,
                        })
                    }
                    _ => return,
                };

                let pts = pes.header.option.as_ref().and_then(|o| o.pts);
                if matches!(tag, Tag::Caption) {
                    self.shooter
                        .on_caption(&self.services, ctx.packet().pid(), pts, &caption);
                } else {
                    self.shooter
                        .on_superimpose(&self.services, ctx.packet().pid(), pts, &caption);
                }
            }
            tag @ _ => {
                log::debug!("invalid tag: {:?}", tag);
            }
        }
    }
}
