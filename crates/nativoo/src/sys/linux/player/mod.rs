use std::ops::RangeInclusive;
use std::path::Path;
use std::time::Duration;

use anyhow::Result;
use isdb::psi::table::ServiceId;

use crate::player::DualMonoMode;

#[derive(Debug, Clone)]
pub struct PlayerEvent(());

pub struct Player<H> {
    _marker: std::marker::PhantomData<H>,
}

impl<H> Player<H> {
    pub fn new(_window: &tao::window::Window, _event_handler: H) -> Result<Player<H>> {
        Ok(Player {
            _marker: std::marker::PhantomData,
        })
    }

    #[inline]
    pub fn is_opened(&self) -> bool {
        false
    }

    pub fn open<P: AsRef<Path>>(&mut self, _path: P) -> Result<()> {
        Ok(())
    }

    pub fn close(&mut self) -> Result<()> {
        Ok(())
    }

    #[inline]
    fn no_session() -> anyhow::Error {
        anyhow::anyhow!("セッションがありません")
    }

    pub fn selected_service(&self) -> Option<isdb::filters::sorter::Service> {
        None
    }

    pub fn active_video_tag(&self) -> Option<u8> {
        None
    }

    pub fn active_audio_tag(&self) -> Option<u8> {
        None
    }

    pub fn services(&self) -> Option<isdb::filters::sorter::ServiceMap> {
        None
    }

    pub fn select_service(&mut self, _service_id: Option<ServiceId>) -> Result<()> {
        Err(Self::no_session())
    }

    pub fn select_video_stream(&mut self, _component_tag: u8) -> Result<()> {
        Err(Self::no_session())
    }

    pub fn select_audio_stream(&mut self, _component_tag: u8) -> Result<()> {
        Err(Self::no_session())
    }

    pub fn handle_event(&mut self, _event: PlayerEvent) -> Result<()> {
        Ok(())
    }

    pub fn play(&mut self) -> Result<()> {
        Err(Self::no_session())
    }

    pub fn pause(&mut self) -> Result<()> {
        Err(Self::no_session())
    }

    pub fn stop(&mut self) -> Result<()> {
        Err(Self::no_session())
    }

    pub fn repaint(&mut self) -> Result<()> {
        Ok(())
    }

    pub fn set_bounds(&mut self, _left: u32, _top: u32, _right: u32, _bottom: u32) -> Result<()> {
        Ok(())
    }

    pub fn duration(&self) -> Option<Duration> {
        None
    }

    pub fn timestamp(&self) -> Option<Duration> {
        None
    }

    pub fn position(&self) -> Result<Duration> {
        Err(Self::no_session())
    }

    pub fn set_position(&mut self, _pos: Duration) -> Result<()> {
        Err(Self::no_session())
    }

    pub fn volume(&self) -> Result<f32> {
        Err(Self::no_session())
    }

    pub fn set_volume(&mut self, _value: f32) -> Result<()> {
        Ok(())
    }

    pub fn muted(&self) -> Result<bool> {
        Ok(false)
    }

    pub fn set_muted(&mut self, _muted: bool) -> Result<()> {
        Ok(())
    }

    pub fn rate_range(&self) -> Result<RangeInclusive<f32>> {
        Err(Self::no_session())
    }

    pub fn rate(&self) -> Result<f32> {
        Ok(1.0)
    }

    pub fn set_rate(&mut self, _value: f32) -> Result<()> {
        Ok(())
    }

    pub fn video_size(&self) -> Result<(u32, u32)> {
        Err(Self::no_session())
    }

    pub fn audio_channels(&self) -> Result<u8> {
        Err(Self::no_session())
    }

    pub fn dual_mono_mode(&self) -> Result<Option<DualMonoMode>> {
        Err(Self::no_session())
    }

    pub fn set_dual_mono_mode(&self, _mode: DualMonoMode) -> Result<()> {
        Err(Self::no_session())
    }
}
