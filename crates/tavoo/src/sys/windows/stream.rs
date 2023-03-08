use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::io::Seek;
use std::path::Path;
use std::sync::Arc;

use fxhash::{FxHashMap, FxHashSet};
use isdb::psi::table::ServiceId;
use windows::core::{self as C, implement, AsImpl, Interface};
use windows::Win32::Foundation as F;
use windows::Win32::Media::KernelStreaming::GUID_NULL;
use windows::Win32::Media::MediaFoundation as MF;

use super::utils::{get_stream_descriptor_by_index, PropVariant, WinResult};

fn io_to_hresult(e: std::io::Error) -> C::HRESULT {
    e.raw_os_error()
        .map(|e| {
            log::error!("IO error: {}", e);
            F::WIN32_ERROR(e as u32).to_hresult()
        })
        .unwrap_or(F::E_FAIL)
}

#[derive(Debug, Clone)]
struct ESPacket {
    pid: isdb::Pid,
    pts: Option<isdb::time::Timestamp>,
    payload: Vec<u8>,
}

#[derive(Debug, Clone)]
enum PlaybackEvent {
    Pat,
    Pmt(ServiceId),
    Eit(ServiceId),
    Video(ESPacket),
    Audio(ESPacket),
    Caption(isdb::Pid, Vec<isdb::AribString>),
}

#[derive(Debug, Default, Clone)]
struct PlaybackEvents {
    // TODO: ESPacketの領域は再利用できるようにしたい
    events: VecDeque<PlaybackEvent>,
}

impl isdb::filters::sorter::Shooter for PlaybackEvents {
    fn on_pat_updated(&mut self) {
        self.events.push_back(PlaybackEvent::Pat);
    }

    fn on_pmt_updated(&mut self, service_id: ServiceId) {
        self.events.push_back(PlaybackEvent::Pmt(service_id));
    }

    fn on_eit_updated(&mut self, service_id: ServiceId, is_present: bool) {
        if is_present {
            self.events.push_back(PlaybackEvent::Eit(service_id));
        }
    }

    fn on_video_packet(
        &mut self,
        pid: isdb::Pid,
        pts: Option<isdb::time::Timestamp>,
        _dts: Option<isdb::time::Timestamp>,
        payload: &[u8],
    ) {
        self.events.push_back(PlaybackEvent::Video(ESPacket {
            pid,
            pts,
            payload: payload.to_vec(),
        }))
    }

    fn on_audio_packet(
        &mut self,
        pid: isdb::Pid,
        pts: Option<isdb::time::Timestamp>,
        _dts: Option<isdb::time::Timestamp>,
        payload: &[u8],
    ) {
        self.events.push_back(PlaybackEvent::Audio(ESPacket {
            pid,
            pts,
            payload: payload.to_vec(),
        }))
    }

    fn on_caption(&mut self, pid: isdb::Pid, caption: &isdb::filters::sorter::Caption) {
        let captions = caption
            .data_units()
            .iter()
            .filter_map(|unit| match *unit {
                isdb::pes::caption::DataUnit::StatementBody(caption) => Some(caption.to_owned()),
                _ => None,
            })
            .collect();
        self.events.push_back(PlaybackEvent::Caption(pid, captions));
    }
}

#[implement(MF::IMFAsyncCallback)]
pub struct AsyncQueue {
    queue: parking_lot::Mutex<VecDeque<Box<dyn FnOnce()>>>,
}

impl AsyncQueue {
    pub fn new() -> MF::IMFAsyncCallback {
        AsyncQueue {
            queue: parking_lot::Mutex::new(VecDeque::new()),
        }
        .into()
    }

    fn intf(&self) -> MF::IMFAsyncCallback {
        unsafe { self.cast().unwrap() }
    }

    pub fn process_queue(&self) -> WinResult<()> {
        unsafe {
            if !self.queue.lock().is_empty() {
                MF::MFPutWorkItem(MF::MFASYNC_CALLBACK_QUEUE_STANDARD, &self.intf(), None)?;
            }
            Ok(())
        }
    }

    pub fn enqueue<F: FnOnce() + 'static>(&self, f: F) -> WinResult<()> {
        self.queue.lock().push_back(Box::new(f));
        self.process_queue()?;
        Ok(())
    }
}

#[allow(non_snake_case)]
impl MF::IMFAsyncCallback_Impl for AsyncQueue {
    fn GetParameters(&self, _: *mut u32, _: *mut u32) -> WinResult<()> {
        Err(F::E_NOTIMPL.into())
    }

    fn Invoke(&self, _: &Option<MF::IMFAsyncResult>) -> WinResult<()> {
        let f = self.queue.lock().pop_front();
        if let Some(f) = f {
            f();
        }

        Ok(())
    }
}

const SAMPLE_QUEUE: usize = 2;
// PidだとGetStreamIdentifierとかで範囲チェックが入って都合が悪いのでu16そのまま扱う
type Pid16 = u16;

struct SelectedStream {
    service_id: ServiceId,
    is_oneseg: bool,

    video_stream: isdb::filters::sorter::Stream,
    audio_stream: isdb::filters::sorter::Stream,
    caption_pids: FxHashSet<isdb::Pid>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceState {
    Stopped,
    Paused,
    Started,
    Shutdown,
}

#[implement(
    MF::IMFGetService,
    MF::IMFMediaSource,
    MF::IMFRateControl,
    MF::IMFRateSupport
)]
pub struct TransportStream {
    mutex: Arc<parking_lot::ReentrantMutex<()>>,

    queue: MF::IMFAsyncCallback,
    state: Cell<SourceState>,

    file: RefCell<std::io::BufReader<std::fs::File>>,
    demuxer: RefCell<isdb::demux::Demuxer<isdb::filters::sorter::Sorter<PlaybackEvents>>>,
    selected_stream: RefCell<SelectedStream>,

    event_queue: MF::IMFMediaEventQueue,
    presentation_descriptor: RefCell<MF::IMFPresentationDescriptor>,
    streams: RefCell<FxHashMap<Pid16, MF::IMFMediaStream>>,

    rate: Cell<f32>,
    first_pts: Cell<Option<isdb::time::Timestamp>>,
    pending_eos: Cell<usize>,
    is_switching: Cell<bool>,
}

