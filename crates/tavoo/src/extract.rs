use std::io::{self, Read, Seek};
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;

use fxhash::FxHashSet;
use isdb::filters::sorter::{Service, ServiceMap, Stream};
use isdb::psi::table::ServiceId;
use isdb::time::Timestamp;
use parking_lot::RwLock;

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
    fn on_stream_changed(&mut self, changed: StreamChanged);

    /// 選択中サービスで映像パケットを受信した際に呼ばれる。
    fn on_video_packet(&mut self, pts: Option<Timestamp>, dts: Option<Timestamp>, payload: &[u8]);

    /// 選択中サービスで音声パケットを受信した際に呼ばれる。
    fn on_audio_packet(&mut self, pts: Option<Timestamp>, dts: Option<Timestamp>, payload: &[u8]);

    /// 選択中サービスで字幕パケットを受信した際に呼ばれる。
    fn on_caption(&mut self, caption: &isdb::filters::sorter::Caption);

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
    /// 再生位置を設定する。
    // 0は指示無し、1～は`Some(Timestamp)`
    set_position: AtomicU64,
    /// TSをリセットして最初から再生する。
    reset: AtomicBool,
    /// TSの読み取りを終了する。
    shutdown: AtomicBool,
}

/// 処理中のTSにおける状態。
#[derive(Debug, Default)]
struct State {
    services: ServiceMap,
    selected_stream: Option<SelectedStream>,
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

    /// 指定された読み取り元`Read`と処理用`Sink`を使い、新しいスレッドで`Extractor`の処理を開始する。
    ///
    /// 戻り値の[`JoinHandle`][std::thread::JoinHandle]を使って終了待ちができるが、
    /// スレッドを終了させるためには事前に[`ExtractHandler::shutdown`]を呼び出す必要がある。
    pub fn spawn<R, T>(self, read: R, sink: T) -> std::thread::JoinHandle<()>
    where
        R: Read + Seek + Send + 'static,
        T: Sink + Send + 'static,
    {
        let read = std::io::BufReader::with_capacity(self.capacity, read);
        let demuxer = isdb::demux::Demuxer::new(isdb::filters::sorter::Sorter::new(Selector::new(
            sink, self.state,
        )));

        let worker = Worker {
            parker: self.parker,
            commands: self.commands,

            read,
            demuxer,
            probe_size: self.probe_size,
        };
        std::thread::spawn(move || worker.run())
    }
}

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
    /// 現在のサービス一覧を返す。
    ///
    /// 戻り値はロックを保持しているため、できるだけ速く破棄すべきである。
    pub fn services(&self) -> parking_lot::MappedRwLockReadGuard<ServiceMap> {
        parking_lot::RwLockReadGuard::map(self.state.read(), |s| &s.services)
    }

    /// 選択中のサービス・ストリームに関する情報を返す。
    ///
    /// 戻り値はロックを保持しているため、できるだけ速く破棄すべきである。
    pub fn selected_stream(&self) -> parking_lot::MappedRwLockReadGuard<Option<SelectedStream>> {
        parking_lot::RwLockReadGuard::map(self.state.read(), |s| &s.selected_stream)
    }

    /// ESを要求する。
    ///
    /// このメソッドを呼び出した際、[`Sink::needs_es`]は`true`を返すべきである。
    pub fn request_es(&self) {
        self.unparker.unpark();
    }

    /// サービス選択を指示する。
    ///
    /// `service_id`に`None`を指定した場合、既定のサービスが選択される。
    pub fn select_service(&self, service_id: Option<ServiceId>) {
        let service_id = service_id.map_or(0, |id| id.get());
        self.commands
            .select_service
            .store(service_id as u32 + 1, Ordering::SeqCst);
        self.commands.has_any.store(true, Ordering::SeqCst);
        self.unparker.unpark();
    }

    /// 映像ストリームの選択を指示する。
    pub fn select_video_stream(&self, component_tag: u8) {
        self.commands
            .select_video_stream
            .store(component_tag as u16 + 1, Ordering::SeqCst);
        self.commands.has_any.store(true, Ordering::SeqCst);
        self.unparker.unpark();
    }

    /// 音声ストリームの選択を指示する。
    pub fn select_audio_stream(&self, component_tag: u8) {
        self.commands
            .select_audio_stream
            .store(component_tag as u16 + 1, Ordering::SeqCst);
        self.commands.has_any.store(true, Ordering::SeqCst);
        self.unparker.unpark();
    }

    /// 再生位置の設定を指示する。
    pub fn set_position(&self, pos: Timestamp) {
        self.commands
            .set_position
            .store(pos.0 + 1, Ordering::SeqCst);
        self.commands.has_any.store(true, Ordering::SeqCst);
        self.unparker.unpark();
    }

    /// TSをリセットし最初から再生しなおすことを指示する。
    pub fn reset(&self) {
        self.commands.reset.store(true, Ordering::SeqCst);
        self.commands.has_any.store(true, Ordering::SeqCst);
        self.unparker.unpark();
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
    pub caption_pids: FxHashSet<isdb::Pid>,
}

