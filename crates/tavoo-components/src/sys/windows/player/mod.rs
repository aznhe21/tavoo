mod dummy;
mod queue;
mod session;
mod source;
mod stream;

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use isdb::psi::table::ServiceId;
use parking_lot::Mutex;
use windows::core::Result as WinResult;
use windows::Win32::Foundation as F;
use windows::Win32::Media::MediaFoundation as MF;
use winit::platform::windows::WindowExtWindows;

use crate::extract::{self, ExtractHandler};

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

struct Session<Event: 'static> {
    hwnd_video: F::HWND,
    event_loop: winit::event_loop::EventLoopProxy<Event>,
    handler: ExtractHandler,
    state: Arc<Mutex<State>>,
}

impl<Event> Clone for Session<Event> {
    fn clone(&self) -> Session<Event> {
        Session {
            hwnd_video: self.hwnd_video,
            event_loop: self.event_loop.clone(),
            handler: self.handler.clone(),
            state: self.state.clone(),
        }
    }
}

pub struct Player<Event: 'static> {
    hwnd_video: F::HWND,
    event_loop: winit::event_loop::EventLoopProxy<Event>,
    session: Option<Session<Event>>,
}

impl<Event> Player<Event> {
    pub fn new(
        window: &winit::window::Window,
        event_loop: winit::event_loop::EventLoopProxy<Event>,
    ) -> Result<Player<Event>> {
        unsafe {
            MF::MFStartup(
                (MF::MF_SDK_VERSION << 16) | MF::MF_API_VERSION,
                MF::MFSTARTUP_NOSOCKET,
            )?;
        }

        Ok(Player {
            hwnd_video: F::HWND(window.hwnd()),
            event_loop,
            session: None,
        })
    }

    #[inline]
    pub fn is_opened(&self) -> bool {
        self.session.is_some()
    }

