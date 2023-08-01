//! TSファイルを別スレッドで順次処理する。

mod caption;

use std::fmt;
use std::io::{self, Read, Seek, SeekFrom};
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use isdb::filters::sorter::{Service, ServiceMap, Stream};
use isdb::psi::table::ServiceId;
use isdb::time::{DateTime, Timestamp};
use parking_lot::RwLock;

use crate::ring_buf::RingBuf;

/// 映像・音声ストリームの変更通知。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamChanged {
    /// 映像ストリームのPIDが変わった。
    pub video_pid: bool,
    /// 映像ストリームの形式が変わった。
    pub video_type: bool,
    /// 音声ストリームのPIDが変わった。
    pub audio_pid: bool,
    /// 音声ストリームの形式が変わった。
    pub audio_type: bool,
}

impl StreamChanged {
    #[inline]
    fn new(
        old_streams: Option<(&Stream, &Stream)>,
        (new_video_stream, new_audio_stream): (&Stream, &Stream),
    ) -> StreamChanged {
        match old_streams {
            Some((old_video_stream, old_audio_stream)) => StreamChanged {
                video_pid: old_video_stream.pid() != new_video_stream.pid(),
                video_type: old_video_stream.stream_type() != new_video_stream.stream_type(),
                audio_pid: old_audio_stream.pid() != new_audio_stream.pid(),
                audio_type: old_audio_stream.stream_type() != new_audio_stream.stream_type(),
            },
            None => StreamChanged {
                video_pid: true,
                video_type: true,
                audio_pid: true,
                audio_type: true,
            },
        }
    }

    #[inline]
    fn any(&self) -> bool {
        self.video_pid || self.video_type || self.audio_pid || self.audio_type
    }
}

/// [`Extractor`]で処理されたTSの情報を受け取るためのトレイト。
pub trait Sink {
    /// TSのサービス一覧が更新された際に呼ばれる。
    ///
    /// サービスの選択状態によってはこの直後にサービスが変更される可能性がある。
    fn on_services_updated(&mut self, services: &ServiceMap);

    /// サービスのストリームが更新された際に呼ばれる。
    fn on_streams_updated(&mut self, service: &Service);

    /// サービスのイベントが更新された際に呼ばれる。
    fn on_event_updated(&mut self, service: &Service, is_present: bool);

    /// サービスが選択し直された際に呼ばれる。
    fn on_service_changed(&mut self, service: &Service);

    /// 選択中サービスのストリームについて何かが変更された際に呼ばれる。
    ///
    /// `immediate`はストリームを即座に切り替える必要がある場合（サービス切り替えやシーク時など）に
    /// `true`となる。
    fn on_stream_changed(&mut self, immediate: bool, changed: StreamChanged);

    /// 選択中サービスで映像パケットを受信した際に呼ばれる。
    fn on_video_packet(&mut self, pos: Option<Duration>, payload: &[u8]);

    /// 選択中サービスで音声パケットを受信した際に呼ばれる。
    fn on_audio_packet(&mut self, pos: Option<Duration>, payload: &[u8]);

    /// 選択中サービスで字幕パケットを受信した際に呼ばれる。
    fn on_caption(&mut self, pos: Option<Duration>, caption: &isdb::filters::sorter::Caption);

    /// 選択中サービスで文字スーパーのパケットを受信した際に呼ばれる。
    fn on_superimpose(&mut self, pos: Option<Duration>, caption: &isdb::filters::sorter::Caption);

    /// TS内の日付時刻が更新された際に呼ばれる。ただし[`ExtractHandler::timestamp`]はより細かい間隔で更新される。
    ///
    /// `timestamp`は更新された日付時刻で、1900年1月1日からの経過時間によって表される。
    fn on_timestamp_updated(&mut self, timestamp: Duration);

    /// TSを終端まで読み終えた際に呼ばれる。
    fn on_end_of_stream(&mut self);

    /// TS読み取り中にエラーが発生した際に呼ばれる。
    ///
    /// TSの読み取りは終了する。
    fn on_stream_error(&mut self, error: io::Error);

    /// ESを読み取る必要があるかどうかを返す。
    ///
    /// このメソッドが`false`を返すとESの読み取りが一時停止する。
    fn needs_es(&self) -> bool;
}

/// [`Extractor::spawn`]に指定した読み取り元からストリームが見つからなかった。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct StreamProbeError;

impl fmt::Display for StreamProbeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("ストリームが見つかりませんでした")
    }
}

impl std::error::Error for StreamProbeError {}

/// `ExtractHandler`を通した指示。
// すべて`0`または`false`が指示無しの値なので`#[derive(Default)]`できる。
#[derive(Debug, Default)]
struct Commands {
    /// 何らかの指示が格納されているかどうかを示す値。
    has_any: AtomicBool,
    /// サービスを選択する。
    // 0は指示無し、1は`None`、2～は`Some(ServiceId)`
    select_service: AtomicU32,
    /// 映像ストリームを選択する。
    // 0は指示無し、1～は`Some(u8)`
    select_video_stream: AtomicU16,
    /// 音声ストリームを選択する。
    // 0は指示無し、1～は`Some(u8)`
    select_audio_stream: AtomicU16,
    /// 再生位置の秒成分を設定する。
    // 0は指示無し、1～は`Some(秒)`
    set_position_secs: AtomicU64,
    /// 再生位置のナノ秒成分を設定する。
    // 指示の有無はset_position_secsで指定する
    set_position_nanos: AtomicU32,
    /// TSをリセットして最初から再生する。
    reset: AtomicBool,
    /// TSの読み取りを終了する。
    shutdown: AtomicBool,
}

/// 処理中のTSにおける状態。
#[derive(Debug, Default)]
struct State {
    // TODO: 追っかけ再生に対応
    length: Option<StreamLength>,
    services: ServiceMap,
    selected_stream: Option<SelectedStream>,
    tot: Option<Tot>,
}

/// TSを処理するオブジェクト。
///
/// [`Extractor::handler`]によって取得できる[`ExtractHandler`]を通し、
/// このオブジェクトに指示を出す、またはこのオブジェクトから状態を取得することができる。
// 実際のところ、処理を行うのは`Worker`である。
pub struct Extractor {
    state: Arc<RwLock<State>>,
    commands: Arc<Commands>,
    parker: crossbeam_utils::sync::Parker,
    capacity: usize,
    probe_size: u64,
    tail_probe_size: u64,
}

