mod dummy;
mod queue;
mod session;
mod source;
mod stream;

use std::ops::RangeInclusive;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use isdb::psi::table::ServiceId;
use parking_lot::{MappedMutexGuard, Mutex, MutexGuard};
use windows::core::Result as WinResult;
use windows::Win32::Foundation as F;
use windows::Win32::Media::MediaFoundation as MF;
use winit::platform::windows::WindowExtWindows;

use crate::extract::{self, ExtractHandler};
use crate::player::EventHandler;

use self::source::TransportStream;

#[derive(Debug, Clone)]
pub struct PlayerEvent(MF::IMFMediaEvent);

impl From<MF::IMFMediaEvent> for PlayerEvent {
    #[inline]
    fn from(event: MF::IMFMediaEvent) -> PlayerEvent {
        PlayerEvent(event)
    }
}

// Safety: C++のサンプルではスレッドをまたいで使っているので安全なはず
unsafe impl Send for PlayerEvent {}

#[derive(Debug, Clone)]
struct PlayerState {
    pub hwnd_video: F::HWND,
    pub bounds: (u32, u32, u32, u32),
    pub volume: f32,
    pub muted: bool,
    pub rate: f32,
}

#[derive(Clone)]
struct Sink<H> {
    player_state: Arc<Mutex<PlayerState>>,
    event_handler: H,
    extract_handler: ExtractHandler,
    session: Arc<Mutex<Option<session::Session>>>,
}

struct Opened<H> {
    thread_handle: Option<std::thread::JoinHandle<()>>,
    sink: Sink<H>,
}

pub struct Player<H> {
    player_state: Arc<Mutex<PlayerState>>,
    event_handler: H,
    opened: Option<Opened<H>>,
}

impl<H: EventHandler + Clone> Player<H> {
    pub fn new(window: &winit::window::Window, event_handler: H) -> Result<Player<H>> {
        unsafe {
            MF::MFStartup(
                (MF::MF_SDK_VERSION << 16) | MF::MF_API_VERSION,
                MF::MFSTARTUP_NOSOCKET,
            )?;
        }

        Ok(Player {
            player_state: Arc::new(Mutex::new(PlayerState {
                hwnd_video: F::HWND(window.hwnd()),
                bounds: (0, 0, 0, 0),
                volume: 1.0,
                muted: false,
                rate: 1.0,
            })),
            event_handler,
            opened: None,
        })
    }

    #[inline]
    pub fn is_opened(&self) -> bool {
        self.opened.is_some()
    }

    pub fn open<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        let _ = self.close();

        let file = std::fs::File::open(path)?;

        let extractor = extract::Extractor::new();
        let sink = Sink {
            player_state: self.player_state.clone(),
            event_handler: self.event_handler.clone(),
            extract_handler: extractor.handler(),
            session: Arc::new(Mutex::new(None)),
        };
        let thread_handle = extractor.spawn(file, sink.clone());