#[derive(Debug)]
struct SeekInfo {
    /// シーク先の位置。
    target_pos: Timestamp,

    /// シーク開始時に選択されていたストリーム。
    orig_stream: Option<SelectedStream>,

    /// 保留する各種イベント。
    pat_updated: bool,
    eit_updated: SortedSet<(ServiceId, bool)>,
    pmt_updated: SortedSet<ServiceId>,
}

#[derive(Debug)]
struct Selector<T> {
    sink: T,

    state: Arc<RwLock<State>>,
    /// 最後に処理したESの時間。
    cur_pos: Timestamp,
    /// シーク中の情報。シークが完了したら`None`が設定される。
    seek_info: Option<SeekInfo>,
}

impl<T: Sink> Selector<T> {
    #[inline]
    fn new(sink: T, state: Arc<RwLock<State>>) -> Selector<T> {
        Selector {
            sink,

            state,
            cur_pos: Timestamp(0),
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
                caption_pids: service.caption_streams().iter().map(|s| s.pid()).collect(),
            });

            changed
        };

        // シーク中はイベント発生を保留
        if self.seek_info.is_none() {
            self.sink.on_service_changed(service);
            if changed.any() {
                self.sink.on_stream_changed(changed);
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
            self.sink.on_stream_changed(changed);
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
            self.sink.on_stream_changed(changed);
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
            selected_stream.caption_pids =
                service.caption_streams().iter().map(|s| s.pid()).collect();

            changed
        };
        // シーク中はイベント発生を保留
        if self.seek_info.is_none() && changed.any() {
            self.sink.on_stream_changed(changed);
        }
    }

    /// シーク中、`pts`がシーク対象以降であればシークを完了させる。
    ///
    /// シーク中でない、またはシークが完了できた場合は`true`を、
    /// まだシークが完了できない場合は`false`を返す。
    ///
    /// また、シークが完了した場合には保留していたイベントを発生させる。
    fn complete_seek(&mut self, pts: Option<Timestamp>) -> bool {
        let seek_info = match (&mut self.seek_info, pts) {
            (None, _) => return true,

            // target_posまでは何も処理しない
            (Some(seek_info), Some(pts)) if pts >= seek_info.target_pos => seek_info,
            _ => return false,
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
                self.sink.on_stream_changed(changed);
            }
        }

        self.seek_info = None;
        true
    }
}

impl<T: Sink> isdb::filters::sorter::Shooter for Selector<T> {
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
        _: &ServiceMap,
        pid: isdb::Pid,
        pts: Option<Timestamp>,
        dts: Option<Timestamp>,
        payload: &[u8],
    ) {
        // シーク中にシーク指示があった場合に基準がずれないようcur_posは常に更新
        if let Some(pts) = pts {
            self.cur_pos = pts;
        }
        if !self.complete_seek(pts) {
            return;
        }

        {
            let state = self.state.read();
            if !matches!(&state.selected_stream, Some(ss) if ss.video_stream.pid() == pid) {
                return;
            }
        }
        self.sink.on_video_packet(pts, dts, payload);
    }

    fn on_audio_packet(
        &mut self,
        _: &ServiceMap,
        pid: isdb::Pid,
        pts: Option<Timestamp>,
        dts: Option<Timestamp>,
        payload: &[u8],
    ) {
        // シーク中にシーク指示があった場合に基準がずれないようcur_posは常に更新
        if let Some(pts) = pts {
            self.cur_pos = pts;
        }
        if !self.complete_seek(pts) {
            return;
        }

        {
            let state = self.state.read();
            if !matches!(&state.selected_stream, Some(ss) if ss.audio_stream.pid() == pid) {
                return;
            }
        }
        self.sink.on_audio_packet(pts, dts, payload);
    }

    fn on_caption(
        &mut self,
        _: &ServiceMap,
        pid: isdb::Pid,
        caption: &isdb::filters::sorter::Caption,
    ) {
        // TODO: シーク完了時点に表示されているであろう字幕はスキップしたくない
        if self.seek_info.is_some() {
            return;
        }

        {
            let state = self.state.read();
            if !matches!(&state.selected_stream, Some(s) if s.caption_pids.contains(&pid)) {
                return;
            }
        }

        self.sink.on_caption(caption);
    }
}