impl Extractor {
    /// `Extractor`を生成する。
    pub fn new() -> Extractor {
        let state = Arc::new(RwLock::new(State::default()));
        let commands = Arc::new(Commands::default());

        Extractor {
            state,
            commands,
            parker: crossbeam_utils::sync::Parker::new(),
            capacity: 188 * 32,
            probe_size: 188 * 4096,
            tail_probe_size: 188 * 1024,
        }
    }

    /// `Extractor`に指示を与えるための[`ExtractHandler`]を取得する。
    #[inline]
    pub fn handler(&self) -> ExtractHandler {
        ExtractHandler {
            state: self.state.clone(),
            commands: self.commands.clone(),
            unparker: self.parker.unparker().clone(),
        }
    }

    /// TSを読み取るのに使うバッファの容量を設定する。
    #[inline]
    pub fn capacity(&mut self, capacity: usize) {
        self.capacity = capacity;
    }

    /// ストリーム情報を初期化する際に解析する最大の容量を設定する。
    ///
    /// ffmpegの`-probesize`に近い。
    #[inline]
    pub fn probe_size(&mut self, probe_size: u64) {
        self.probe_size = probe_size;
    }

    /// ストリーム長を取得するために末尾から解析する際の容量を設定する。
    #[inline]
    pub fn tail_probe_size(&mut self, tail_probe_size: u64) {
        self.tail_probe_size = tail_probe_size;
    }

    /// 指定された読み取り元`Read`と処理用`Sink`を使い、新しいスレッドで`Extractor`の処理を開始する。
    ///
    /// 戻り値の[`JoinHandle`][std::thread::JoinHandle]を使って終了待ちができるが、
    /// スレッドを終了させるためには事前に[`ExtractHandler::shutdown`]を呼び出す必要がある。
    pub fn spawn<R, T>(self, read: R, sink: T) -> std::thread::JoinHandle<()>
    where
        R: Read + Seek + Send + 'static,
        T: Sink + Send + 'static,
    {
        let read = io::BufReader::with_capacity(self.capacity, read);
        let demuxer = isdb::demux::Demuxer::new(isdb::filters::sorter::Sorter::new(Selector::new(
            sink, read, self.state,
        )));

        let worker = Worker {
            parker: self.parker,
            commands: self.commands,
            state: WorkerState::Working,

            demuxer,
            probe_size: self.probe_size,
            tail_probe_size: self.tail_probe_size,
        };
        std::thread::spawn(move || worker.run())
    }
}

/// [`ExtractHandler`]を通した`Extractor`への要求時に発生するエラー。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtractorError {
    /// 既にシャットダウン済み。
    AlreadyShutdown,

    /// シークできないストリーム。
    Unseekable,
}

impl fmt::Display for ExtractorError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ExtractorError::AlreadyShutdown => f.write_str("既にシャットダウン済み"),
            ExtractorError::Unseekable => f.write_str("シークできないストリーム"),
        }
    }
}

impl std::error::Error for ExtractorError {}

/// TS処理について、指示を出す、または状態を取得するためのオブジェクト。
#[derive(Debug, Clone)]
pub struct ExtractHandler {
    /// 現在の状態。
    state: Arc<RwLock<State>>,
    /// 指示が格納される構造体。
    commands: Arc<Commands>,
    /// 指示が出された際にワーカースレッドを起床させるためのハンドル。
    unparker: crossbeam_utils::sync::Unparker,
}

impl ExtractHandler {
    fn check_shutdown(&self) -> Result<(), ExtractorError> {
        if self.commands.shutdown.load(Ordering::SeqCst) {
            Err(ExtractorError::AlreadyShutdown)
        } else {
            Ok(())
        }
    }

    /// ストリームの長さを返す。
    ///
    /// ストリーム長が不明な場合は`None`を返す。
    #[inline]
    pub fn duration(&self) -> Option<Duration> {
        self.state.read().length.as_ref().map(|br| br.duration())
    }

    /// TOTとPCRによって計算される、1900年1月1日からの経過時間を返す。
    pub fn timestamp(&self) -> Option<Duration> {
        let state = self.state.read();
        let tot = state.tot.as_ref()?;

        let (_, default_service) = state.services.first()?;
        let pcr = default_service.pcr()?;

        Some(tot.timestamp(pcr))
    }

    /// 現在のサービス一覧を返す。
    ///
    /// 戻り値はロックを保持しているため、できるだけ早く破棄すべきである。
    pub fn services(&self) -> parking_lot::MappedRwLockReadGuard<ServiceMap> {
        parking_lot::RwLockReadGuard::map(self.state.read(), |s| &s.services)
    }

    /// 選択中のサービス・ストリームに関する情報を返す。
    ///
    /// 戻り値はロックを保持しているため、できるだけ早く破棄すべきである。
    pub fn selected_stream(&self) -> parking_lot::MappedRwLockReadGuard<Option<SelectedStream>> {
        parking_lot::RwLockReadGuard::map(self.state.read(), |s| &s.selected_stream)
    }

    /// ESを要求する。
    ///
    /// このメソッドを呼び出した際、[`Sink::needs_es`]は`true`を返すべきである。
    pub fn request_es(&self) -> Result<(), ExtractorError> {
        self.check_shutdown()?;

        self.unparker.unpark();
        Ok(())
    }

    /// サービス選択を指示する。
    ///
    /// `service_id`に`None`を指定した場合、既定のサービスが選択される。
    pub fn select_service(&self, service_id: Option<ServiceId>) -> Result<(), ExtractorError> {
        self.check_shutdown()?;

        let service_id = service_id.map_or(0, |id| id.get());
        self.commands
            .select_service
            .store(service_id as u32 + 1, Ordering::SeqCst);
        self.commands.has_any.store(true, Ordering::SeqCst);
        self.unparker.unpark();
        Ok(())
    }

    /// 映像ストリームの選択を指示する。
    pub fn select_video_stream(&self, component_tag: u8) -> Result<(), ExtractorError> {
        self.check_shutdown()?;

        self.commands
            .select_video_stream
            .store(component_tag as u16 + 1, Ordering::SeqCst);
        self.commands.has_any.store(true, Ordering::SeqCst);
        self.unparker.unpark();
        Ok(())
    }

    /// 音声ストリームの選択を指示する。
    pub fn select_audio_stream(&self, component_tag: u8) -> Result<(), ExtractorError> {
        self.check_shutdown()?;

        self.commands
            .select_audio_stream
            .store(component_tag as u16 + 1, Ordering::SeqCst);
        self.commands.has_any.store(true, Ordering::SeqCst);
        self.unparker.unpark();
        Ok(())
    }