        self.opened = Some(Opened {
            sink,
            thread_handle: Some(thread_handle),
        });
        Ok(())
    }

    pub fn close(&mut self) -> Result<()> {
        if let Some(opened) = self.opened.take() {
            opened.sink.extract_handler.shutdown();

            if let Some(session) = &*opened.sink.session.lock() {
                session.close()?;
            }
            if let Some(thread_handle) = opened.thread_handle {
                let _ = thread_handle.join();
            }
        }
        Ok(())
    }

    fn no_session() -> anyhow::Error {
        anyhow::anyhow!("セッションがありません")
    }

    #[inline]
    fn session(&self) -> Option<MappedMutexGuard<session::Session>> {
        let opened = self.opened.as_ref()?;
        MutexGuard::try_map(opened.sink.session.lock(), |session| session.as_mut()).ok()
    }

    #[inline]
    fn session_must(&self) -> Result<MappedMutexGuard<session::Session>> {
        self.session().ok_or_else(Self::no_session)
    }

    #[inline]
    pub fn selected_service(&self) -> Option<isdb::filters::sorter::Service> {
        let opened = self.opened.as_ref()?;
        let selected_service = opened.sink.extract_handler.selected_stream();
        let selected_service = selected_service.as_ref()?;
        let service = opened.sink.extract_handler.services()[&selected_service.service_id].clone();
        Some(service)
    }

    #[inline]
    pub fn active_video_tag(&self) -> Option<u8> {
        let opened = self.opened.as_ref()?;
        let selected_stream = opened.sink.extract_handler.selected_stream();
        selected_stream.as_ref()?.video_stream.component_tag()
    }

    #[inline]
    pub fn active_audio_tag(&self) -> Option<u8> {
        let opened = self.opened.as_ref()?;
        let selected_stream = opened.sink.extract_handler.selected_stream();
        selected_stream.as_ref()?.audio_stream.component_tag()
    }

    #[inline]
    pub fn services(&self) -> Option<isdb::filters::sorter::ServiceMap> {
        let opened = self.opened.as_ref()?;
        let services = opened.sink.extract_handler.services();
        Some(services.clone())
    }

    #[inline]
    pub fn select_service(&mut self, service_id: Option<ServiceId>) -> Result<()> {
        let opened = self.opened.as_ref().ok_or_else(Self::no_session)?;
        opened.sink.extract_handler.select_service(service_id)?;
        Ok(())
    }

    #[inline]
    pub fn select_video_stream(&mut self, component_tag: u8) -> Result<()> {
        let opened = self.opened.as_ref().ok_or_else(Self::no_session)?;
        opened
            .sink
            .extract_handler
            .select_video_stream(component_tag)?;
        Ok(())
    }

    #[inline]
    pub fn select_audio_stream(&mut self, component_tag: u8) -> Result<()> {
        let opened = self.opened.as_ref().ok_or_else(Self::no_session)?;
        opened
            .sink
            .extract_handler
            .select_audio_stream(component_tag)?;
        Ok(())
    }

    #[inline]
    pub fn handle_event(&mut self, event: PlayerEvent) -> Result<()> {
        if let Some(session) = self.session() {
            session.handle_event(event.0)?;
        }
        Ok(())
    }

    #[inline]
    pub fn play(&mut self) -> Result<()> {
        self.session_must()?.play()?;
        Ok(())
    }

    #[inline]
    pub fn pause(&mut self) -> Result<()> {
        self.session_must()?.pause()?;
        Ok(())
    }

    #[inline]
    pub fn stop(&mut self) -> Result<()> {
        self.session_must()?.stop()?;
        Ok(())
    }

    #[inline]
    pub fn repaint(&mut self) -> Result<()> {
        if let Some(session) = self.session() {
            session.repaint()?;
        }
        Ok(())
    }

    #[inline]
    pub fn set_bounds(&mut self, left: u32, top: u32, right: u32, bottom: u32) -> Result<()> {
        if let Some(session) = self.session() {
            session.set_bounds(left, top, right, bottom)?;
        } else {
            self.player_state.lock().bounds = (left, top, right, bottom);
        }
        Ok(())
    }

    #[inline]
    pub fn duration(&self) -> Option<Duration> {
        let opened = self.opened.as_ref()?;
        opened.sink.extract_handler.duration()
    }

    #[inline]
    pub fn position(&self) -> Result<Duration> {
        let pos = self.session_must()?.position()?;
        Ok(pos)
    }

    #[inline]
    pub fn set_position(&mut self, pos: Duration) -> Result<()> {
        self.session_must()?.set_position(pos)?;
        Ok(())
    }

    #[inline]
    pub fn volume(&self) -> Result<f32> {
        let volume = self.player_state.lock().volume;
        Ok(volume)
    }

    #[inline]
    pub fn set_volume(&mut self, value: f32) -> Result<()> {
        if let Some(session) = self.session() {
            session.set_volume(value)?;
        } else {
            self.player_state.lock().volume = value;
        }
        Ok(())
    }

    #[inline]
    pub fn muted(&self) -> Result<bool> {
        let muted = self.player_state.lock().muted;
        Ok(muted)
    }

    #[inline]
    pub fn set_muted(&mut self, muted: bool) -> Result<()> {
        if let Some(session) = self.session() {
            session.set_muted(muted)?;
        } else {
            self.player_state.lock().muted = muted;
        }
        Ok(())
    }

    #[inline]
    pub fn rate_range(&self) -> Result<RangeInclusive<f32>> {
        let range = self.session_must()?.rate_range()?;
        Ok(range)
    }

    #[inline]
    pub fn rate(&self) -> Result<f32> {
        let rate = self.player_state.lock().rate;
        Ok(rate)
    }

    #[inline]
    pub fn set_rate(&mut self, value: f32) -> Result<()> {
        if let Some(session) = self.session() {
            session.set_rate(value)?;
        } else {
            self.player_state.lock().rate = value;
        }
        Ok(())
    }
}

