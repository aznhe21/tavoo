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

#[derive(Default)]
struct State {
    thread_handle: Option<std::thread::JoinHandle<()>>,
    session: Option<session::Session>,
}

#[derive(Clone)]
struct Session<H> {
    hwnd_video: F::HWND,
    event_handler: H,
    extract_handler: ExtractHandler,
    state: Arc<Mutex<State>>,
}

pub struct Player<H> {
    hwnd_video: F::HWND,
    event_handler: H,
    session: Option<Session<H>>,
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
            hwnd_video: F::HWND(window.hwnd()),
            event_handler,
            session: None,
        })
    }

    #[inline]
    pub fn is_opened(&self) -> bool {
        self.session.is_some()
    }

    pub fn open<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        let _ = self.close();

        let extractor = extract::Extractor::new();
        let session = Session {
            hwnd_video: self.hwnd_video,
            event_handler: self.event_handler.clone(),
            extract_handler: extractor.handler(),
            state: Arc::new(Mutex::new(State::default())),
        };
        let file = std::fs::File::open(path)?;
        let thread_handle = extractor.spawn(file, session.clone());
        session.state.lock().thread_handle = Some(thread_handle);

        self.session = Some(session);
        Ok(())
    }

    pub fn close(&mut self) -> Result<()> {
        if let Some(session) = self.session.take() {
            session.extract_handler.shutdown();

            let (session, thread_handle) = {
                let mut state = session.state.lock();
                (state.session.take(), state.thread_handle.take())
            };

            if let Some(session) = session {
                session.close()?;
            }
            if let Some(thread_handle) = thread_handle {
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
        match &self.session {
            Some(session) => {
                MutexGuard::try_map(session.state.lock(), |state| state.session.as_mut()).ok()
            }
            None => None,
        }
    }

    #[inline]
    fn session_must(&self) -> Result<MappedMutexGuard<session::Session>> {
        self.session().ok_or_else(Self::no_session)
    }

    #[inline]
    pub fn selected_service(&self) -> Option<isdb::filters::sorter::Service> {
        let session = self.session.as_ref()?;
        let selected_service = session.extract_handler.selected_stream();
        let selected_service = selected_service.as_ref()?;
        let service = session.extract_handler.services()[&selected_service.service_id].clone();
        Some(service)
    }

    #[inline]
    pub fn active_video_tag(&self) -> Option<u8> {
        let session = self.session.as_ref()?;
        let selected_stream = session.extract_handler.selected_stream();
        selected_stream.as_ref()?.video_stream.component_tag()
    }

    #[inline]
    pub fn active_audio_tag(&self) -> Option<u8> {
        let session = self.session.as_ref()?;
        let selected_stream = session.extract_handler.selected_stream();
        selected_stream.as_ref()?.audio_stream.component_tag()
    }

    #[inline]
    pub fn services(&self) -> Option<isdb::filters::sorter::ServiceMap> {
        let session = self.session.as_ref()?;
        let services = session.extract_handler.services();
        Some(services.clone())
    }

    #[inline]
    pub fn select_service(&mut self, service_id: Option<ServiceId>) -> Result<()> {
        let session = self.session.as_ref().ok_or_else(Self::no_session)?;
        session.extract_handler.select_service(service_id);
        Ok(())
    }

    #[inline]
    pub fn select_video_stream(&mut self, component_tag: u8) -> Result<()> {
        let session = self.session.as_ref().ok_or_else(Self::no_session)?;
        session.extract_handler.select_video_stream(component_tag);
        Ok(())
    }

    #[inline]
    pub fn select_audio_stream(&mut self, component_tag: u8) -> Result<()> {
        let session = self.session.as_ref().ok_or_else(Self::no_session)?;
        session.extract_handler.select_audio_stream(component_tag);
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
    pub fn play_or_pause(&mut self) -> Result<()> {
        self.session_must()?.play_or_pause()?;
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
        }
        Ok(())
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
        let volume = self.session_must()?.volume()?;
        Ok(volume)
    }

    #[inline]
    pub fn set_volume(&mut self, value: f32) -> Result<()> {
        self.session_must()?.set_volume(value)?;
        Ok(())
    }

    #[inline]
    pub fn rate_range(&self) -> Result<RangeInclusive<f32>> {
        let range = self.session_must()?.rate_range()?;
        Ok(range)
    }

    #[inline]
    pub fn rate(&self) -> Result<f32> {
        let rate = self.session_must()?.rate()?;
        Ok(rate)
    }

    #[inline]
    pub fn set_rate(&mut self, value: f32) -> Result<()> {
        self.session_must()?.set_rate(value)?;
        Ok(())
    }
}

impl<H> Drop for Player<H> {
    fn drop(&mut self) {
        unsafe {
            let _ = MF::MFShutdown();
        }
    }
}

impl<H: EventHandler + Clone> Session<H> {
    /// サービスが未選択の場合はパニックする。
    fn reset(&self, changed: extract::StreamChanged) -> WinResult<()> {
        let mut state = self.state.lock();
        let (volume, rate) = if let Some(session) = state.session.take() {
            // 音声種別が変わらない場合は何もしない
            if !changed.video_type && !changed.video_pid && !changed.audio_type {
                return Ok(());
            }

            let volume = session.volume().ok();
            let rate = session.rate().ok();

            session.close()?;

            (volume, rate)
        } else {
            (None, None)
        };

        let selected_stream = self.extract_handler.selected_stream();
        let selected_stream = selected_stream.as_ref().expect("サービス未選択");
        let source = TransportStream::new(
            self.extract_handler.clone(),
            &selected_stream.video_stream,
            &selected_stream.audio_stream,
        )?;

        state.session = Some(session::Session::new(
            self.hwnd_video,
            self.event_handler.clone(),
            self.extract_handler.clone(),
            source,
            volume,
            rate,
        )?);
        Ok(())
    }
}

impl<H: EventHandler + Clone> extract::Sink for Session<H> {
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
        let source = self.state.lock().session.as_ref().map(|s| s.source());
        if let Some(source) = source {
            source.deliver_video_packet(pos, payload);
        }
    }

    fn on_audio_packet(&mut self, pos: Option<Duration>, payload: &[u8]) {
        let source = self.state.lock().session.as_ref().map(|s| s.source());
        if let Some(source) = source {
            source.deliver_audio_packet(pos, payload);
        }
    }

    fn on_caption(&mut self, caption: &isdb::filters::sorter::Caption) {
        self.event_handler.on_caption(caption);
    }

    fn on_superimpose(&mut self, caption: &isdb::filters::sorter::Caption) {
        self.event_handler.on_superimpose(caption);
    }

    fn on_end_of_stream(&mut self) {
        let source = self.state.lock().session.as_ref().map(|s| s.source());
        if let Some(source) = source {
            let _ = source.end_of_mpeg_stream();
        }

        self.event_handler.on_end_of_stream();
    }

    fn on_stream_error(&mut self, error: std::io::Error) {
        self.on_end_of_stream();
        self.event_handler.on_stream_error(error.into());
    }

    fn needs_es(&self) -> bool {
        let source = self.state.lock().session.as_ref().map(|s| s.source());
        if let Some(source) = source {
            source.streams_need_data()
        } else {
            false
        }
    }
}