    /// 再生位置の設定を指示する。
    pub fn set_position(&self, pos: Duration) -> Result<(), ExtractorError> {
        self.check_shutdown()?;

        if self.state.read().length.is_none() {
            return Err(ExtractorError::Unseekable);
        }

        // 秒が`u64::MAX`になるようなシークはしないと思われ
        self.commands
            .set_position_secs
            .store(pos.as_secs().saturating_add(1), Ordering::SeqCst);
        self.commands
            .set_position_nanos
            .store(pos.subsec_nanos(), Ordering::SeqCst);
        self.commands.has_any.store(true, Ordering::SeqCst);
        self.unparker.unpark();
        Ok(())
    }

    /// TSをリセットし最初から再生し直すことを指示する。
    pub fn reset(&self) -> Result<(), ExtractorError> {
        self.check_shutdown()?;

        self.commands.reset.store(true, Ordering::SeqCst);
        self.commands.has_any.store(true, Ordering::SeqCst);
        self.unparker.unpark();
        Ok(())
    }

    /// 処理の終了を指示する。
    ///
    /// このメソッドを呼び出してもすぐに処理が終わるわけではない。
    /// 処理が終わるまで待機するには[`Extractor::spawn`]の戻り値を使用する。
    pub fn shutdown(&self) {
        self.commands.shutdown.store(true, Ordering::SeqCst);
        self.commands.has_any.store(true, Ordering::SeqCst);
        self.unparker.unpark();
    }
}

/// 常にソート済みで重複のない集合。
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct SortedSet<T>(Vec<T>);

impl<T: Ord> SortedSet<T> {
    #[inline]
    pub const fn new() -> SortedSet<T> {
        SortedSet(Vec::new())
    }

    #[inline]
    pub fn insert(&mut self, value: T) {
        match self.0.binary_search(&value) {
            // 同値を挿入済み
            Ok(_) => {}
            Err(index) => self.0.insert(index, value),
        }
    }
}

impl<T> std::ops::Deref for SortedSet<T> {
    type Target = [T];

    #[inline]
    fn deref(&self) -> &[T] {
        &*self.0
    }
}

/// 選択されたサービス・ストリームの情報。
#[derive(Debug, Clone)]
pub struct SelectedStream {
    /// 選択されたサービスのサービス識別。
    pub service_id: ServiceId,
    /// 選択された映像のストリーム情報。
    pub video_stream: Stream,
    /// 選択された音声のストリーム情報。
    pub audio_stream: Stream,
    /// 選択されたサービスにおける字幕のPID。
    pub caption_pid: Option<isdb::Pid>,
    /// 選択されたサービスにおける文字スーパーのPID。
    pub superimpose_pid: Option<isdb::Pid>,
}

/// 再生時間。
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct PlaybackTime {
    prev_ts: Option<Timestamp>,
    duration: Duration,
}

impl PlaybackTime {
    /// 現在の再生時間を更新する。
    ///
    /// 前パケットとの差分を積分していくことでラップアラウンドを回避する。
    pub fn update(&mut self, new_ts: Timestamp) {
        if let Some(prev_ts) = self.prev_ts.replace(new_ts) {
            if new_ts >= prev_ts {
                self.duration += (new_ts - prev_ts).to_duration();
            } else {
                // PTSが遡るパケット
                self.duration = self
                    .duration
                    .saturating_sub((prev_ts - new_ts).to_duration());
            }
        } else {
            // シークでは`position`は非ゼロだが`prev_ts`が`None`の場合がある
        }
    }
}

#[derive(Debug, Clone)]
struct StreamLength {
    first_pcr: Timestamp,
    last_pcr: Timestamp,
    size: u64,
}

impl StreamLength {
    #[inline]
    pub fn duration(&self) -> Duration {
        // TODO: 2回以上のラップアラウンドを考慮する？
        (self.last_pcr - self.first_pcr).to_duration()
    }

    /// 時間からファイルサイズを推定する。
    pub fn estimate_size(&self, dur: Duration) -> u64 {
        (self.size as f64 * (dur.as_secs_f64() / self.duration().as_secs_f64())) as u64
    }
}

#[derive(Debug)]
struct Tot {
    datetime: DateTime,
    base_pcr: Timestamp,
}

impl Tot {
    fn timestamp(&self, pcr: Timestamp) -> Duration {
        const NANOS_PER_SECS: u64 = 1_000_000_000;

        let diff = (pcr - self.base_pcr).as_nanos();
        let secs = self.datetime.ntp_timestamp() + diff / NANOS_PER_SECS;
        let nanos = (diff % NANOS_PER_SECS) as u32;

        Duration::new(secs, nanos)
    }
}

#[derive(Debug)]
enum Caption {
    Caption(caption::Caption),
    Superimpose(caption::Caption),
}

#[derive(Debug)]
struct SeekInfo {
    /// シーク先の位置。
    target_pos: Duration,

    /// シーク開始時に選択されていたストリーム。
    orig_stream: Option<SelectedStream>,

    /// 保留する各種イベント。
    pat_updated: bool,
    eit_updated: SortedSet<(ServiceId, bool)>,
    pmt_updated: SortedSet<ServiceId>,

    /// 保留する字幕・文字スーパー。
    last_captions: RingBuf<(Option<Duration>, Caption), 10>,
}

#[derive(Debug)]
struct Selector<R, T> {
    read: PositionedRead<io::BufReader<R>>,
    sink: T,

    state: Arc<RwLock<State>>,
    /// ESのPIDからサービス識別を得るテーブル。
    es2svc: isdb::pid::PidTable<Option<ServiceId>>,
    /// 既定サービスのPCRを元にした再生位置。
    pcr_time: PlaybackTime,
    /// シーク中の情報。シークが完了したら`None`が設定される。
    seek_info: Option<SeekInfo>,
}

impl<R: Read + Seek, T: Sink> Selector<R, T> {
    #[inline]
    fn new(sink: T, read: io::BufReader<R>, state: Arc<RwLock<State>>) -> Selector<R, T> {
        Selector {
            read: PositionedRead::new(read),
            sink,

            state,
            es2svc: isdb::pid::PidTable::from_fn(|_| None),
            pcr_time: PlaybackTime::default(),
            seek_info: None,
        }
    }