impl<H> Drop for Player<H> {
    fn drop(&mut self) {
        let _ = unsafe { MF::MFShutdown() };
    }
}

impl<H: EventHandler + Clone> Sink<H> {
    /// サービスが未選択の場合はパニックする。
    fn reset(&self, changed: extract::StreamChanged) -> WinResult<()> {
        let mut guard = self.session.lock();
        if let Some(session) = &*guard {
            // 音声種別が変わらない場合は何もしない
            if !changed.video_type && !changed.video_pid && !changed.audio_type {
                return Ok(());
            }

            session.close()?;
            *guard = None;
        }

        let selected_stream = self.extract_handler.selected_stream();
        let selected_stream = selected_stream.as_ref().expect("サービス未選択");
        let source = TransportStream::new(
            self.extract_handler.clone(),
            &selected_stream.video_stream,
            &selected_stream.audio_stream,
        )?;

        *guard = Some(session::Session::new(
            self.player_state.clone(),
            self.event_handler.clone(),
            self.extract_handler.clone(),
            source,
        )?);
        Ok(())
    }
}

impl<H: EventHandler + Clone> extract::Sink for Sink<H> {
    fn on_services_updated(&mut self, services: &isdb::filters::sorter::ServiceMap) {
        self.event_handler.on_services_updated(services);
    }

    fn on_streams_updated(&mut self, service: &isdb::filters::sorter::Service) {
        self.event_handler.on_streams_updated(service);
    }

    fn on_event_updated(&mut self, service: &isdb::filters::sorter::Service, is_present: bool) {
        self.event_handler.on_event_updated(service, is_present);
    }

    fn on_service_changed(&mut self, service: &isdb::filters::sorter::Service) {
        self.event_handler.on_service_changed(service);
    }

    fn on_stream_changed(&mut self, changed: extract::StreamChanged) {
        // ストリームに変化があったということはサービスは選択されている
        match self.reset(changed.clone()) {
            Ok(()) => self.event_handler.on_stream_changed(changed),
            Err(e) => self.event_handler.on_stream_error(e.into()),
        }
    }

    fn on_video_packet(&mut self, pos: Option<Duration>, payload: &[u8]) {
        if let Some(session) = self.session.lock().as_ref() {
            session.source().deliver_video_packet(pos, payload);
        }
    }

    fn on_audio_packet(&mut self, pos: Option<Duration>, payload: &[u8]) {
        if let Some(session) = self.session.lock().as_ref() {
            session.source().deliver_audio_packet(pos, payload);
        }
    }

    fn on_caption(&mut self, caption: &isdb::filters::sorter::Caption) {
        self.event_handler.on_caption(caption);
    }

    fn on_superimpose(&mut self, caption: &isdb::filters::sorter::Caption) {
        self.event_handler.on_superimpose(caption);
    }

    fn on_end_of_stream(&mut self) {
        if let Some(session) = self.session.lock().as_ref() {
            let _ = session.source().end_of_mpeg_stream();
        }

        self.event_handler.on_end_of_stream();
    }

    fn on_stream_error(&mut self, error: std::io::Error) {
        self.on_end_of_stream();
        self.event_handler.on_stream_error(error.into());
    }

    fn needs_es(&self) -> bool {
        if let Some(session) = self.session.lock().as_ref() {
            session.source().streams_need_data()
        } else {
            false
        }
    }
}