impl TransportStream {
    pub fn new<P: AsRef<Path>>(path: P) -> WinResult<MF::IMFMediaSource> {
        use std::io::Read;
        let file = std::fs::File::open(path).map_err(io_to_hresult)?;
        let file = std::io::BufReader::with_capacity(188 * 32, file);

        let player = PlaybackEvents::default();
        let demuxer = isdb::demux::Demuxer::new(isdb::filters::sorter::Sorter::new(player));

        unsafe {
            let event_queue = MF::MFCreateEventQueue()?;
            let dummy_pd = MF::MFCreatePresentationDescriptor(None)?;
            let source: MF::IMFMediaSource = TransportStream {
                mutex: Arc::new(parking_lot::ReentrantMutex::new(())),

                queue: AsyncQueue::new(),
                state: Cell::new(SourceState::Stopped),

                file: RefCell::new(file),
                demuxer: RefCell::new(demuxer),
                selected_stream: RefCell::new(SelectedStream {
                    service_id: ServiceId::new(1).unwrap(),
                    is_oneseg: false,
                    video_stream: isdb::filters::sorter::Stream::invalid(),
                    audio_stream: isdb::filters::sorter::Stream::invalid(),
                    caption_pids: FxHashSet::default(),
                }),

                event_queue,
                presentation_descriptor: RefCell::new(dummy_pd),
                streams: RefCell::new(FxHashMap::default()),

                rate: Cell::new(1.),
                first_pts: Cell::new(None),
                pending_eos: Cell::new(0),
                is_switching: Cell::new(false),
            }
            .into();
            let this = source.as_impl();

            'done: {
                let mut file = this.file.borrow_mut();
                let mut demuxer = this.demuxer.borrow_mut();

                // ストリーム情報を初期化するために最大4096パケット読み込む
                let mut probe_file = file.by_ref().take(188 * 4096);
                loop {
                    let Some(packet) = isdb::Packet::read(&mut probe_file).map_err(io_to_hresult)?
                    else {
                        log::warn!("動画の情報が定まらなかった");
                        return Err(F::E_FAIL.into());
                    };
                    demuxer.feed(&packet);

                    let filter = demuxer.filter_mut();
                    while let Some(event) = filter.shooter_mut().events.pop_front() {
                        if matches!(event, PlaybackEvent::Pat | PlaybackEvent::Pmt(_)) {
                            let Some((_, service)) = filter.services().first() else {
                                continue;
                            };
                            let Some(video_stream) = service.find_video_stream(None) else {
                                continue;
                            };
                            let Some(audio_stream) = service.find_audio_stream(None) else {
                                continue;
                            };

                            *this.selected_stream.borrow_mut() = SelectedStream {
                                service_id: service.service_id(),
                                is_oneseg: service.is_oneseg(),

                                video_stream: video_stream.clone(),
                                audio_stream: audio_stream.clone(),
                                caption_pids: service
                                    .caption_streams()
                                    .iter()
                                    .map(|s| s.pid())
                                    .collect(),
                            };
                            break 'done;
                        }
                    }
                }
            }

            this.update_streams()?;

            Ok(source)
        }
    }

    fn intf(&self) -> MF::IMFMediaSource {
        unsafe { self.cast().unwrap() }
    }

    fn queue(&self) -> &AsyncQueue {
        self.queue.as_impl()
    }

    fn update_streams(&self) -> WinResult<()> {
        unsafe {
            let selected_stream = self.selected_stream.borrow();

            let mut old_streams = std::mem::take(&mut *self.streams.borrow_mut());
            let mut new_streams = FxHashMap::default();
            let mut descriptors = Vec::new();
            let mut selections = Vec::new();

            let demuxer = self.demuxer.borrow();
            for service in demuxer.filter().services().values() {
                for strm in service.video_streams() {
                    if new_streams.contains_key(&strm.pid().get()) {
                        continue;
                    }

                    let vef = strm
                        .video_encode_format()
                        .unwrap_or_else(|| isdb::psi::desc::VideoEncodeFormat::from(0b0001));

                    let stream = if let Some(stream) = old_streams.remove(&strm.pid().get()) {
                        let es: &ElementaryStream = stream.as_impl();
                        es.update_video_stream_type(strm.stream_type(), strm.pid(), vef)?;

                        stream
                    } else {
                        ElementaryStream::video(self, strm.stream_type(), strm.pid(), vef)?
                    };

                    if strm.pid() == selected_stream.video_stream.pid() {
                        selections.push(descriptors.len());
                    }

                    descriptors.push(stream.as_impl().stream_descriptor.borrow().clone());
                    new_streams.insert(strm.pid().get(), stream);
                    log::debug!(
                        "service=0x{:04X}, pid={:?}, stream_type={:?}",
                        service.service_id(),
                        strm.pid(),
                        strm.stream_type(),
                    );
                }
                for strm in service.audio_streams() {
                    if new_streams.contains_key(&strm.pid().get()) {
                        continue;
                    }

                    let stream = if let Some(stream) = old_streams.remove(&strm.pid().get()) {
                        let es: &ElementaryStream = stream.as_impl();
                        es.update_audio_stream_type(strm.stream_type(), strm.pid())?;

                        stream
                    } else {
                        ElementaryStream::audio(self, strm.stream_type(), strm.pid())?
                    };

                    if strm.pid() == selected_stream.audio_stream.pid() {
                        selections.push(descriptors.len());
                    }

                    descriptors.push(stream.as_impl().stream_descriptor.borrow().clone());
                    new_streams.insert(strm.pid().get(), stream);
                    log::debug!(
                        "service=0x{:04X}, pid={:?}, stream_type={:?}",
                        service.service_id(),
                        strm.pid(),
                        strm.stream_type(),
                    );
                }
            }

            let pd = MF::MFCreatePresentationDescriptor(Some(&*descriptors))?;
            for &sel in &*selections {
                pd.SelectStream(sel as u32)?;
            }

            *self.presentation_descriptor.borrow_mut() = pd.clone();
            *self.streams.borrow_mut() = new_streams;

            let mut changed = false;
            for lost_stream in old_streams.values() {
                let es: &ElementaryStream = lost_stream.as_impl();
                if es.is_active() {
                    log::trace!("MEEndOfStream: {:p}", es);
                    es.event_queue.QueueEventParamVar(
                        MF::MEEndOfStream.0 as u32,
                        &GUID_NULL,
                        F::S_OK,
                        std::ptr::null(),
                    )?;

                    changed = true;
                }
            }

            if changed && !self.is_switching.get() {
                self.event_queue.QueueEventParamUnk(
                    MF::MENewPresentation.0 as u32,
                    &GUID_NULL,
                    F::S_OK,
                    &pd,
                )?;
                self.event_queue.QueueEventParamVar(
                    MF::MEEndOfPresentationSegment.0 as u32,
                    &GUID_NULL,
                    F::S_OK,
                    std::ptr::null(),
                )?;
                self.is_switching.set(true);
            }

            Ok(())
        }
    }

    fn update_service(&self) -> WinResult<()> {
        unsafe {
            let _lock = self.mutex.lock();

            if !self
                .demuxer
                .borrow()
                .filter()
                .services()
                .contains_key(&self.selected_stream.borrow().service_id)
            {
                // 選択中のサービスがなくなったので再選択
                if !self.select_service_internal(None) {
                    log::error!("選択すべきサービスがない");
                    return Ok(());
                }
            }

            let selected_stream = self.selected_stream.borrow();
            let video_pid16 = selected_stream.video_stream.pid().get();
            let audio_pid16 = selected_stream.audio_stream.pid().get();

            let pd = self.presentation_descriptor.borrow().Clone()?;
            let streams = self.streams.borrow();
            let mut changed = false;
            for i in 0..streams.len() as u32 {
                let (_, sd) = get_stream_descriptor_by_index(&pd, i)?;
                let pid16 = sd.GetStreamIdentifier()? as Pid16;
                let stream = streams.get(&pid16).ok_or(F::E_INVALIDARG)?;
                let es = stream.as_impl();

                if pid16 == video_pid16 || pid16 == audio_pid16 {
                    // 選択中のサービスにおける映像・音声の記述子を選択する
                    pd.SelectStream(i)?;

                    if !es.is_active() {
                        changed = true;
                    }
                } else {
                    pd.DeselectStream(i)?;

                    if es.is_active() {
                        changed = true;

                        log::trace!("MEEndOfStream {:p}", es);
                        es.event_queue.QueueEventParamVar(
                            MF::MEEndOfStream.0 as u32,
                            &GUID_NULL,
                            F::S_OK,
                            std::ptr::null(),
                        )?;
                    }
                }
            }

            // ストリーム切り替え中はサービス変更を通知しない
            if changed && !self.is_switching.get() {
                let demuxer = self.demuxer.borrow();
                let service_name = demuxer
                    .filter()
                    .services()
                    .get(&selected_stream.service_id)
                    .unwrap()
                    .service_name();
                log::info!(
                    "service changed: {}",
                    service_name.display(Default::default())
                );

                self.event_queue.QueueEventParamUnk(
                    MF::MENewPresentation.0 as u32,
                    &GUID_NULL,
                    F::S_OK,
                    &pd,
                )?;
                self.event_queue.QueueEventParamVar(
                    MF::MEEndOfPresentationSegment.0 as u32,
                    &GUID_NULL,
                    F::S_OK,
                    std::ptr::null(),
                )?;
                self.is_switching.set(true);
            }

            Ok(())
        }
    }

    fn select_service_internal(&self, service_id: Option<ServiceId>) -> bool {
        let demuxer = self.demuxer.borrow();
        let filter = demuxer.filter();
        let mut selected_stream = self.selected_stream.borrow_mut();

        let service = if let Some(service_id) = service_id {
            let Some(service) = filter.services().get(&service_id) else {
                log::error!("サービスが存在しない");
                return false;
            };

            service
        } else {
            let Some((_, service)) = filter.services().first() else {
                log::error!("サービスが存在しない");
                return false;
            };

            service
        };

        let Some(video_stream) = service
            .find_video_stream(selected_stream.video_stream.component_tag())
            .cloned()
        else {
            log::error!("映像ストリームが存在しない");
            return false;
        };
        let Some(audio_stream) = service
            .find_audio_stream(selected_stream.audio_stream.component_tag())
            .cloned()
        else {
            log::error!("映像ストリームが存在しない");
            return false;
        };

        *selected_stream = SelectedStream {
            service_id: service.service_id(),
            is_oneseg: service.is_oneseg(),

            video_stream,
            audio_stream,
            caption_pids: service.caption_streams().iter().map(|s| s.pid()).collect(),
        };
        true
    }

    // FIXME: 切り替えが遅すぎる
    pub fn select_service(&self, service_id: Option<ServiceId>) -> WinResult<()> {
        let _lock = self.mutex.lock();

        if self.is_switching.get() {
            return Err(MF::MF_E_INVALIDREQUEST.into());
        }

        if !self.select_service_internal(service_id) {
            return Err(F::E_INVALIDARG.into());
        }

        self.update_service()?;

        Ok(())
    }

    /// 指定された映像のコンポーネントタグから映像ストリームを選択する。
    pub fn select_video_stream(&self, component_tag: u8) -> WinResult<()> {
        let _lock = self.mutex.lock();

        if self.is_switching.get() {
            return Err(MF::MF_E_INVALIDREQUEST.into());
        }

        {
            let demuxer = self.demuxer.borrow();
            let filter = demuxer.filter();
            let mut selected_stream = self.selected_stream.borrow_mut();

            let service = filter.services().get(&selected_stream.service_id).unwrap();

            selected_stream.video_stream = service
                .find_video_stream(Some(component_tag))
                .cloned()
                .ok_or(F::E_INVALIDARG)?;
        }

        self.update_service()?;

        Ok(())
    }

    /// 指定された音声のコンポーネントタグから音声ストリームを選択する。
    // FIXME: 音が消える
    pub fn select_audio_stream(&self, component_tag: u8) -> WinResult<()> {
        let _lock = self.mutex.lock();

        if self.is_switching.get() {
            return Err(MF::MF_E_INVALIDREQUEST.into());
        }

        {
            let demuxer = self.demuxer.borrow();
            let filter = demuxer.filter();
            let mut selected_stream = self.selected_stream.borrow_mut();

            let service = filter.services().get(&selected_stream.service_id).unwrap();

            selected_stream.audio_stream = service
                .find_audio_stream(Some(component_tag))
                .cloned()
                .ok_or(F::E_INVALIDARG)?;
        }

        self.update_service()?;

        Ok(())
    }

    // TODO:コピーしたくない
    pub fn selected_service(&self) -> Option<isdb::filters::sorter::Service> {
        let _lock = self.mutex.lock();
        let service_id = self.selected_stream.borrow().service_id;
        self.demuxer
            .borrow()
            .filter()
            .services()
            .get(&service_id)
            .cloned()
    }

    pub fn active_video_tag(&self) -> Option<u8> {
        let _lock = self.mutex.lock();
        self.selected_stream.borrow().video_stream.component_tag()
    }

    pub fn active_audio_tag(&self) -> Option<u8> {
        let _lock = self.mutex.lock();
        self.selected_stream.borrow().audio_stream.component_tag()
    }

    // TODO:コピーしたくない
    pub fn services(&self) -> isdb::filters::sorter::ServiceMap {
        let _lock = self.mutex.lock();
        self.demuxer.borrow().filter().services().clone()
    }

    #[track_caller]
    fn enqueue_op<F: FnOnce(&TransportStream) -> WinResult<()> + 'static>(
        &self,
        f: F,
    ) -> WinResult<()> {
        let location = std::panic::Location::caller();

        let this = self.intf();
        self.queue().enqueue(move || {
            let this = this.as_impl();
            let _lock = this.mutex.lock();

            let r = f(this).and_then(|()| this.queue().process_queue());
            if let Err(e) = r {
                log::debug!("error[enqueue_op]: {} at {}", e, location);
                this.streaming_error(e);
            }
        })
    }

    fn streaming_error(&self, error: C::Error) {
        unsafe {
            if self.state.get() != SourceState::Shutdown {
                let _ = self.intf().QueueEvent(
                    MF::MEError.0 as u32,
                    &GUID_NULL,
                    error.into(),
                    std::ptr::null(),
                );
            }
        }
    }

    fn check_shutdown(&self) -> WinResult<()> {
        if self.state.get() == SourceState::Shutdown {
            Err(MF::MF_E_SHUTDOWN.into())
        } else {
            Ok(())
        }
    }

    fn validate_presentation_descriptor(
        &self,
        pd: &MF::IMFPresentationDescriptor,
    ) -> WinResult<()> {
        unsafe {
            let streams = self.streams.borrow();
            let c_streams = pd.GetStreamDescriptorCount()?;

            let mut any_selected = false;
            for i in 0..c_streams {
                let (selected, sd) = get_stream_descriptor_by_index(pd, i)?;

                let pid16 = sd.GetStreamIdentifier()? as Pid16;
                if !streams.contains_key(&pid16) {
                    return Err(F::E_INVALIDARG.into());
                }

                if selected {
                    // PIDを全部チェックしたいのでbreakはしない
                    any_selected = true;
                }
            }

            if !any_selected {
                return Err(F::E_INVALIDARG.into());
            }

            Ok(())
        }
    }

    fn do_start(
        &self,
        pd: &MF::IMFPresentationDescriptor,
        start_pos: &PropVariant,
    ) -> WinResult<()> {
        fn do_start(
            this: &TransportStream,
            pd: &MF::IMFPresentationDescriptor,
            start_pos: &PropVariant,
        ) -> WinResult<()> {
            unsafe {
                log::trace!("TransportStream::do_start");

                // TODO: start_posを元にthis.fileをシーク

                this.select_streams(pd, Some(start_pos))?;

                this.state.set(SourceState::Started);

                this.event_queue.QueueEventParamVar(
                    MF::MESourceStarted.0 as u32,
                    &GUID_NULL,
                    F::S_OK,
                    &start_pos.to_raw(),
                )?;
                this.is_switching.set(false);

                Ok(())
            }
        }

        let r = do_start(self, pd, start_pos);
        if let Err(ref e) = r {
            log::debug!("error[do_start]: {}", e);
            unsafe {
                let _ = self.event_queue.QueueEventParamVar(
                    MF::MESourceStarted.0 as u32,
                    &GUID_NULL,
                    e.code(),
                    std::ptr::null(),
                );
            }
        }

        r
    }

    fn do_stop(&self) -> WinResult<()> {
        unsafe {
            log::trace!("TransportStream::do_stop");

            for stream in self.streams.borrow().values() {
                let stream: &ElementaryStream = stream.as_impl();
                if stream.is_active() {
                    stream.stop()?;
                }
            }

            let _ = self.file.borrow_mut().seek(std::io::SeekFrom::Start(0));
            self.demuxer.borrow_mut().reset_packets();

            self.state.set(SourceState::Stopped);

            self.event_queue.QueueEventParamVar(
                MF::MESourceStopped.0 as u32,
                &GUID_NULL,
                F::S_OK,
                std::ptr::null(),
            )?;

            Ok(())
        }
    }

    fn do_pause(&self) -> WinResult<()> {
        unsafe {
            log::trace!("TransportStream::do_pause");

            for stream in self.streams.borrow().values() {
                let stream: &ElementaryStream = stream.as_impl();
                if stream.is_active() {
                    stream.pause()?;
                }
            }

            self.state.set(SourceState::Paused);

            self.event_queue.QueueEventParamVar(
                MF::MESourcePaused.0 as u32,
                &GUID_NULL,
                F::S_OK,
                std::ptr::null(),
            )?;

            Ok(())
        }
    }

    fn end_of_stream(&self) -> WinResult<()> {
        unsafe {
            log::trace!("TransportStream::end_of_stream");

            let count = self.pending_eos.get() - 1;
            self.pending_eos.set(count);
            if count == 0 {
                self.event_queue.QueueEventParamVar(
                    MF::MEEndOfPresentation.0 as u32,
                    &GUID_NULL,
                    F::S_OK,
                    std::ptr::null(),
                )?;
            }

            Ok(())
        }
    }

    fn select_streams(
        &self,
        pd: &MF::IMFPresentationDescriptor,
        start_pos: Option<&PropVariant>,
    ) -> WinResult<()> {
        unsafe {
            self.pending_eos.set(0);
            let streams = self.streams.borrow();

            for i in 0..streams.len() as u32 {
                let (selected, sd) = get_stream_descriptor_by_index(pd, i)?;
                let stream_pid = sd.GetStreamIdentifier()? as Pid16;
                let es: &ElementaryStream =
                    streams.get(&stream_pid).ok_or(F::E_INVALIDARG)?.as_impl();

                let was_selected = es.is_active();
                es.activate(selected);

                if selected {
                    log::debug!("selected {:p} {:?}", es, es.stream_pid.get());
                    self.pending_eos.set(self.pending_eos.get() + 1);

                    if was_selected {
                        log::trace!("MEUpdatedStream {:p}", es);
                        self.event_queue.QueueEventParamUnk(
                            MF::MEUpdatedStream.0 as u32,
                            &GUID_NULL,
                            F::S_OK,
                            &es.intf(),
                        )?;
                    } else {
                        log::trace!("MENewStream {:p}", es);
                        self.event_queue.QueueEventParamUnk(
                            MF::MENewStream.0 as u32,
                            &GUID_NULL,
                            F::S_OK,
                            &es.intf(),
                        )?;
                    }

                    es.start(start_pos)?;
                }
            }

            Ok(())
        }
    }

    fn streams_need_data(&self) -> bool {
        match self.state.get() {
            SourceState::Shutdown => false,
            _ => self
                .streams
                .borrow()
                .values()
                .any(|s| AsImpl::<ElementaryStream>::as_impl(s).needs_data()),
        }
    }

    fn end_of_mpeg_stream(&self) -> WinResult<()> {
        for stream in self.streams.borrow().values() {
            let stream: &ElementaryStream = stream.as_impl();
            if stream.is_active() {
                stream.end_of_stream()?;
            }
        }

        Ok(())
    }

    fn create_sample(&self, packet: &ESPacket) -> WinResult<MF::IMFSample> {
        unsafe {
            let buffer = MF::MFCreateMemoryBuffer(packet.payload.len() as u32)?;
            let mut data = std::ptr::null_mut();
            buffer.Lock(&mut data, None, None)?;
            std::ptr::copy_nonoverlapping(packet.payload.as_ptr(), data, packet.payload.len());
            buffer.Unlock()?;
            buffer.SetCurrentLength(packet.payload.len() as u32)?;

            let sample = MF::MFCreateSample()?;
            sample.AddBuffer(&buffer)?;

            if let Some(pts) = packet.pts {
                let pts = if let Some(first_pts) = self.first_pts.get() {
                    // TODO: ラップアラウンドを考慮する
                    // pts - first_pts
                    pts.saturating_sub(first_pts)
                } else {
                    self.first_pts.set(Some(pts));
                    isdb::time::Timestamp(0)
                };
                sample.SetSampleTime((pts.as_nanos() / 100) as i64)?;
            }

            Ok(sample)
        }
    }

    fn dispatch_events(&self) -> WinResult<()> {
        let mut events = {
            let mut demuxer = self.demuxer.borrow_mut();
            let events = demuxer.filter_mut().shooter_mut();
            std::mem::take(&mut events.events)
        };

        let demuxer = self.demuxer.borrow();
        let filter = demuxer.filter();
        while let Some(event) = events.pop_front() {
            match event {
                PlaybackEvent::Pat => {
                    self.update_service()?;
                }
                PlaybackEvent::Pmt(service_id) => {
                    let demuxer = self.demuxer.borrow();
                    if demuxer.filter().services().values().all(|s| s.pmt_filled()) {
                        self.update_streams()?;
                    }

                    if self.selected_stream.borrow().service_id == service_id {
                        self.update_service()?;
                    }
                }
                PlaybackEvent::Eit(service_id) => {
                    if self.selected_stream.borrow().service_id != service_id {
                        continue;
                    }

                    let Some(event) = filter
                        .services()
                        .get(&service_id)
                        .and_then(|service| service.present_event())
                    else {
                        continue;
                    };
                    if let Some(name) = &event.name {
                        log::info!("event changed: {}", name.display(Default::default()));
                    }
                }
                PlaybackEvent::Video(packet) | PlaybackEvent::Audio(packet) => {
                    let streams = self.streams.borrow();
                    let Some(stream) = streams.get(&packet.pid.get()) else {
                        // PMT完成前に受信設定されたパケット
                        continue;
                    };
                    let stream: &ElementaryStream = stream.as_impl();
                    if !stream.is_active() {
                        continue;
                    }

                    stream.deliver_payload(self.create_sample(&packet)?)?;
                }
                PlaybackEvent::Caption(pid, captions) => {
                    let selected_stream = self.selected_stream.borrow();
                    if !selected_stream.caption_pids.contains(&pid) {
                        continue;
                    }

                    let decode_opts = if selected_stream.is_oneseg {
                        isdb::eight::decode::Options::ONESEG_CAPTION
                    } else {
                        isdb::eight::decode::Options::CAPTION
                    };
                    for caption in captions {
                        let caption = caption.to_string(decode_opts);
                        if !caption.is_empty() {
                            log::info!("caption: {}", caption);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn request_sample(&self) -> WinResult<()> {
        self.dispatch_events()?;

        while self.streams_need_data() {
            // TODO: 別スレッドに逃がす
            let Some(packet) = isdb::Packet::read(&mut *self.file.borrow_mut()).map_err(io_to_hresult)? else {
                self.end_of_mpeg_stream()?;
                break;
            };
            self.demuxer.borrow_mut().feed(&packet);

            self.dispatch_events()?;
        }

        Ok(())
    }
}

#[allow(non_snake_case)]
impl MF::IMFGetService_Impl for TransportStream {
    fn GetService(
        &self,
        sid: *const windows::core::GUID,
        iid: *const windows::core::GUID,
        ppv: *mut *mut core::ffi::c_void,
    ) -> WinResult<()> {
        unsafe {
            use windows::core::Vtable;

            match (*sid, *iid) {
                (MF::MF_RATE_CONTROL_SERVICE, MF::IMFRateControl::IID) => {
                    *ppv = self.cast::<MF::IMFRateControl>().unwrap().into_raw();
                    Ok(())
                }
                (MF::MF_RATE_CONTROL_SERVICE, MF::IMFRateSupport::IID) => {
                    *ppv = self.cast::<MF::IMFRateSupport>().unwrap().into_raw();
                    Ok(())
                }
                _ => {
                    *ppv = std::ptr::null_mut();
                    Err(MF::MF_E_UNSUPPORTED_SERVICE.into())
                }
            }
        }
    }
}

#[allow(non_snake_case)]
impl MF::IMFMediaEventGenerator_Impl for TransportStream {
    fn GetEvent(
        &self,
        dwflags: MF::MEDIA_EVENT_GENERATOR_GET_EVENT_FLAGS,
    ) -> WinResult<MF::IMFMediaEvent> {
        unsafe {
            log::trace!("TransportStream::GetEvent");

            let queue = {
                let _lock = self.mutex.lock();
                self.check_shutdown()?;
                self.event_queue.clone()
            };
            queue.GetEvent(dwflags.0)
        }
    }

    fn BeginGetEvent(
        &self,
        pcallback: &Option<MF::IMFAsyncCallback>,
        punkstate: &Option<C::IUnknown>,
    ) -> WinResult<()> {
        unsafe {
            log::trace!("TransportStream::BeginGetEvent");

            let _lock = self.mutex.lock();
            self.check_shutdown()?;
            self.event_queue
                .BeginGetEvent(pcallback.as_ref(), punkstate.as_ref())
        }
    }

    fn EndGetEvent(&self, presult: &Option<MF::IMFAsyncResult>) -> WinResult<MF::IMFMediaEvent> {
        unsafe {
            log::trace!("TransportStream::EndGetEvent");

            let _lock = self.mutex.lock();
            self.check_shutdown()?;
            self.event_queue.EndGetEvent(presult.as_ref())
        }
    }

    fn QueueEvent(
        &self,
        met: u32,
        guidextendedtype: *const C::GUID,
        hrstatus: C::HRESULT,
        pvvalue: *const windows::Win32::System::Com::StructuredStorage::PROPVARIANT,
    ) -> WinResult<()> {
        unsafe {
            log::trace!("TransportStream::QueueEvent");

            let _lock = self.mutex.lock();
            self.check_shutdown()?;
            self.event_queue
                .QueueEventParamVar(met, guidextendedtype, hrstatus, pvvalue)
        }
    }
}

#[allow(non_snake_case)]
impl MF::IMFMediaSource_Impl for TransportStream {
    fn GetCharacteristics(&self) -> WinResult<u32> {
        log::trace!("TransportStream::GetCharacteristics");

        let _lock = self.mutex.lock();
        self.check_shutdown()?;

        // TODO: リアルタイム視聴では0？
        Ok(MF::MFMEDIASOURCE_CAN_PAUSE.0 as u32)
    }

    fn CreatePresentationDescriptor(&self) -> WinResult<MF::IMFPresentationDescriptor> {
        unsafe {
            log::debug!("TransportStream::CreatePresentationDescriptor");

            let _lock = self.mutex.lock();
            self.check_shutdown()?;

            let pd = self.presentation_descriptor.borrow().Clone()?;
            Ok(pd)
        }
    }

    fn Start(
        &self,
        pd: &Option<MF::IMFPresentationDescriptor>,
        time_format: *const C::GUID,
        start_pos: *const windows::Win32::System::Com::StructuredStorage::PROPVARIANT,
    ) -> WinResult<()> {
        unsafe {
            log::debug!(
                "TransportStream::Start: pd={:?}, time_format={:?}, start_pos={:?}",
                pd,
                time_format.as_ref(),
                start_pos.as_ref().and_then(PropVariant::new),
            );

            let _lock = self.mutex.lock();

            let pd = pd.as_ref().ok_or(F::E_INVALIDARG)?;
            let Some(start_pos) = start_pos.as_ref() else {
                return Err(F::E_INVALIDARG.into());
            };
            if !time_format.is_null() && *time_format != GUID_NULL {
                return Err(MF::MF_E_UNSUPPORTED_TIME_FORMAT.into());
            }
            let start_pos = match PropVariant::new(start_pos) {
                Some(v @ PropVariant::Empty) => v,

                Some(v @ PropVariant::I64(_)) => {
                    if self.state.get() != SourceState::Stopped {
                        return Err(MF::MF_E_INVALIDREQUEST.into());
                    }

                    v
                }

                _ => return Err(MF::MF_E_UNSUPPORTED_TIME_FORMAT.into()),
            };

            self.check_shutdown()?;
            self.validate_presentation_descriptor(pd)?;

            let pd = pd.clone();
            self.enqueue_op({
                let pd = pd.clone();
                move |this| this.do_start(&pd, &start_pos)
            })?;

            Ok(())
        }
    }

    fn Stop(&self) -> WinResult<()> {
        log::debug!("TransportStream::Stop");

        let _lock = self.mutex.lock();
        self.check_shutdown()?;

        self.enqueue_op(move |this| this.do_stop())?;

        Ok(())
    }

    fn Pause(&self) -> WinResult<()> {
        log::debug!("TransportStream::Pause");

        let _lock = self.mutex.lock();
        self.check_shutdown()?;

        self.enqueue_op(move |this| this.do_pause())?;

        Ok(())
    }

    fn Shutdown(&self) -> WinResult<()> {
        unsafe {
            log::debug!("TransportStream::Shutdown");

            let _lock = self.mutex.lock();
            self.check_shutdown()?;

            for stream in self.streams.borrow().values() {
                let stream: &ElementaryStream = stream.as_impl();
                let _ = stream.shutdown();
            }
            let _ = self.event_queue.Shutdown();

            self.state.set(SourceState::Shutdown);
            Ok(())
        }
    }
}

#[allow(non_snake_case)]
impl MF::IMFRateControl_Impl for TransportStream {
    fn SetRate(&self, thin: F::BOOL, rate: f32) -> WinResult<()> {
        unsafe {
            log::trace!("TransportStream::SetRate");

            let _lock = self.mutex.lock();
            self.check_shutdown()?;

            // TODO: リアルタイム視聴では速度変更不可

            if rate < 0. {
                return Err(MF::MF_E_REVERSE_UNSUPPORTED.into());
            }
            if thin.as_bool() {
                return Err(MF::MF_E_THINNING_UNSUPPORTED.into());
            }

            self.rate.set(rate);

            self.event_queue.QueueEventParamVar(
                MF::MESourceRateChanged.0 as u32,
                &GUID_NULL,
                F::S_OK,
                &PropVariant::F32(rate).to_raw(),
            )?;

            Ok(())
        }
    }

    fn GetRate(&self, thin: *mut F::BOOL, rate: *mut f32) -> WinResult<()> {
        unsafe {
            log::trace!("TransportStream::GetRate");

            let _lock = self.mutex.lock();
            self.check_shutdown()?;

            if let Some(thin) = thin.as_mut() {
                *thin = F::FALSE;
            }
            if let Some(rate) = rate.as_mut() {
                *rate = self.rate.get();
            }

            Ok(())
        }
    }
}

#[allow(non_snake_case)]
impl MF::IMFRateSupport_Impl for TransportStream {
    fn GetSlowestRate(
        &self,
        dir: MF::MFRATE_DIRECTION,
        thin: F::BOOL,
    ) -> windows::core::Result<f32> {
        log::trace!("TransportStream::GetSlowestRate");

        let _lock = self.mutex.lock();
        self.check_shutdown()?;

        if dir == MF::MFRATE_REVERSE {
            return Err(MF::MF_E_REVERSE_UNSUPPORTED.into());
        }
        if thin.as_bool() {
            return Err(MF::MF_E_THINNING_UNSUPPORTED.into());
        }

        // TODO: リアルタイム視聴では1.0？
        Ok(0.)
    }

    fn GetFastestRate(
        &self,
        dir: MF::MFRATE_DIRECTION,
        thin: F::BOOL,
    ) -> windows::core::Result<f32> {
        log::trace!("TransportStream::GetFastestRate");

        let _lock = self.mutex.lock();
        self.check_shutdown()?;

        if dir == MF::MFRATE_REVERSE {
            return Err(MF::MF_E_REVERSE_UNSUPPORTED.into());
        }
        if thin.as_bool() {
            return Err(MF::MF_E_THINNING_UNSUPPORTED.into());
        }

        // TODO: リアルタイム視聴では1.0？
        Ok(128.)
    }

    fn IsRateSupported(
        &self,
        thin: F::BOOL,
        rate: f32,
        nearest_supported_rate: *mut f32,
    ) -> windows::core::Result<()> {
        unsafe {
            log::trace!("TransportStream::IsRateSupported");

            let _lock = self.mutex.lock();
            self.check_shutdown()?;

            if rate < 0. {
                return Err(MF::MF_E_REVERSE_UNSUPPORTED.into());
            }
            if thin.as_bool() {
                return Err(MF::MF_E_THINNING_UNSUPPORTED.into());
            }

            // TODO: リアルタイム視聴では1.0以外不可？
            if rate > 128. {
                if let Some(nearest_supported_rate) = nearest_supported_rate.as_mut() {
                    *nearest_supported_rate = 128.;
                }

                return Err(MF::MF_E_UNSUPPORTED_RATE.into());
            }

            if let Some(nearest_supported_rate) = nearest_supported_rate.as_mut() {
                *nearest_supported_rate = rate;
            }

            Ok(())
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StreamState {
    Stopped,
    Paused,
    Started,
    Shutdown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct VefInfo {
    /// デコード後の幅。
    pub decoded_width: u32,
    /// デコード後の高さ。
    pub decoded_height: u32,
    /// エンコードされた状態での幅。
    pub width: u32,
    /// エンコードされた状態での高さ。
    pub height: u32,
    /// FPS。
    pub fps: u16,
    /// インターレースかどうか。
    pub is_interlace: bool,
}

impl VefInfo {
    pub fn new(vef: isdb::psi::desc::VideoEncodeFormat) -> Option<VefInfo> {
        use isdb::psi::desc::VideoEncodeFormat;

        match vef {
            VideoEncodeFormat::Vef1080P => Some(VefInfo {
                decoded_width: 1920,
                decoded_height: 1080,
                width: 1920,
                height: 1088,
                fps: 30,
                is_interlace: false,
            }),
            VideoEncodeFormat::Vef1080I => Some(VefInfo {
                decoded_width: 1920,
                decoded_height: 1080,
                width: 1440,
                height: 1088,
                fps: 30,
                is_interlace: true,
            }),
            VideoEncodeFormat::Vef720P => Some(VefInfo {
                decoded_width: 1280,
                decoded_height: 720,
                width: 1280,
                height: 720,
                fps: 30,
                is_interlace: false,
            }),
            VideoEncodeFormat::Vef480P => Some(VefInfo {
                decoded_width: 720,
                decoded_height: 480,
                width: 720,
                height: 480,
                fps: 30,
                is_interlace: false,
            }),
            VideoEncodeFormat::Vef480I => Some(VefInfo {
                decoded_width: 720,
                decoded_height: 480,
                width: 544,
                height: 480,
                fps: 30,
                is_interlace: true,
            }),
            // VideoEncodeFormat::Vef240P => Some(VefInfo {
            //     decoded_width: todo!(),
            //     decoded_height: 240,
            //     width: todo!(),
            //     height: 240,
            //     fps: 30,
            //     is_interlace: false,
            // }),
            // VideoEncodeFormat::Vef120P => Some(VefInfo {
            //     decoded_width: todo!(),
            //     decoded_height: 120,
            //     width: todo!(),
            //     height: 120,
            //     fps: 30,
            //     is_interlace: false,
            // }),
            VideoEncodeFormat::Vef2160_60P => Some(VefInfo {
                decoded_width: 3840,
                decoded_height: 2160,
                width: 3840,
                height: 2160,
                fps: 60,
                is_interlace: false,
            }),
            // VideoEncodeFormat::Vef180P => Some(VefInfo {
            //     decoded_width: todo!(),
            //     decoded_height: 180,
            //     width: todo!(),
            //     height: 180,
            //     fps: 30,
            //     is_interlace: false,
            // }),
            VideoEncodeFormat::Vef2160_120P => Some(VefInfo {
                decoded_width: 3840,
                decoded_height: 2160,
                width: 3840,
                height: 2160,
                fps: 120,
                is_interlace: false,
            }),
            VideoEncodeFormat::Vef4320_60P => Some(VefInfo {
                decoded_width: 7680,
                decoded_height: 4320,
                width: 7680,
                height: 4320,
                fps: 60,
                is_interlace: false,
            }),
            VideoEncodeFormat::Vef4320_120P => Some(VefInfo {
                decoded_width: 7680,
                decoded_height: 4320,
                width: 7680,
                height: 4320,
                fps: 120,
                is_interlace: false,
            }),
            _ => None,
        }
    }
}

#[implement(MF::IMFMediaStream)]
pub struct ElementaryStream {
    mutex: Arc<parking_lot::ReentrantMutex<()>>,

    source: C::Weak<MF::IMFMediaSource>,
    event_queue: MF::IMFMediaEventQueue,

    stream_pid: Cell<isdb::Pid>,
    stream_type: Cell<isdb::psi::desc::StreamType>,
    stream_descriptor: RefCell<MF::IMFStreamDescriptor>,

    state: Cell<StreamState>,
    is_active: Cell<bool>,
    is_eos: Cell<bool>,

    samples: RefCell<VecDeque<MF::IMFSample>>,
    requests: RefCell<VecDeque<Option<C::IUnknown>>>,
}

impl ElementaryStream {
    fn create_video_descriptor(
        stream_type: isdb::psi::desc::StreamType,
        stream_pid: isdb::Pid,
        vef: isdb::psi::desc::VideoEncodeFormat,
    ) -> WinResult<MF::IMFStreamDescriptor> {
        unsafe {
            use isdb::psi::desc::StreamType;

            let media_type = MF::MFCreateMediaType()?;
            match stream_type {
                StreamType::MPEG2_VIDEO => {
                    media_type.SetGUID(&MF::MF_MT_MAJOR_TYPE, &MF::MFMediaType_Video)?;
                    media_type.SetGUID(&MF::MF_MT_SUBTYPE, &MF::MFVideoFormat_MPEG2)?;
                    media_type.SetUINT32(&MF::MF_MT_FIXED_SIZE_SAMPLES, 0)?;
                    media_type.SetUINT32(&MF::MF_MT_COMPRESSED, 1)?;

                    if let Some(info) = VefInfo::new(vef) {
                        media_type.SetUINT64(
                            &MF::MF_MT_FRAME_SIZE,
                            (info.width as u64) << 32 | (info.height as u64),
                        )?;

                        if info.is_interlace {
                            let (numerator, denominator) = if info.height == 1088 {
                                (16, 9)
                            } else {
                                (info.decoded_width, info.decoded_height)
                            };
                            media_type.SetUINT64(
                                &MF::MF_MT_PIXEL_ASPECT_RATIO,
                                (numerator as u64) << 32 | (denominator as u64),
                            )?;
                        }
                    }
                }
                StreamType::H264 => {
                    media_type.SetGUID(&MF::MF_MT_MAJOR_TYPE, &MF::MFMediaType_Video)?;
                    media_type.SetGUID(&MF::MF_MT_SUBTYPE, &MF::MFVideoFormat_H264)?;
                    media_type.SetUINT32(&MF::MF_MT_FIXED_SIZE_SAMPLES, 0)?;
                    media_type.SetUINT32(&MF::MF_MT_COMPRESSED, 1)?;
                }
                StreamType::H265 => {
                    media_type.SetGUID(&MF::MF_MT_MAJOR_TYPE, &MF::MFMediaType_Video)?;
                    media_type.SetGUID(&MF::MF_MT_SUBTYPE, &MF::MFVideoFormat_H265)?;
                    media_type.SetUINT32(&MF::MF_MT_FIXED_SIZE_SAMPLES, 0)?;
                    media_type.SetUINT32(&MF::MF_MT_COMPRESSED, 1)?;
                }

                _ => return Err(F::E_INVALIDARG.into()),
            }

            let stream_descriptor =
                MF::MFCreateStreamDescriptor(stream_pid.get() as u32, &[media_type.clone()])?;

            let handler = stream_descriptor.GetMediaTypeHandler()?;
            handler.SetCurrentMediaType(&media_type)?;

            Ok(stream_descriptor)
        }
    }

    fn create_audio_descriptor(
        stream_type: isdb::psi::desc::StreamType,
        stream_pid: isdb::Pid,
    ) -> WinResult<MF::IMFStreamDescriptor> {
        const SAMPLES_PER_SEC: u32 = 48000;
        const NUM_CHANNELS: u32 = 2;
        const BITS_PER_SAMPLE: u32 = 16;
        const BLOCK_ALIGNMENT: u32 = BITS_PER_SAMPLE * NUM_CHANNELS / 8;
        const AVG_BYTES_PER_SECOND: u32 = SAMPLES_PER_SEC * BLOCK_ALIGNMENT;

        unsafe {
            let media_type = MF::MFCreateMediaType()?;

            use isdb::psi::desc::StreamType;
            match stream_type {
                StreamType::MPEG1_AUDIO | StreamType::MPEG2_AUDIO => {
                    // media_type.SetGUID(&MF::MF_MT_MAJOR_TYPE, &MF::MFMediaType_Audio)?;
                    // media_type.SetGUID(&MF::MF_MT_SUBTYPE, &MF::MFAudioFormat_MPEG)?;
                    todo!()
                }
                StreamType::AAC => {
                    media_type.SetGUID(&MF::MF_MT_MAJOR_TYPE, &MF::MFMediaType_Audio)?;
                    media_type.SetGUID(&MF::MF_MT_SUBTYPE, &MF::MFAudioFormat_AAC)?;
                    media_type.SetUINT32(&MF::MF_MT_AUDIO_NUM_CHANNELS, NUM_CHANNELS)?;
                    media_type.SetUINT32(&MF::MF_MT_AUDIO_SAMPLES_PER_SECOND, SAMPLES_PER_SEC)?;
                    media_type
                        .SetUINT32(&MF::MF_MT_AUDIO_AVG_BYTES_PER_SECOND, AVG_BYTES_PER_SECOND)?;
                    media_type.SetUINT32(&MF::MF_MT_AUDIO_BLOCK_ALIGNMENT, BLOCK_ALIGNMENT)?;
                    media_type.SetUINT32(&MF::MF_MT_AUDIO_BITS_PER_SAMPLE, BITS_PER_SAMPLE)?;

                    // HEAACWAVEINFOとaudioSpecificConfig()
                    #[repr(C, packed(1))]
                    #[allow(non_snake_case)]
                    struct AacInfo {
                        wPayloadType: u16,
                        wAudioProfileLevelIndication: u16,
                        wStructType: u16,
                        wReserved1: u16,
                        dwReserved2: u32,
                        // https://wiki.multimedia.cx/index.php/MPEG-4_Audio
                        audioSpecificConfig: [u8; 2],
                    }
                    const fn audio_specific_config(
                        audio_object_type: u8,
                        freq: u32,
                        channel_configuration: u8,
                    ) -> [u8; 2] {
                        let sampling_frequency_index = match freq {
                            96000 => 0,
                            88200 => 1,
                            64000 => 2,
                            48000 => 3,
                            44100 => 4,
                            32000 => 5,
                            24000 => 6,
                            22050 => 7,
                            16000 => 8,
                            12000 => 9,
                            11025 => 10,
                            8000 => 11,
                            7350 => 12,
                            _ => unreachable!(),
                        };

                        u16::to_be_bytes(
                            (audio_object_type as u16) << (16 - 5)
                                | sampling_frequency_index << (16 - 5 - 4)
                                | (channel_configuration as u16) << (16 - 5 - 4 - 4),
                        )
                    }

                    const AAC_INFO: AacInfo = AacInfo {
                        wPayloadType: 1, // ADTS
                        wAudioProfileLevelIndication: 0x29,
                        wStructType: 0,
                        wReserved1: 0,
                        dwReserved2: 0,
                        audioSpecificConfig: audio_specific_config(
                            2, // AAC LC
                            SAMPLES_PER_SEC,
                            NUM_CHANNELS as u8,
                        ),
                    };
                    const USER_DATA: [u8; 14] = unsafe { std::mem::transmute(AAC_INFO) };
                    media_type.SetBlob(&MF::MF_MT_USER_DATA, &USER_DATA)?;
                }
                StreamType::AC3 => {
                    // media_type.SetGUID(&MF::MF_MT_MAJOR_TYPE, &MF::MFMediaType_Audio)?;
                    // media_type.SetGUID(&MF::MF_MT_SUBTYPE, &MF::MFAudioFormat_Dolby_AC3)?;
                    todo!()
                }

                _ => return Err(F::E_INVALIDARG.into()),
            }

            let stream_descriptor =
                MF::MFCreateStreamDescriptor(stream_pid.get() as u32, &[media_type.clone()])?;

            let handler = stream_descriptor.GetMediaTypeHandler()?;
            handler.SetCurrentMediaType(&media_type)?;

            Ok(stream_descriptor)
        }
    }

    fn new(
        source: &TransportStream,
        stream_type: isdb::psi::desc::StreamType,
        stream_pid: isdb::Pid,
        stream_descriptor: MF::IMFStreamDescriptor,
    ) -> WinResult<MF::IMFMediaStream> {
        unsafe {
            let event_queue = MF::MFCreateEventQueue()?;
            Ok(ElementaryStream {
                mutex: source.mutex.clone(),

                source: source.intf().downgrade().unwrap(),
                event_queue,

                stream_pid: Cell::new(stream_pid),
                stream_type: Cell::new(stream_type),
                stream_descriptor: RefCell::new(stream_descriptor),

                state: Cell::new(StreamState::Stopped),
                is_active: Cell::new(false),
                is_eos: Cell::new(false),

                samples: RefCell::new(VecDeque::new()),
                requests: RefCell::new(VecDeque::new()),
            }
            .into())
        }
    }

    pub fn video(
        source: &TransportStream,
        stream_type: isdb::psi::desc::StreamType,
        stream_pid: isdb::Pid,
        vef: isdb::psi::desc::VideoEncodeFormat,
    ) -> WinResult<MF::IMFMediaStream> {
        let sd = Self::create_video_descriptor(stream_type, stream_pid, vef)?;
        Self::new(source, stream_type, stream_pid, sd)
    }

    pub fn audio(
        source: &TransportStream,
        stream_type: isdb::psi::desc::StreamType,
        stream_pid: isdb::Pid,
    ) -> WinResult<MF::IMFMediaStream> {
        let sd = Self::create_audio_descriptor(stream_type, stream_pid)?;
        Self::new(source, stream_type, stream_pid, sd)
    }

    fn intf(&self) -> MF::IMFMediaStream {
        unsafe { self.cast().unwrap() }
    }

    fn source(&self) -> MF::IMFMediaSource {
        self.source.upgrade().unwrap()
    }

    fn check_shutdown(&self) -> WinResult<()> {
        if self.state.get() == StreamState::Shutdown {
            Err(MF::MF_E_SHUTDOWN.into())
        } else {
            Ok(())
        }
    }

    #[inline]
    pub fn is_active(&self) -> bool {
        self.is_active.get()
    }

    pub fn needs_data(&self) -> bool {
        let _lock = self.mutex.lock();

        self.is_active() && !self.is_eos.get() && self.samples.borrow().len() < SAMPLE_QUEUE
    }

    /// 映像ストリームのストリーム形式種別を更新する。
    ///
    /// 更新したかどうかを真偽値で返す。
    pub fn update_video_stream_type(
        &self,
        stream_type: isdb::psi::desc::StreamType,
        stream_pid: isdb::Pid,
        vef: isdb::psi::desc::VideoEncodeFormat,
    ) -> WinResult<bool> {
        if self.stream_type.get() == stream_type {
            return Ok(false);
        }

        let sd = Self::create_video_descriptor(stream_type, stream_pid, vef)?;
        self.stream_type.set(stream_type);
        *self.stream_descriptor.borrow_mut() = sd;
        log::debug!("stream_type updated");
        Ok(true)
    }

    /// 音声ストリームのストリーム形式種別を更新する。
    ///
    /// 更新したかどうかを真偽値で返す。
    pub fn update_audio_stream_type(
        &self,
        stream_type: isdb::psi::desc::StreamType,
        stream_pid: isdb::Pid,
    ) -> WinResult<bool> {
        if self.stream_type.get() == stream_type {
            return Ok(false);
        }

        let sd = Self::create_audio_descriptor(stream_type, stream_pid)?;
        self.stream_type.set(stream_type);
        *self.stream_descriptor.borrow_mut() = sd;
        Ok(true)
    }

    pub fn end_of_stream(&self) -> WinResult<()> {
        self.is_eos.set(true);
        self.dispatch_samples()
    }

    pub fn deliver_payload(&self, sample: MF::IMFSample) -> WinResult<()> {
        let _lock = self.mutex.lock();

        self.samples.borrow_mut().push_back(sample);
        self.dispatch_samples()?;

        Ok(())
    }

    pub fn dispatch_samples(&self) -> WinResult<()> {
        fn dispatch_samples(this: &ElementaryStream) -> WinResult<()> {
            unsafe {
                if this.state.get() != StreamState::Started {
                    return Ok(());
                }

                {
                    let mut samples = this.samples.borrow_mut();
                    let mut requests = this.requests.borrow_mut();
                    while !samples.is_empty() && !requests.is_empty() {
                        let sample = samples.pop_front().unwrap();
                        let token = requests.pop_front().unwrap();

                        if let Some(token) = token {
                            sample.SetUnknown(&MF::MFSampleExtension_Token, &token)?;
                        }

                        log::trace!("sample dispatching {:p}", this);
                        this.event_queue.QueueEventParamUnk(
                            MF::MEMediaSample.0 as u32,
                            &GUID_NULL,
                            F::S_OK,
                            &sample,
                        )?;
                    }
                }

                if this.samples.borrow().is_empty() && this.is_eos.get() {
                    log::debug!("sample exhausted ({:p})", this);
                    this.event_queue.QueueEventParamVar(
                        MF::MEEndOfStream.0 as u32,
                        &GUID_NULL,
                        F::S_OK,
                        std::ptr::null(),
                    )?;
                    this.source()
                        .as_impl()
                        .enqueue_op(|this| this.end_of_stream())?;
                } else if this.needs_data() {
                    this.source()
                        .as_impl()
                        .enqueue_op(|this| this.request_sample())?;
                }

                Ok(())
            }
        }

        let _lock = self.mutex.lock();

        let r = dispatch_samples(self);
        if let Err(ref e) = r {
            if self.state.get() != StreamState::Shutdown {
                log::debug!("error[dispatch_samples]: {}", e);
                unsafe {
                    let _ = self.source().QueueEvent(
                        MF::MEError.0 as u32,
                        &GUID_NULL,
                        e.code(),
                        std::ptr::null(),
                    );
                }
            }
        }

        r
    }

    pub fn activate(&self, active: bool) {
        let _lock = self.mutex.lock();

        if active == self.is_active.get() {
            return;
        }

        log::debug!("activate {} {:p} {:?}", active, self, self.stream_pid.get());
        self.is_active.set(active);

        if !active {
            self.samples.borrow_mut().clear();
            self.requests.borrow_mut().clear();
        }
    }

    pub fn start(&self, start_pos: Option<&PropVariant>) -> WinResult<()> {
        use MF::IMFMediaEventGenerator_Impl;

        let _lock = self.mutex.lock();
        self.check_shutdown()?;

        self.QueueEvent(
            MF::MEStreamStarted.0 as u32,
            &GUID_NULL,
            F::S_OK,
            match start_pos {
                Some(start_pos) => &start_pos.to_raw(),
                None => std::ptr::null(),
            },
        )?;
        self.state.set(StreamState::Started);
        self.dispatch_samples()?;

        Ok(())
    }

    pub fn pause(&self) -> WinResult<()> {
        use MF::IMFMediaEventGenerator_Impl;

        let _lock = self.mutex.lock();
        self.check_shutdown()?;

        self.state.set(StreamState::Paused);
        self.QueueEvent(
            MF::MEStreamPaused.0 as u32,
            &GUID_NULL,
            F::S_OK,
            std::ptr::null(),
        )?;

        Ok(())
    }

    pub fn stop(&self) -> WinResult<()> {
        use MF::IMFMediaEventGenerator_Impl;

        let _lock = self.mutex.lock();
        self.check_shutdown()?;

        self.requests.borrow_mut().clear();
        self.samples.borrow_mut().clear();

        self.state.set(StreamState::Stopped);
        self.QueueEvent(
            MF::MEStreamStopped.0 as u32,
            &GUID_NULL,
            F::S_OK,
            std::ptr::null(),
        )?;

        Ok(())
    }

    pub fn shutdown(&self) -> WinResult<()> {
        unsafe {
            let _lock = self.mutex.lock();
            self.check_shutdown()?;
            self.state.set(StreamState::Shutdown);

            let _ = self.event_queue.Shutdown();

            self.samples.borrow_mut().clear();
            self.requests.borrow_mut().clear();

            Ok(())
        }
    }
}

#[allow(non_snake_case)]
impl MF::IMFMediaEventGenerator_Impl for ElementaryStream {
    fn GetEvent(
        &self,
        dwflags: MF::MEDIA_EVENT_GENERATOR_GET_EVENT_FLAGS,
    ) -> WinResult<MF::IMFMediaEvent> {
        unsafe {
            log::trace!("ElementaryStream::GetEvent");

            let queue = {
                let _lock = self.mutex.lock();
                self.check_shutdown()?;
                self.event_queue.clone()
            };

            queue.GetEvent(dwflags.0)
        }
    }

    fn BeginGetEvent(
        &self,
        pcallback: &Option<MF::IMFAsyncCallback>,
        punkstate: &Option<C::IUnknown>,
    ) -> WinResult<()> {
        unsafe {
            log::trace!("ElementaryStream::BeginGetEvent");

            let _lock = self.mutex.lock();
            self.check_shutdown()?;

            self.event_queue
                .BeginGetEvent(pcallback.as_ref(), punkstate.as_ref())
        }
    }

    fn EndGetEvent(&self, presult: &Option<MF::IMFAsyncResult>) -> WinResult<MF::IMFMediaEvent> {
        unsafe {
            log::trace!("ElementaryStream::EndGetEvent");

            let _lock = self.mutex.lock();
            self.check_shutdown()?;

            self.event_queue.EndGetEvent(presult.as_ref())
        }
    }

    fn QueueEvent(
        &self,
        met: u32,
        guidextendedtype: *const C::GUID,
        hrstatus: C::HRESULT,
        pvvalue: *const windows::Win32::System::Com::StructuredStorage::PROPVARIANT,
    ) -> WinResult<()> {
        unsafe {
            log::trace!("ElementaryStream::QueueEvent");

            let _lock = self.mutex.lock();
            self.check_shutdown()?;

            self.event_queue
                .QueueEventParamVar(met, guidextendedtype, hrstatus, pvvalue)
        }
    }
}

#[allow(non_snake_case)]
impl MF::IMFMediaStream_Impl for ElementaryStream {
    fn GetMediaSource(&self) -> WinResult<MF::IMFMediaSource> {
        log::trace!("ElementaryStream::GetMediaSource");

        let _lock = self.mutex.lock();
        self.check_shutdown()?;
        Ok(self.source())
    }

    fn GetStreamDescriptor(&self) -> WinResult<MF::IMFStreamDescriptor> {
        log::trace!("ElementaryStream::GetStreamDescriptor ({:p})", self);

        let _lock = self.mutex.lock();
        self.check_shutdown()?;
        Ok(self.stream_descriptor.borrow().clone())
    }

    fn RequestSample(&self, ptoken: &Option<C::IUnknown>) -> WinResult<()> {
        fn request_sample(this: &ElementaryStream, ptoken: &Option<C::IUnknown>) -> WinResult<()> {
            this.check_shutdown()?;
            if this.state.get() == StreamState::Stopped || !this.is_active() {
                return Err(MF::MF_E_INVALIDREQUEST.into());
            }
            if this.is_eos.get() && this.samples.borrow().is_empty() {
                return Err(MF::MF_E_END_OF_STREAM.into());
            }

            this.requests.borrow_mut().push_back(ptoken.clone());
            this.dispatch_samples()?;

            Ok(())
        }

        log::trace!("ElementaryStream::RequestSample {:p}", self);

        let _lock = self.mutex.lock();

        let r = request_sample(self, ptoken);
        if let Err(ref e) = r {
            if self.state.get() != StreamState::Shutdown {
                log::debug!("error[RequestSample]: {}", e);
                unsafe {
                    self.source().QueueEvent(
                        MF::MEError.0 as u32,
                        &GUID_NULL,
                        e.code(),
                        std::ptr::null(),
                    )?;
                }
            }
        }

        Ok(())
    }
}