    fn select_service(&mut self, services: &ServiceMap, service_id: Option<ServiceId>) {
        let service = if let Some(service_id) = service_id {
            let Some(service) = services.get(&service_id) else {
                log::info!("select_service：指定されたサービスが存在しない");
                return;
            };

            service
        } else {
            let Some((_, service)) = services.first() else {
                log::error!("select_service：サービスが存在しない");
                return;
            };

            service
        };

        if !service.pmt_filled() {
            log::trace!("select_service：PMT未受信");
            return;
        }

        let changed = {
            let mut state = self.state.write();
            if matches!(&state.selected_stream, Some(ss) if ss.service_id == service.service_id()) {
                // サービスが変わらないので何もしない
                return;
            }

            let (video_tag, audio_tag, old_streams) = match &state.selected_stream {
                Some(ss) => (
                    ss.video_stream.component_tag(),
                    ss.audio_stream.component_tag(),
                    Some((&ss.video_stream, &ss.audio_stream)),
                ),
                None => (None, None, None),
            };

            let Some(video_stream) = service.find_video_stream(video_tag) else {
                log::info!("select_service：映像ストリームが存在しない");
                return;
            };
            let Some(audio_stream) = service.find_audio_stream(audio_tag) else {
                log::info!("select_service：音声ストリームが存在しない");
                return;
            };

            let changed = StreamChanged::new(old_streams, (video_stream, audio_stream));

            state.selected_stream = Some(SelectedStream {
                service_id: service.service_id(),
                video_stream: video_stream.clone(),
                audio_stream: audio_stream.clone(),
                caption_pid: service.caption_stream().map(|s| s.pid()),
                superimpose_pid: service.superimpose_stream().map(|s| s.pid()),
            });

            changed
        };

        // シーク中はイベント発生を保留
        if self.seek_info.is_none() {
            self.sink.on_service_changed(service);
            if changed.any() {
                self.sink.on_stream_changed(true, changed);
            }
        }
    }

    fn select_video_stream(&mut self, services: &ServiceMap, component_tag: u8) {
        let changed = {
            let mut state = self.state.write();
            let Some(selected_stream) = state.selected_stream.as_mut() else {
                log::debug!("select_video_stream：サービス未選択");
                return;
            };

            let service = &services[&selected_stream.service_id];
            let Some(video_stream) = service.find_video_stream(Some(component_tag)) else {
                log::info!("select_video_stream：映像ストリームが存在しない");
                return;
            };

            let video_pid_changed = selected_stream.video_stream.pid() != video_stream.pid();
            let video_type_changed =
                selected_stream.video_stream.stream_type() != video_stream.stream_type();

            selected_stream.video_stream = video_stream.clone();

            StreamChanged {
                video_pid: video_pid_changed,
                video_type: video_type_changed,
                audio_pid: false,
                audio_type: false,
            }
        };

        // シーク中はイベント発生を保留
        if self.seek_info.is_none() && (changed.video_pid || changed.video_type) {
            self.sink.on_stream_changed(false, changed);
        }
    }

    fn select_audio_stream(&mut self, services: &ServiceMap, component_tag: u8) {
        let changed = {
            let mut state = self.state.write();
            let Some(selected_stream) = state.selected_stream.as_mut() else {
                log::debug!("select_audio_stream：サービス未選択");
                return;
            };

            let service = &services[&selected_stream.service_id];
            let Some(audio_stream) = service.find_audio_stream(Some(component_tag)) else {
                log::info!("select_audio_stream：音声ストリームが存在しない");
                return;
            };

            let audio_pid_changed = selected_stream.audio_stream.pid() != audio_stream.pid();
            let audio_type_changed =
                selected_stream.audio_stream.stream_type() != audio_stream.stream_type();

            selected_stream.audio_stream = audio_stream.clone();

            StreamChanged {
                video_pid: false,
                video_type: false,
                audio_pid: audio_pid_changed,
                audio_type: audio_type_changed,
            }
        };

        // シーク中はイベント発生を保留
        if self.seek_info.is_none() && (changed.audio_pid || changed.audio_type) {
            self.sink.on_stream_changed(false, changed);
        }
    }

    /// サービスが未選択の場合はパニックする。
    fn update_es(&mut self, service: &Service) {
        // ESの変更に追従
        let changed = {
            let mut state = self.state.write();
            let selected_stream = state.selected_stream.as_mut().expect("サービス未選択");

            let video_tag = selected_stream.video_stream.component_tag();
            let audio_tag = selected_stream.audio_stream.component_tag();

            let Some(video_stream) = service.find_video_stream(video_tag) else {
                log::info!("update_es：映像ストリームが存在しない");
                return;
            };
            let Some(audio_stream) = service.find_audio_stream(audio_tag) else {
                log::info!("update_es：音声ストリームが存在しない");
                return;
            };

            let changed = StreamChanged::new(
                Some((&selected_stream.video_stream, &selected_stream.audio_stream)),
                (video_stream, audio_stream),
            );

            selected_stream.video_stream = video_stream.clone();
            selected_stream.audio_stream = audio_stream.clone();
            selected_stream.caption_pid = service.caption_stream().map(|s| s.pid());
            selected_stream.superimpose_pid = service.superimpose_stream().map(|s| s.pid());

            changed
        };
        // シーク中はイベント発生を保留
        if self.seek_info.is_none() && changed.any() {
            self.sink.on_stream_changed(false, changed);
        }
    }

    /// 再生時間がシーク対象以降であればシークを完了させる。
    ///
    /// シークが完了した場合には保留していたイベントを発生させる。
    fn complete_seek(&mut self) {
        let seek_info = match &mut self.seek_info {
            // target_posまでは何も処理しない
            Some(seek_info) if self.pcr_time.duration >= seek_info.target_pos => seek_info,
            _ => return,
        };

        // 保留していたイベントを発生させる
        let state = self.state.read();
        if seek_info.pat_updated {
            self.sink.on_services_updated(&state.services);
        }
        for &(service_id, is_present) in &*seek_info.eit_updated {
            self.sink
                .on_event_updated(&state.services[&service_id], is_present);
        }
        for service_id in &*seek_info.pmt_updated {
            self.sink.on_streams_updated(&state.services[service_id]);
        }
        // 保留していた字幕を放流
        for &(pos, ref caption) in &seek_info.last_captions {
            match caption {
                Caption::Caption(caption) => self.sink.on_caption(pos, &caption.into()),
                Caption::Superimpose(caption) => self.sink.on_superimpose(pos, &caption.into()),
            };
        }

        // `selected_stream`が`Some`から`None`になることはないことから、
        // `selected_stream`が`None`の場合は変更なしのため`Some`だけ見れば良い
        if let Some(selected_stream) = &state.selected_stream {
            let (old_service_id, old_streams) = match &seek_info.orig_stream {
                Some(ss) => (
                    Some(ss.service_id),
                    Some((&ss.video_stream, &ss.audio_stream)),
                ),
                None => (None, None),
            };

            let changed = StreamChanged::new(
                old_streams,
                (&selected_stream.video_stream, &selected_stream.audio_stream),
            );

            if old_service_id != Some(selected_stream.service_id) {
                self.sink
                    .on_service_changed(&state.services[&selected_stream.service_id]);
            }
            if changed.any() {
                self.sink.on_stream_changed(true, changed);
            }
        }

        self.seek_info = None;
    }
}

