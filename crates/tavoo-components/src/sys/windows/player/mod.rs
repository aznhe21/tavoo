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
use parking_lot::Mutex;
use windows::Win32::Foundation as F;
use windows::Win32::Media::MediaFoundation as MF;
use winit::platform::windows::WindowExtWindows;

use crate::player::{DualMonoMode, EventHandler};

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

pub struct Player<H> {
    player_state: Arc<Mutex<PlayerState>>,
    event_handler: H,
    session: Option<session::Session>,
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
            session: None,
        })
    }

    #[inline]
    pub fn is_opened(&self) -> bool {
        self.session.is_some()
    }

    pub fn open<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        let _ = self.close();

        let file = std::fs::File::open(path)?;

        self.session = Some(session::Session::new(
            self.player_state.clone(),
            self.event_handler.clone(),
            file,
        )?);
        Ok(())
    }

    pub fn close(&mut self) -> Result<()> {
        if let Some(session) = self.session.take() {
            session.close()?;
        }
        Ok(())
    }

    #[inline]
    fn session_must(&self) -> Result<&session::Session> {
        self.session
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("セッションがありません"))
    }

    pub fn selected_service(&self) -> Option<isdb::filters::sorter::Service> {
        let extract_handler = self.session.as_ref()?.extract_handler();
        let selected_service = extract_handler.selected_stream();
        let selected_service = selected_service.as_ref()?;
        let service = extract_handler.services()[&selected_service.service_id].clone();
        Some(service)
    }

    pub fn active_video_tag(&self) -> Option<u8> {
        let extract_handler = self.session.as_ref()?.extract_handler();
        let selected_stream = extract_handler.selected_stream();
        selected_stream.as_ref()?.video_stream.component_tag()
    }

    pub fn active_audio_tag(&self) -> Option<u8> {
        let extract_handler = self.session.as_ref()?.extract_handler();
        let selected_stream = extract_handler.selected_stream();
        selected_stream.as_ref()?.audio_stream.component_tag()
    }

    pub fn services(&self) -> Option<isdb::filters::sorter::ServiceMap> {
        let extract_handler = self.session.as_ref()?.extract_handler();
        let services = extract_handler.services();
        Some(services.clone())
    }

    pub fn select_service(&mut self, service_id: Option<ServiceId>) -> Result<()> {
        let extract_handler = self.session_must()?.extract_handler();
        extract_handler.select_service(service_id)?;
        Ok(())
    }

    pub fn select_video_stream(&mut self, component_tag: u8) -> Result<()> {
        let extract_handler = self.session_must()?.extract_handler();
        extract_handler.select_video_stream(component_tag)?;
        Ok(())
    }

    pub fn select_audio_stream(&mut self, component_tag: u8) -> Result<()> {
        let extract_handler = self.session_must()?.extract_handler();
        extract_handler.select_audio_stream(component_tag)?;
        Ok(())
    }

    pub fn handle_event(&mut self, event: PlayerEvent) -> Result<()> {
        if let Some(session) = &self.session {
            session.handle_event(event.0)?;
        }
        Ok(())
    }

    pub fn play(&mut self) -> Result<()> {
        self.session_must()?.play()?;
        Ok(())
    }

    pub fn pause(&mut self) -> Result<()> {
        self.session_must()?.pause()?;
        Ok(())
    }

    pub fn stop(&mut self) -> Result<()> {
        self.session_must()?.stop()?;
        Ok(())
    }

    pub fn repaint(&mut self) -> Result<()> {
        if let Some(session) = &self.session {
            session.repaint()?;
        }
        Ok(())
    }

    pub fn set_bounds(&mut self, left: u32, top: u32, right: u32, bottom: u32) -> Result<()> {
        if let Some(session) = &self.session {
            session.set_bounds(left, top, right, bottom)?;
        } else {
            self.player_state.lock().bounds = (left, top, right, bottom);
        }
        Ok(())
    }

    pub fn duration(&self) -> Option<Duration> {
        self.session.as_ref()?.extract_handler().duration()
    }

    pub fn timestamp(&self) -> Option<Duration> {
        self.session.as_ref()?.extract_handler().timestamp()
    }

    pub fn position(&self) -> Result<Duration> {
        let pos = self.session_must()?.position()?;
        Ok(pos)
    }

    pub fn set_position(&mut self, pos: Duration) -> Result<()> {
        self.session_must()?.set_position(pos)?;
        Ok(())
    }

    pub fn volume(&self) -> Result<f32> {
        let volume = self.player_state.lock().volume;
        Ok(volume)
    }

    pub fn set_volume(&mut self, value: f32) -> Result<()> {
        if let Some(session) = &self.session {
            session.set_volume(value)?;
        } else {
            self.player_state.lock().volume = value;
        }
        Ok(())
    }

    pub fn muted(&self) -> Result<bool> {
        let muted = self.player_state.lock().muted;
        Ok(muted)
    }

    pub fn set_muted(&mut self, muted: bool) -> Result<()> {
        if let Some(session) = &self.session {
            session.set_muted(muted)?;
        } else {
            self.player_state.lock().muted = muted;
        }
        Ok(())
    }

    pub fn rate_range(&self) -> Result<RangeInclusive<f32>> {
        let range = self.session_must()?.rate_range()?;
        Ok(range)
    }

    pub fn rate(&self) -> Result<f32> {
        let rate = self.player_state.lock().rate;
        Ok(rate)
    }

    pub fn set_rate(&mut self, value: f32) -> Result<()> {
        if let Some(session) = &self.session {
            session.set_rate(value)?;
        } else {
            self.player_state.lock().rate = value;
        }
        Ok(())
    }

    pub fn video_size(&self) -> Result<(u32, u32)> {
        let size = self.session_must()?.video_size()?;
        Ok(size)
    }

    pub fn audio_channels(&self) -> Result<u8> {
        let num_channels = self.session_must()?.audio_channels()?;
        Ok(num_channels)
    }

    pub fn dual_mono_mode(&self) -> Result<Option<DualMonoMode>> {
        let mode = self.session_must()?.dual_mono_mode()?;
        Ok(mode)
    }

    pub fn set_dual_mono_mode(&self, mode: DualMonoMode) -> Result<()> {
        self.session_must()?.set_dual_mono_mode(mode)?;
        Ok(())
    }
}

impl<H> Drop for Player<H> {
    fn drop(&mut self) {
        let _ = unsafe { MF::MFShutdown() };
    }
}