struct Worker<R, T: Sink> {
    parker: crossbeam_utils::sync::Parker,
    commands: Arc<Commands>,

    read: io::BufReader<R>,
    demuxer: isdb::demux::Demuxer<isdb::filters::sorter::Sorter<Selector<T>>>,
    probe_size: u64,
}

impl<R: Read + Seek, T: Sink> Worker<R, T> {
    #[inline]
    fn selector(&mut self) -> &mut Selector<T> {
        self.demuxer.filter_mut().shooter_mut()
    }

    /// ストリームを確定させる。
    ///
    /// ストリームが見つからなかった場合は`false`を返す。
    fn probe_stream(&mut self) -> bool {
        let mut probe_file = self.read.by_ref().take(self.probe_size);
        loop {
            match isdb::Packet::read(&mut probe_file) {
                Ok(Some(packet)) => {
                    self.demuxer.feed(&packet);

                    let selector = self.demuxer.filter_mut().shooter_mut();
                    if selector.state.read().selected_stream.is_some() {
                        break true;
                    }
                }
                Ok(None) => {
                    break false;
                }
                Err(e) => {
                    self.selector().sink.on_stream_error(e);
                    break false;
                }
            }
        }
    }

    /// 次のパケットを処理する。
    ///
    /// `Worker`を終了する必要がある場合には`false`を返す。
    fn next_packet(&mut self) -> bool {
        match isdb::Packet::read(&mut self.read) {
            Ok(Some(packet)) => {
                self.demuxer.feed(&packet);
                true
            }
            Ok(None) => {
                self.selector().sink.on_end_of_stream();
                false
            }
            Err(e) => {
                self.selector().sink.on_stream_error(e);
                false
            }
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

    fn set_position(&mut self, pos: Timestamp) -> bool {
        // TODO: シーク位置をキャッシュして高速化

        if self.selector().cur_pos > pos {
            if !self.reset() {
                return false;
            }
        }

        let orig_stream = self.selector().state.read().selected_stream.clone();
        self.selector().seek_info = Some(SeekInfo {
            target_pos: pos,
            orig_stream,
            pat_updated: false,
            pmt_updated: SortedSet::new(),
            eit_updated: SortedSet::new(),
        });
        true
    }

    fn reset(&mut self) -> bool {
        match self.read.rewind() {
            Ok(_) => {
                self.demuxer.reset_packets();
                true
            }
            Err(e) => {
                // TODO: リアルタイム視聴中はエラーではない
                self.selector().sink.on_stream_error(e);
                false
            }
        }
    }

    /// コマンドを実行する。
    ///
    /// `Worker`を終了する必要がある場合には`false`を返す。
    fn run_commands(&mut self) -> bool {
        // コマンドの実行の際、TSの読み取りは伴わないため処理順序に意味はない。
        // また`has_any`の変更より各コマンド用フィールドの変更が後になるが、コマンドは上書きされても構わず、
        // かつ後から`has_any`だけが`true`になっても`run_commands`が空振りするだけなので問題はない。
        if self.commands.shutdown.load(Ordering::SeqCst) {
            self.selector().sink.on_end_of_stream();
            return false;
        }

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

        let set_position = self.commands.set_position.swap(0, Ordering::SeqCst);
        if set_position > 0 {
            if !self.set_position(Timestamp(set_position - 1)) {
                return false;
            }
        }

        let reset = self.commands.reset.swap(false, Ordering::SeqCst);
        if reset {
            if !self.reset() {
                return false;
            }
        }

        true
    }

    pub fn run(mut self) {
        if !self.probe_stream() {
            return;
        }
        log::trace!("ストリーム確定");

        loop {
            let has_any_command = self.commands.has_any.swap(false, Ordering::SeqCst);
            let needs_es = self.selector().sink.needs_es();

            if has_any_command {
                if !self.run_commands() {
                    break;
                }
            }
            if needs_es {
                if !self.next_packet() {
                    break;
                }
            }

            if !has_any_command && !needs_es {
                self.parker.park();
            }
        }
    }
}