impl<R: Read + Seek, T: Sink> isdb::filters::sorter::Shooter for Selector<R, T> {
    fn on_pat_updated(&mut self, services: &ServiceMap) {
        self.state.write().services.clone_from(services);

        // シーク中はイベント発生を保留
        if let Some(seek_info) = &mut self.seek_info {
            seek_info.pat_updated = true;
        } else {
            self.sink.on_services_updated(services);
        }

        let do_select = match &self.state.read().selected_stream {
            // サービス未選択
            None => true,
            // 選択中のサービスがなくなった
            Some(selected_stream) => !services.contains_key(&selected_stream.service_id),
        };
        if do_select {
            self.select_service(services, None);
        }
    }

    fn on_pmt_updated(&mut self, services: &ServiceMap, service: &Service) {
        self.state.write().services.clone_from(services);

        self.es2svc.fill(None);
        for service in services.values().rev() {
            std::iter::empty()
                .chain(service.video_streams())
                .chain(service.audio_streams())
                .chain(service.caption_stream())
                .chain(service.superimpose_stream())
                .for_each(|stream| self.es2svc[stream.pid()] = Some(service.service_id()));
        }

        // シーク中はイベント発生を保留
        if let Some(seek_info) = &mut self.seek_info {
            seek_info.pmt_updated.insert(service.service_id());
        } else {
            self.sink.on_streams_updated(service);
        }

        let selected_service_id = {
            let state = self.state.read();
            state.selected_stream.as_ref().map(|ss| ss.service_id)
        };
        match selected_service_id {
            None => self.select_service(services, None),
            Some(service_id) if service_id == service.service_id() => self.update_es(service),
            Some(_) => {}
        }
    }

    fn on_eit_updated(&mut self, services: &ServiceMap, service: &Service, is_present: bool) {
        self.state.write().services.clone_from(services);

        // シーク中はイベント発生を保留
        if let Some(seek_info) = &mut self.seek_info {
            seek_info
                .eit_updated
                .insert((service.service_id(), is_present));
        } else {
            self.sink.on_event_updated(service, is_present);
        }
    }

    fn on_video_packet(
        &mut self,
        services: &ServiceMap,
        pid: isdb::Pid,
        pts: Option<Timestamp>,
        _: Option<Timestamp>,
        payload: &[u8],
    ) {
        let Some(service_id) = self.es2svc[pid] else { return };

        // シーク中はパケットを処理しない
        if self.seek_info.is_some() {
            return;
        }

        {
            let state = self.state.read();
            if !matches!(&state.selected_stream, Some(ss) if ss.video_stream.pid() == pid) {
                return;
            }
        }

        // 選択中サービスのPCRと既定サービスのPCRが同じ時刻と見做して位置を計算する
        let pos = pts.and_then(|pts| {
            let pcr = services.get(&service_id)?.pcr()?;
            Some(self.pcr_time.duration + (pts - pcr).to_duration())
        });
        self.sink.on_video_packet(pos, payload);
    }

    fn on_audio_packet(
        &mut self,
        services: &ServiceMap,
        pid: isdb::Pid,
        pts: Option<Timestamp>,
        _: Option<Timestamp>,
        payload: &[u8],
    ) {
        let Some(service_id) = self.es2svc[pid] else { return };

        // シーク中はパケットを処理しない
        if self.seek_info.is_some() {
            return;
        }

        {
            let state = self.state.read();
            if !matches!(&state.selected_stream, Some(ss) if ss.audio_stream.pid() == pid) {
                return;
            }
        }

        // 選択中サービスのPCRと既定サービスのPCRが同じ時刻と見做して位置を計算する
        let pos = pts.and_then(|pts| {
            let pcr = services.get(&service_id)?.pcr()?;
            Some(self.pcr_time.duration + (pts - pcr).to_duration())
        });
        self.sink.on_audio_packet(pos, payload);
    }

    fn on_caption(
        &mut self,
        services: &ServiceMap,
        pid: isdb::Pid,
        pts: Option<Timestamp>,
        caption: &isdb::filters::sorter::Caption,
    ) {
        let Some(service_id) = self.es2svc[pid] else { return };

        {
            let state = self.state.read();
            if !matches!(&state.selected_stream, Some(ss) if ss.caption_pid == Some(pid)) {
                return;
            }
        }

        // 選択中サービスのPCRと既定サービスのPCRが同じ時刻と見做して位置を計算する
        let pos = pts.and_then(|pts| {
            let pcr = services.get(&service_id)?.pcr()?;
            Some(self.pcr_time.duration + (pts - pcr).to_duration())
        });

        if let Some(seek_info) = &mut self.seek_info {
            seek_info
                .last_captions
                .push((pos, Caption::Caption(caption::Caption::new(caption))));
        } else {
            self.sink.on_caption(pos, caption);
        }
    }

    fn on_superimpose(
        &mut self,
        services: &ServiceMap,
        pid: isdb::Pid,
        pts: Option<Timestamp>,
        caption: &isdb::filters::sorter::Caption,
    ) {
        let Some(service_id) = self.es2svc[pid] else { return };

        {
            let state = self.state.read();
            if !matches!(&state.selected_stream, Some(ss) if ss.superimpose_pid == Some(pid)) {
                return;
            }
        }

        // 選択中サービスのPCRと既定サービスのPCRが同じ時刻と見做して位置を計算する
        let pos = pts.and_then(|pts| {
            let pcr = services.get(&service_id)?.pcr()?;
            Some(self.pcr_time.duration + (pts - pcr).to_duration())
        });

        if let Some(seek_info) = &mut self.seek_info {
            seek_info
                .last_captions
                .push((pos, Caption::Superimpose(caption::Caption::new(caption))));
        } else {
            self.sink.on_superimpose(pos, caption);
        }
    }

    fn on_pcr(&mut self, services: &ServiceMap, service_ids: &[ServiceId]) {
        self.state.write().services.clone_from(services);

        let Some((_, service)) = services.first() else {
            return;
        };
        if !service_ids.contains(&service.service_id()) {
            return;
        }

        self.pcr_time.update(service.pcr().expect("PCRは更新済み"));
        self.complete_seek();
    }