    pub fn open<P: AsRef<Path>>(&mut self, path: P) -> Result<()>
    where
        Event: From<crate::player::PlayerEvent> + Send,
    {
        let _ = self.close();

        let extractor = extract::Extractor::new();
        let session = Session {
            hwnd_video: self.hwnd_video,
            event_loop: self.event_loop.clone(),
            handler: extractor.handler(),
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
            session.handler.shutdown();

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

    fn with_session_must<T, E, F>(&self, f: F) -> Result<T>
    where
        E: Into<anyhow::Error>,
        F: FnOnce(&session::Session) -> Result<T, E>,
    {
        let session = self.session.as_ref().ok_or_else(Self::no_session)?;
        let state = session.state.lock();
        let session = state.session.as_ref().ok_or_else(Self::no_session)?;
        f(session).map_err(Into::into)
    }

    #[inline]
    pub fn selected_service(&self) -> Option<isdb::filters::sorter::Service> {
        let session = self.session.as_ref()?;
        let selected_service = session.handler.selected_stream();
        let selected_service = selected_service.as_ref()?;
        let service = session.handler.services()[&selected_service.service_id].clone();
        Some(service)
    }

    #[inline]
    pub fn active_video_tag(&self) -> Option<u8> {
        let session = self.session.as_ref()?;
        let selected_stream = session.handler.selected_stream();
        selected_stream.as_ref()?.video_stream.component_tag()
    }

    #[inline]
    pub fn active_audio_tag(&self) -> Option<u8> {
        let session = self.session.as_ref()?;
        let selected_stream = session.handler.selected_stream();
        selected_stream.as_ref()?.audio_stream.component_tag()
    }

    #[inline]
    pub fn services(&self) -> Option<isdb::filters::sorter::ServiceMap> {
        let session = self.session.as_ref()?;
        let services = session.handler.services();
        Some(services.clone())
    }

    #[inline]
    pub fn select_service(&mut self, service_id: Option<ServiceId>) -> Result<()> {
        let session = self.session.as_ref().ok_or_else(Self::no_session)?;
        session.handler.select_service(service_id);
        Ok(())
    }

    #[inline]
    pub fn select_video_stream(&mut self, component_tag: u8) -> Result<()> {
        let session = self.session.as_ref().ok_or_else(Self::no_session)?;
        session.handler.select_video_stream(component_tag);
        Ok(())
    }

    #[inline]
    pub fn select_audio_stream(&mut self, component_tag: u8) -> Result<()> {
        let session = self.session.as_ref().ok_or_else(Self::no_session)?;
        session.handler.select_audio_stream(component_tag);
        Ok(())
    }

    #[inline]
    pub fn handle_event(&mut self, event: PlayerEvent) -> Result<()> {
        match &self.session {
            Some(session) => {
                let state = session.state.lock();
                if let Some(session) = &state.session {
                    session.handle_event(event.0)?;
                }
            }
            None => {}
        }

        Ok(())
    }

    #[inline]
    pub fn play(&mut self) -> Result<()> {
        self.with_session_must(|session| session.play())
    }

    #[inline]
    pub fn pause(&mut self) -> Result<()> {
        self.with_session_must(|session| session.pause())
    }

    #[inline]
    pub fn play_or_pause(&mut self) -> Result<()> {
        self.with_session_must(|session| session.play_or_pause())
    }

    #[inline]
    pub fn stop(&mut self) -> Result<()> {
        self.with_session_must(|session| session.stop())
    }

    #[inline]
    pub fn repaint(&mut self) -> Result<()> {
        match &self.session {
            Some(session) => {
                let state = session.state.lock();
                if let Some(session) = &state.session {
                    session.repaint()?;
                }
            }
            None => {}
        }

        Ok(())
    }

    #[inline]
    pub fn resize_video(&mut self, width: u32, height: u32) -> Result<()> {
        match &self.session {
            Some(session) => {
                let state = session.state.lock();
                if let Some(session) = &state.session {
                    session.resize_video(width, height)?;
                }
            }
            None => {}
        }
        Ok(())
    }

    #[inline]
    pub fn position(&self) -> Result<Duration> {
        self.with_session_must(|session| session.position())
    }

    #[inline]
    pub fn set_position(&mut self, pos: Duration) -> Result<()> {
        self.with_session_must(|session| session.set_position(pos))
    }

    #[inline]
    pub fn volume(&self) -> Result<f32> {
        self.with_session_must(|session| session.volume())
    }

    #[inline]
    pub fn set_volume(&mut self, value: f32) -> Result<()> {
        self.with_session_must(|session| session.set_volume(value))
    }

    #[inline]
    pub fn rate(&self) -> Result<f32> {
        self.with_session_must(|session| session.rate())
    }

    #[inline]
    pub fn set_rate(&mut self, value: f32) -> Result<()> {
        self.with_session_must(|session| session.set_rate(value))
    }
}

impl<Event> Drop for Player<Event> {
    fn drop(&mut self) {
        unsafe {
            let _ = MF::MFShutdown();
        }
    }
}

impl<Event> Session<Event>
where
    Event: From<crate::player::PlayerEvent> + Send,
{
    /// サービスが未選択の場合はパニックする。
    fn reset(&self, changed: extract::StreamChanged) -> WinResult<()> {
        let mut state = self.state.lock();
        let (volume, rate) = if let Some(session) = &state.session {
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

        let selected_stream = self.handler.selected_stream();
        let selected_stream = selected_stream.as_ref().expect("サービス未選択");
        let source = TransportStream::new(
            self.handler.clone(),
            &selected_stream.video_stream,
            &selected_stream.audio_stream,
        )?;
        let event_loop = session::EventLoopWrapper::new(self.event_loop.clone());

        state.session = Some(session::Session::new(
            self.hwnd_video,
            event_loop,
            self.handler.clone(),
            source,
            volume,
            rate,
        )?);
        Ok(())
    }
}

impl<Event> extract::Sink for Session<Event>
where
    Event: From<crate::player::PlayerEvent> + Send,
{
    fn on_services_updated(&mut self, _: &isdb::filters::sorter::ServiceMap) {
        // TODO: UIに通知
    }
    fn on_streams_updated(&mut self, _: &isdb::filters::sorter::Service) {
        // TODO: UIに通知
    }

    fn on_event_updated(&mut self, service: &isdb::filters::sorter::Service, is_present: bool) {
        // TODO: UIに通知
        let service_id = self
            .handler
            .selected_stream()
            .as_ref()
            .map(|ss| ss.service_id);
        if service_id != Some(service.service_id()) {
            return;
        }

        if is_present {
            if let Some(name) = service.present_event().and_then(|e| e.name.as_ref()) {
                log::info!("event changed: {}", name.display(Default::default()));
            }
        }
    }

    fn on_service_changed(&mut self, service: &isdb::filters::sorter::Service) {
        // TODO: UIに通知
        log::info!(
            "service changed: {} ({:04X})",
            service.service_name().display(Default::default()),
            service.service_id()
        );
    }

    fn on_stream_changed(&mut self, changed: extract::StreamChanged) {
        // ストリームに変化があったということはサービスは選択されている
        let r = self.reset(changed);
        if let Err(e) = r {
            // TODO: UIに通知
            log::error!("stream error: {}", e);
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
        // TODO: UIに通知
        let service_id = {
            let selected_stream = self.handler.selected_stream();
            let Some(selected_stream) = selected_stream.as_ref() else {
                return;
            };
            selected_stream.service_id
        };
        let decode_opts = if self.handler.services()[&service_id].is_oneseg() {
            isdb::eight::decode::Options::ONESEG_CAPTION
        } else {
            isdb::eight::decode::Options::CAPTION
        };

        for data_unit in caption.data_units() {
            let isdb::pes::caption::DataUnit::StatementBody(caption) = data_unit else {
                continue;
            };

            let caption = caption.to_string(decode_opts);
            if !caption.is_empty() {
                log::info!("caption: {}", caption);
            }
        }
    }

    fn on_superimpose(&mut self, caption: &isdb::filters::sorter::Caption) {
        // TODO: UIに通知
        let service_id = {
            let selected_stream = self.handler.selected_stream();
            let Some(selected_stream) = selected_stream.as_ref() else {
                return;
            };
            selected_stream.service_id
        };
        let decode_opts = if self.handler.services()[&service_id].is_oneseg() {
            isdb::eight::decode::Options::ONESEG_CAPTION
        } else {
            isdb::eight::decode::Options::CAPTION
        };

        for data_unit in caption.data_units() {
            let isdb::pes::caption::DataUnit::StatementBody(caption) = data_unit else {
                continue;
            };

            if !caption.is_empty() {
                log::info!("superimpose: {:?}", caption.display(decode_opts));
            }
        }
    }

    fn on_end_of_stream(&mut self) {
        let source = self.state.lock().session.as_ref().map(|s| s.source());
        if let Some(source) = source {
            let _ = source.end_of_mpeg_stream();
        }
    }

    fn on_stream_error(&mut self, error: std::io::Error) {
        self.on_end_of_stream();
        log::error!("stream error: {}", error);
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
