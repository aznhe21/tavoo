//! パケットを仕分けるためのフィルター。

use std::time::Duration;

use fxhash::FxHashSet;

use crate::demux;
use crate::packet;
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
    /// PATが更新された際に呼ばれる。
    fn on_pat_updated(&mut self, services: &ServiceMap);

    /// `service_id`のPMTが更新された際に呼ばれる。
    fn on_pmt_updated(&mut self, services: &ServiceMap, service: &Service);

    /// `service_id`のEITが更新された際に呼ばれる。
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
    fn on_caption(&mut self, services: &ServiceMap, pid: Pid, caption: &Caption);
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

/// PAT・PMTから送出されるサービス情報。
#[derive(Debug, Clone)]
pub struct Service {
    service_id: ServiceId,
    pmt_pid: Pid,
    pcr_pid: Pid,
    pcr: Option<packet::Pcr>,
    pmt_filled: bool,
    /// 映像ストリーム一覧。component_tagにより昇順に並ぶ
    video_streams: Vec<Stream>,
    /// 音声ストリーム一覧。component_tagにより昇順に並ぶ
    audio_streams: Vec<Stream>,
    /// 字幕ストリーム一覧。component_tagにより昇順に並ぶ
    caption_streams: Vec<Stream>,

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

    /// サービスにおける現在のPCR。
    #[inline]
    pub fn pcr(&self) -> Option<packet::Pcr> {
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

    /// 字幕ストリーム一覧。
    #[inline]
    pub fn caption_streams(&self) -> &[Stream] {
        &*self.caption_streams
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
    /// `video_tag`に`None`を指定した場合、デフォルトESを返す。
    ///
    /// ARIBの仕様上デフォルトESが必ず存在するが、
    /// データ上はストリームが存在しないこともあり得るため、
    /// このメソッドは`Option`を返すようにしている。
    pub fn find_video_stream(&self, video_tag: Option<u8>) -> Option<&Stream> {
        Self::find_stream(&*self.video_streams, video_tag)
    }

    /// `audio_tag`と一致するコンポーネントタグの音声ストリームを検索する。
    ///
    /// `audio_tag`に`None`を指定した場合、デフォルトESを返す。
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
    /// 番組の継続時間。
    pub duration: Duration,
    /// 番組名。
    pub name: Option<AribString>,
    /// 番組情報。
    pub text: Option<AribString>,
    /// 拡張番組情報。
    pub extended_items: Option<Vec<ExtendedEventItem>>,
}

/// 拡張番組情報の要素。
#[derive(Debug, Clone)]
pub struct ExtendedEventItem {
    /// 項目名。
    pub item: AribString,
    /// 概要。
    pub description: AribString,
}

/// 字幕データ。
#[derive(Debug)]
pub enum Caption<'a> {
    /// 字幕管理データ。
    ManagementData(pes::caption::CaptionManagementData<'a>),
    /// 字幕文データ。
    Data(pes::caption::CaptionData<'a>),
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
}

impl<T> Sorter<T> {
    /// `Sorter`を生成する。
    pub fn new(shooter: T) -> Sorter<T> {
        Sorter {
            shooter,
            repo: psi::Repository::new(),

            services: ServiceMap::default(),
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
        Pcr,

        // PES
        Video,
        Audio,
        Caption,
    }
}

use sealed::Tag;

impl<T: Shooter> demux::Filter for Sorter<T> {
    type Tag = Tag;

    fn on_setup(&mut self, table: &mut demux::Table<Self::Tag>) {
        table.set_as_psi(Pid::PAT, Tag::Pat);
        table.set_as_psi(Pid::SDT, Tag::Sdt);
        table.set_as_psi(Pid::H_EIT, Tag::Eit);
    }

    fn on_psi_section(&mut self, ctx: &mut demux::Context<Self::Tag>, psi: &psi::PsiSection) {
        match ctx.tag() {
            Tag::Pat => {
                let Some(pat) = self.repo.read::<psi::table::Pat>(psi) else {
                    return;
                };

                for (i, program) in pat.pmts.iter().enumerate() {
                    let service_id = program.program_number;

                    let entry = self.services.entry(service_id);
                    let index = entry.index();
                    entry.or_insert_with(|| Service {
                        service_id,
                        pmt_pid: program.program_map_pid,
                        pcr_pid: Pid::NULL,
                        pcr: None,
                        pmt_filled: false,
                        video_streams: Vec::new(),
                        audio_streams: Vec::new(),
                        caption_streams: Vec::new(),
                        provider_name: AribString::new(),
                        service_name: AribString::new(),
                        present_event: None,
                        following_event: None,
                    });

                    // pat.pmtsの順に並べる
                    self.services.move_index(index, i);

                    ctx.table().set_as_psi(program.program_map_pid, Tag::Pmt);
                }

                // PATからいなくなったサービスを消す
                for (_, service) in self.services.drain(pat.pmts.len()..) {
                    ctx.table().unset(service.pmt_pid);
                    ctx.table().unset(service.pcr_pid);

                    let mut unset_streams = |streams: &[Stream]| {
                        for stream in streams {
                            ctx.table().unset(stream.pid);
                        }
                    };
                    unset_streams(&*service.video_streams);
                    unset_streams(&*service.audio_streams);
                    unset_streams(&*service.caption_streams);
                }

                self.shooter.on_pat_updated(&self.services);
            }
            Tag::Pmt => {
                let Some(pmt) = self.repo.read::<psi::table::Pmt>(psi) else {
                    return;
                };
                let Some(service) = self.services.get_mut(&pmt.program_number) else {
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

                let mut lost_pids: FxHashSet<Pid> = std::iter::empty()
                    .chain(&*service.video_streams)
                    .chain(&*service.audio_streams)
                    .chain(&*service.caption_streams)
                    .map(|s| s.pid)
                    .collect();

                service.video_streams.clear();
                service.audio_streams.clear();
                service.caption_streams.clear();

                for stream in &*pmt.streams {
                    let video_encode_format = stream
                        .descriptors
                        .get::<psi::desc::VideoDecodeControlDescriptor>()
                        .map(|vdcd| vdcd.video_encode_format);
                    let component_tag = stream
                        .descriptors
                        .get::<psi::desc::StreamIdDescriptor>()
                        .map(|sid| sid.component_tag);

                    let streams = match stream.stream_type {
                        t if t.is_video() => {
                            if !ctx.table().is_pes(stream.elementary_pid) {
                                ctx.table().set_as_pes(stream.elementary_pid, Tag::Video);
                            }

                            &mut service.video_streams
                        }

                        t if t.is_audio() => {
                            if !ctx.table().is_pes(stream.elementary_pid) {
                                ctx.table().set_as_pes(stream.elementary_pid, Tag::Audio);
                            }

                            &mut service.audio_streams
                        }

                        psi::desc::StreamType::CAPTION => {
                            if !ctx.table().is_pes(stream.elementary_pid) {
                                ctx.table().set_as_pes(stream.elementary_pid, Tag::Caption);
                            }

                            &mut service.caption_streams
                        }

                        _ => continue,
                    };

                    streams.push(Stream {
                        pid: stream.elementary_pid,
                        stream_type: stream.stream_type,
                        component_tag,
                        video_encode_format,
                    });
                    lost_pids.remove(&stream.elementary_pid);
                }

                // コンポーネントタグの昇順でソート
                let f = |s: &Stream| s.component_tag;
                service.video_streams.sort_unstable_by_key(f);
                service.audio_streams.sort_unstable_by_key(f);
                service.caption_streams.sort_unstable_by_key(f);
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
                let is_present = match &psi.syntax {
                    Some(syntax) if syntax.section_number == 0 => true,
                    Some(syntax) if syntax.section_number == 1 => false,
                    _ => return,
                };
                let Some(psi::table::Eit::ActualPf(eit)) = self.repo.read(psi) else {
                    return;
                };
                // TODO: transport_stream_idやoriginal_network_idもチェックすべき？
                let Some(service) = self.services.get_mut(&eit.service_id) else {
                    return;
                };

                let event = eit.events.get(0).map(|event| {
                    let (name, text) = if let Some(sed) =
                        event.descriptors.get::<psi::desc::ShortEventDescriptor>()
                    {
                        (Some(sed.event_name.to_owned()), Some(sed.text.to_owned()))
                    } else {
                        (None, None)
                    };
                    let extended_items = event
                        .descriptors
                        .get::<psi::desc::ExtendedEventDescriptor>()
                        .map(|eed| {
                            eed.items
                                .iter()
                                .map(|item| ExtendedEventItem {
                                    item: item.item.to_owned(),
                                    description: item.item_description.to_owned(),
                                })
                                .collect()
                        });
                    EventInfo {
                        event_id: event.event_id,
                        start_time: event.start_time.clone(),
                        duration: Duration::from_secs(event.duration as u64),
                        name,
                        text,
                        extended_items,
                    }
                });

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
            Tag::Caption => {
                let Some(pes) = pes::IndependentPes::read(pes.data) else {
                    return;
                };
                let Some(data_group) = pes::caption::DataGroup::read(pes.data().pes_data) else {
                    return;
                };

                let caption = if matches!(data_group.data_group_id, 0x00 | 0x20) {
                    use pes::caption::CaptionManagementData;
                    let Some(management) = CaptionManagementData::read(data_group.data_group_data)
                    else {
                        return;
                    };

                    Caption::ManagementData(management)
                } else {
                    let Some(caption) = pes::caption::CaptionData::read(data_group.data_group_data)
                    else {
                        return;
                    };

                    Caption::Data(caption)
                };

                self.shooter
                    .on_caption(&self.services, ctx.packet().pid(), &caption);
            }
            tag @ _ => {
                log::debug!("invalid tag: {:?}", tag);
            }
        }
    }

    fn on_custom_packet(&mut self, ctx: &mut demux::Context<Self::Tag>, _: bool) {
        match ctx.tag() {
            Tag::Pcr => {
                let Some(pcr) = ctx.packet().adaptation_field().and_then(|af| af.pcr()) else {
                    return;
                };

                let pid = ctx.packet().pid();
                for service in self.services.values_mut() {
                    if service.pcr_pid == pid {
                        service.pcr = Some(pcr);
                    }
                }
            }
            tag @ _ => {
                log::debug!("invalid tag: {:?}", tag);
            }
        }
    }
}