    fn on_tot(
        &mut self,
        _: &ServiceMap,
        datetime: DateTime,
        _: Option<isdb::psi::desc::LocalTimeOffsetEntry>,
    ) {
        if let Some(base_pcr) = self.pcr_time.prev_ts {
            self.state.write().tot = Some(Tot { datetime, base_pcr });

            if self.seek_info.is_none() {
                self.sink
                    .on_timestamp_updated(Duration::from_secs(datetime.ntp_timestamp()));
            }
        }
    }
}

/// 現在位置が記録される[`Read`]。
///
/// 現在位置は`u64`の範囲内でのみ記録され、それ以上はオーバーフローせず上限で留まる。
#[derive(Debug)]
struct PositionedRead<T> {
    inner: T,
    pos: u64,
}

impl<T> PositionedRead<T> {
    #[inline]
    pub fn new(inner: T) -> PositionedRead<T> {
        PositionedRead { inner, pos: 0 }
    }

    /// 現在位置を返す。
    #[inline]
    pub fn pos(&self) -> u64 {
        self.pos
    }
}

impl<T: Read> Read for PositionedRead<T> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let r = self.inner.read(buf);
        if let Ok(c) = r {
            self.pos = self.pos.saturating_add(c as u64);
        }
        r
    }
}

impl<T: Seek> Seek for PositionedRead<T> {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let new_pos = self.inner.seek(pos)?;
        self.pos = new_pos;
        Ok(new_pos)
    }
}

struct Limit<'a, R> {
    inner: R,
    limit: &'a mut u64,
}

impl<'a, R> Limit<'a, R> {
    #[inline]
    pub fn new(inner: R, limit: &'a mut u64) -> Limit<'a, R> {
        Limit { inner, limit }
    }
}

impl<'a, R: Read> Read for Limit<'a, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if *self.limit == 0 {
            return Ok(0);
        }

        let max = std::cmp::min(buf.len() as u64, *self.limit) as usize;
        let n = self.inner.read(&mut buf[..max])?;
        assert!(
            n as u64 <= *self.limit,
            "number of read bytes exceeds limit"
        );
        *self.limit -= n as u64;
        Ok(n)
    }
}

fn next_pcr<R: Read>(mut read: R, pcr_pid: isdb::Pid) -> io::Result<Option<Timestamp>> {
    loop {
        match isdb::Packet::read(&mut read) {
            Ok(Some(packet)) => {
                if packet.pid() == pcr_pid {
                    if let Some(pcr) = packet.adaptation_field().and_then(|af| af.pcr()) {
                        break Ok(Some(pcr));
                    }
                }
            }
            Ok(None) => break Ok(None),
            Err(e) => break Err(e),
        }
    }
}

enum WorkerState {
    Working,
    Eos,
    Error,
}

struct Worker<R: Read + Seek, T: Sink> {
    parker: crossbeam_utils::sync::Parker,
    commands: Arc<Commands>,
    state: WorkerState,

    demuxer: isdb::demux::Demuxer<isdb::filters::sorter::Sorter<Selector<R, T>>>,
    probe_size: u64,
    tail_probe_size: u64,
}

impl<R: Read + Seek, T: Sink> Worker<R, T> {
    #[inline]
    fn selector(&mut self) -> &mut Selector<R, T> {
        self.demuxer.filter_mut().shooter_mut()
    }

    /// ストリーム終端。
    #[inline]
    fn on_eos(&mut self) {
        self.state = WorkerState::Eos;
        self.selector().sink.on_end_of_stream();
    }

    /// エラー発生。
    #[inline]
    fn on_error(&mut self, e: io::Error) {
        self.state = WorkerState::Error;
        self.selector().sink.on_stream_error(e);
    }

    /// EOSやエラーから復帰した。
    #[inline]
    fn on_restored(&mut self) {
        self.state = WorkerState::Working;
    }

    fn on_rewinded(&mut self, pcr_time: PlaybackTime) {
        self.on_restored();
        self.selector().pcr_time = pcr_time;
        self.selector().state.write().tot = None;
        self.demuxer.reset_packets();
    }

    /// ストリームを確定させる。
    ///
    /// ストリームが見つからなかった場合は`false`を返す。
    fn probe_stream(&mut self) -> bool {
        let mut limit = self.probe_size;
        loop {
            match isdb::Packet::read(Limit::new(&mut self.selector().read, &mut limit)) {
                Ok(Some(packet)) => {
                    self.demuxer.feed(&packet);

                    if self.selector().state.read().selected_stream.is_some() {
                        break;
                    }
                }
                Ok(None) => return false,
                Err(e) => {
                    self.on_error(e);
                    return false;
                }
            }
        }

        let start_pos = self.selector().read.pos();

        let (pcr_pid, pcr) = {
            let service = self.demuxer.filter_mut().services().first().unwrap().1;
            (service.pcr_pid(), service.pcr())
        };

        let first_pcr = if let Some(pcr) = pcr {
            pcr
        } else {
            // 既定サービスにおける最初のPCRをprobe_sizeの範囲内で探す
            match next_pcr(Limit::new(&mut self.selector().read, &mut limit), pcr_pid) {
                Ok(Some(pcr)) => pcr,
                // 最初のPCRが見つからなくてもエラーにはしない
                Ok(None) => return true,
                Err(e) => {
                    self.on_error(e);
                    return true;
                }
            }
        };

        let Ok(len) = self.selector().read.seek(SeekFrom::End(0)) else {
            // ファイルの長さが取得できなくてもエラーにはしない
            return true;
        };

        'probe: {
            // ファイルサイズがtail_probe_sizeより小さい場合はストリームが確定した位置から読み取り続行
            let seek_pos = len.checked_sub(self.tail_probe_size).unwrap_or(start_pos);
            if self
                .selector()
                .read
                .seek(SeekFrom::Start(seek_pos))
                .is_err()
            {
                // シークできない場合は最後のPCR探しをやめる
                // 確定位置まで戻すためbreak
                break 'probe;
            }

            // 既定サービスにおける最後のPCRを探す
            if let Ok(Some(last_pcr)) = next_pcr(&mut self.selector().read, pcr_pid) {
                let length = StreamLength {
                    first_pcr,
                    last_pcr,
                    size: self.selector().read.pos() - start_pos,
                };
                log::trace!("ストリーム長：{:?}", length.duration());
                self.selector().state.write().length = Some(length);
            }
        }

        if let Err(e) = self.selector().read.seek(SeekFrom::Start(start_pos)) {
            // 確定位置まで戻れなかったのでエラー
            self.on_error(e);
            return false;
        }

        true
    }

    /// 次のパケットを処理する。
    ///
    /// `Worker`を終了する必要がある場合には`false`を返す。
    fn next_packet(&mut self) {
        match isdb::Packet::read(&mut self.selector().read) {
            Ok(Some(packet)) => self.demuxer.feed(&packet),
            Ok(None) => self.on_eos(),
            Err(e) => self.on_error(e),
        }
    }

    fn select_service(&mut self, service_id: Option<ServiceId>) {
        let sorter = self.demuxer.filter_mut();
        let (services, shooter) = sorter.pair();
        shooter.select_service(services, service_id);
    }

    fn select_video_stream(&mut self, component_tag: u8) {
        let sorter = self.demuxer.filter_mut();
        let (services, shooter) = sorter.pair();
        shooter.select_video_stream(services, component_tag);
    }

    fn select_audio_stream(&mut self, component_tag: u8) {
        let sorter = self.demuxer.filter_mut();
        let (services, shooter) = sorter.pair();
        shooter.select_audio_stream(services, component_tag);
    }

    fn set_position(&mut self, pos: Duration) {
        /// 先頭へのシークと見做す最大の位置。
        const HEAD_MAX_POS: Duration = Duration::from_secs(1);
        /// シークを完了させる際に許容する最大の時間差。
        const IDLE_MAX: Duration = Duration::from_secs(30);
        /// シークを完了させるために必要な最低限の時間差。
        /// PSIやPESを貯めるため`pos`の少し手前にシークする必要がある。
        const IDLE_MIN: Duration = Duration::from_millis(500);
        /// ファイルをシークする場合の実際の目的地との時間差。
        /// ある程度差を設けることで行き過ぎを防ぐ。
        const FILE_OFFSET: Duration = Duration::from_secs(3);
        const _: () = {
            assert!(IDLE_MIN.as_nanos() < IDLE_MAX.as_nanos());
            assert!(
                FILE_OFFSET.as_nanos() > IDLE_MIN.as_nanos()
                    && FILE_OFFSET.as_nanos() < IDLE_MAX.as_nanos(),
                "FILE_OFFSETはIDLE_MIN/MAXの範囲内でないと無意味",
            );
        };

        enum Direction {
            Forward,
            Backward,
        }

        let Some(mut length) = self.selector().state.read().length.clone() else {
            log::warn!("シークできないストリームへのシーク要求");
            return;
        };
        let Some(pcr_pid) = self.demuxer.filter().services().first().map(|(_, svc)| svc.pcr_pid())
        else {
            log::debug!("サービスがない");
            return;
        };

        let target_pos = pos.saturating_sub(IDLE_MIN);

        // パケットを読み飛ばすのに必要な情報を設定
        let orig_stream = self.selector().state.read().selected_stream.clone();
        self.selector().seek_info = Some(SeekInfo {
            target_pos,
            orig_stream,
            pat_updated: false,
            pmt_updated: SortedSet::new(),
            eit_updated: SortedSet::new(),
            last_captions: RingBuf::new(),
        });

        if matches!(self.state, WorkerState::Eos) {
            // TOTが来ないままEOSになった後のシークでズレるので巻き戻しておく
            if !self.rewind() {
                return;
            }
        }

        let start_pos = self.selector().read.pos();
        let start_ts = self.selector().pcr_time.prev_ts.unwrap_or(length.first_pcr);

        let current_pos = self.selector().pcr_time.duration;
        let (mut diff, mut dir, target_pcr_min, target_pcr_max) = if target_pos >= current_pos {
            let diff = target_pos - current_pos;
            if diff <= IDLE_MAX {
                // 近めへの早送りは読み飛ばすだけ
                return;
            }

            (
                diff,
                Direction::Forward,
                // diffはIDLE_MAXより大きいためアンダーフローはしない
                start_ts + (diff - IDLE_MAX),
                start_ts + diff,
            )
        } else {
            if target_pos <= HEAD_MAX_POS {
                // 先頭へのシークは常に行き過ぎ判定に入るため確定で頭出し
                self.rewind();
                return;
            }

            let diff = current_pos - target_pos;
            (
                diff,
                Direction::Backward,
                start_ts - (diff + IDLE_MAX),
                start_ts - diff,
            )
        };

        // 無限ループにならないよう最大でも6回試行する
        for _ in 0..6 {
            let seek_pos = if let Direction::Forward = dir {
                SeekFrom::Current(length.estimate_size(diff - FILE_OFFSET) as i64)
            } else {
                let backward = length.estimate_size(diff + FILE_OFFSET);

                if self.selector().read.pos() >= backward {
                    SeekFrom::Current(-(backward as i64))
                } else {
                    SeekFrom::Start(0)
                }
            };
            if let Err(e) = self.selector().read.seek(seek_pos) {
                self.on_error(e);
                return;
            }

            match next_pcr(&mut self.selector().read, pcr_pid) {
                Ok(Some(pcr)) => {
                    if pcr < target_pcr_min {
                        diff = (target_pcr_max - pcr).to_duration();
                        dir = Direction::Forward;
                        log::trace!("シーク：足りない（{:?}の{:?}前）", target_pos, diff);
                    } else if pcr > target_pcr_max {
                        diff = (pcr - target_pcr_max).to_duration();
                        dir = Direction::Backward;
                        log::trace!("シーク：行き過ぎ（{:?}の{:?}後）", target_pos, diff);
                    } else {
                        let diff = (target_pcr_max - pcr).to_duration();
                        log::trace!("シーク：確定（{:?}の{:?}前）", target_pos, diff);
                        self.on_rewinded(PlaybackTime {
                            prev_ts: Some(pcr),
                            // 同じ位置へのシークを繰り返すと約-75ナノ秒ずつズレていくっぽいが、
                            // この程度なら誤差としたい（complete_seekで観測できる）
                            duration: target_pos.saturating_sub(diff),
                        });

                        return;
                    }

                    // 精度を高めるためにストリーム長を短い範囲のものに置き換え
                    let new_pos = self.selector().read.pos();
                    length = if new_pos > start_pos {
                        StreamLength {
                            first_pcr: start_ts,
                            last_pcr: pcr,
                            size: new_pos - start_pos,
                        }
                    } else {
                        StreamLength {
                            first_pcr: pcr,
                            last_pcr: start_ts,
                            size: start_pos - new_pos,
                        }
                    };
                    continue;
                }
                Ok(None) => {
                    self.on_eos();
                    return;
                }
                Err(e) => {
                    self.on_error(e);
                    return;
                }
            }
        }

        // TODO: 二分探索？
        if let Direction::Forward = dir {
            // 早送りでは飛ばすだけ
            log::info!("ビットレートによるシークに失敗。読み飛ばして再検索");
            self.on_restored();
        } else {
            // 巻き戻しでは先頭から探す
            log::info!("ビットレートによるシークに失敗。頭出しにより再検索");
            self.rewind();
        }
    }

    /// TSの頭出しをする。
    ///
    /// シーク情報は設定されないため呼び出し側で適宜設定すること。
    fn rewind(&mut self) -> bool {
        match self.selector().read.rewind() {
            Ok(_) => {
                self.on_rewinded(PlaybackTime::default());
                true
            }
            Err(e) => {
                // TODO: リアルタイム視聴中はエラーではない
                self.on_error(e);
                false
            }
        }
    }

    fn reset(&mut self) {
        if self.rewind() {
            let orig_stream = self.selector().state.read().selected_stream.clone();
            self.selector().seek_info = Some(SeekInfo {
                target_pos: Duration::ZERO,
                orig_stream,
                pat_updated: false,
                pmt_updated: SortedSet::new(),
                eit_updated: SortedSet::new(),
                last_captions: RingBuf::new(),
            });
        }
    }

    /// コマンドを実行する。
    fn run_commands(&mut self) {
        // コマンドの実行の際、TSの読み取りは伴わないため処理順序に意味はない。
        // また`has_any`の変更より各コマンド用フィールドの変更が後になるが、コマンドは上書きされても構わず、
        // かつ後から`has_any`だけが`true`になっても`run_commands`が空振りするだけなので問題はない。
        let select_service = self.commands.select_service.swap(0, Ordering::SeqCst);
        if select_service > 0 {
            self.select_service(ServiceId::new((select_service - 1) as u16));
        }

        let select_video_stream = self.commands.select_video_stream.swap(0, Ordering::SeqCst);
        if select_video_stream > 0 {
            self.select_video_stream((select_video_stream - 1) as u8);
        }

        let select_audio_stream = self.commands.select_audio_stream.swap(0, Ordering::SeqCst);
        if select_audio_stream > 0 {
            self.select_audio_stream((select_audio_stream - 1) as u8);
        }

        let set_position_secs = self.commands.set_position_secs.swap(0, Ordering::SeqCst);
        if set_position_secs > 0 {
            let set_position_nanos = self.commands.set_position_nanos.load(Ordering::SeqCst);
            let pos = Duration::new(set_position_secs - 1, set_position_nanos);
            self.set_position(pos);
        }

        let reset = self.commands.reset.swap(false, Ordering::SeqCst);
        if reset {
            self.reset();
        }
    }

    pub fn run(mut self) {
        if !self.probe_stream() {
            self.selector()
                .sink
                .on_stream_error(io::Error::new(io::ErrorKind::InvalidData, StreamProbeError));
            return;
        }
        log::trace!("ストリーム確定");

        loop {
            let has_any_command = self.commands.has_any.swap(false, Ordering::SeqCst);
            // エラーがある場合はパケットを処理しない
            let needs_es =
                !matches!(self.state, WorkerState::Error) && self.selector().sink.needs_es();

            if has_any_command {
                if self.commands.shutdown.load(Ordering::SeqCst) {
                    break;
                }

                self.run_commands();
            }
            if needs_es {
                if matches!(self.state, WorkerState::Working) {
                    self.next_packet();
                } else {
                    // EOS後はパケットを読み取らず、EOSを通知するだけ
                    self.selector().sink.on_end_of_stream();
                }
            }

            if !has_any_command && !needs_es {
                self.parker.park();
            }
        }
    }
}

impl<R: Read + Seek, T: Sink> Drop for Worker<R, T> {
    fn drop(&mut self) {
        self.commands.shutdown.store(true, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_playback_time() {
        const INIT_DUR: Duration = Duration::from_secs(100);
        const INIT_PT: PlaybackTime = PlaybackTime {
            prev_ts: None,
            duration: INIT_DUR,
        };

        // 普通のパターン
        let mut pt = INIT_PT;
        pt.update(Duration::from_millis(100).into());
        assert_eq!(pt.duration - INIT_DUR, Duration::ZERO);
        pt.update(Duration::from_millis(200).into());
        assert_eq!(pt.duration - INIT_DUR, Duration::from_millis(100));

        // ラップアラウンド
        let mut pt = INIT_PT;
        pt.update(Timestamp::new(8589906560, 0));
        assert_eq!(pt.duration - INIT_DUR, Duration::ZERO);
        pt.update(Timestamp::new(7473, 0));
        assert_eq!(pt.duration - INIT_DUR, Duration::from_secs_f64(0.3945));

        // 遡るパターン
        let mut pt = INIT_PT;
        pt.update(Duration::from_millis(200).into());
        assert_eq!(INIT_DUR - pt.duration, Duration::ZERO);
        pt.update(Duration::from_millis(100).into());
        assert_eq!(INIT_DUR - pt.duration, Duration::from_millis(100));

        // ラップアラウンドで遡るパターン
        let mut pt = INIT_PT;
        pt.update(Timestamp::new(1467, 0));
        assert_eq!(INIT_DUR - pt.duration, Duration::ZERO);
        pt.update(Timestamp::new(8589910400, 0));
        assert_eq!(INIT_DUR - pt.duration, Duration::from_secs_f64(0.2851));
    }

    #[test]
    fn test_stream_length() {
        assert_eq!(
            StreamLength {
                first_pcr: Timestamp::ZERO,
                last_pcr: Duration::from_secs(1).into(),
                size: 100,
            }
            .duration(),
            Duration::from_secs(1)
        );

        assert_eq!(
            StreamLength {
                first_pcr: Timestamp::ZERO,
                last_pcr: Duration::from_secs(1).into(),
                size: 100
            }
            .estimate_size(Duration::from_secs(2)),
            200
        );
        assert_eq!(
            StreamLength {
                first_pcr: Timestamp::ZERO,
                last_pcr: Duration::from_secs(2).into(),
                size: 100
            }
            .estimate_size(Duration::from_secs(1)),
            50
        );
        assert_eq!(
            StreamLength {
                first_pcr: Timestamp::ZERO,
                last_pcr: Duration::from_secs(3).into(),
                size: 100
            }
            .estimate_size(Duration::from_secs(1)),
            33
        );
    }
}
