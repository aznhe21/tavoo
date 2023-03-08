use std::cell::{Cell, RefCell};
use std::marker::PhantomData;
use std::path::Path;
use std::time::Duration;

use anyhow::Result;
use isdb::psi::table::ServiceId;
use parking_lot::lock_api::{RawMutex, RawMutexTimed};
use raw_window_handle::{HasRawWindowHandle, RawWindowHandle};
use windows::core::{self as C, implement, AsImpl, Interface};
use windows::Win32::Foundation as F;
use windows::Win32::Media::KernelStreaming::GUID_NULL;
use windows::Win32::Media::MediaFoundation as MF;

use super::stream::TransportStream;
use super::utils::{get_stream_descriptor_by_index, PropVariant, WinResult};

pub type PlayerEvent = MF::IMFMediaEvent;

// ジェネリクスと#[implement]を併用できないので型消去
struct EventLoopWrapper(Box<dyn EventLoop>);

trait EventLoop {
    fn send_player_event(
        &self,
        event: PlayerEvent,
    ) -> Result<(), winit::event_loop::EventLoopClosed<()>>;
}

impl<E: From<crate::player::PlayerEvent>> EventLoop for winit::event_loop::EventLoopProxy<E> {
    fn send_player_event(
        &self,
        event: PlayerEvent,
    ) -> Result<(), winit::event_loop::EventLoopClosed<()>> {
        match self.send_event(crate::player::PlayerEvent(event).into()) {
            Ok(()) => Ok(()),
            Err(_) => Err(winit::event_loop::EventLoopClosed(())),
        }
    }
}

pub struct Player<E> {
    inner: MF::IMFAsyncCallback,
    _marker: PhantomData<E>,
}

impl<E: From<crate::player::PlayerEvent>> Player<E> {
    pub fn new(
        window: &winit::window::Window,
        event_loop: winit::event_loop::EventLoopProxy<E>,
    ) -> Result<Player<E>> {
        let RawWindowHandle::Win32(handle) = window.raw_window_handle() else {
            unreachable!()
        };
        let hwnd_video = F::HWND(handle.hwnd as isize);

        let event_loop = EventLoopWrapper(Box::new(event_loop));

        Ok(Player {
            inner: PlayerInner::new(hwnd_video, event_loop)?,
            _marker: PhantomData,
        })
    }
}

impl<E> Player<E> {
    #[inline]
    fn inner(&self) -> &PlayerInner {
        self.inner.as_impl()
    }

    #[inline]
    pub fn open<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let path = path.as_ref();
        self.inner().open(path)?;
        Ok(())
    }

    #[inline]
    pub fn selected_service(&self) -> Option<isdb::filters::sorter::Service> {
        self.inner().selected_service()
    }

    #[inline]
    pub fn active_video_tag(&self) -> Option<u8> {
        self.inner().active_video_tag()
    }

    #[inline]
    pub fn active_audio_tag(&self) -> Option<u8> {
        self.inner().active_audio_tag()
    }

    #[inline]
    pub fn services(&self) -> isdb::filters::sorter::ServiceMap {
        self.inner().services()
    }

    #[inline]
    pub fn select_service(&self, service_id: Option<ServiceId>) -> Result<()> {
        self.inner().select_service(service_id)?;
        Ok(())
    }

    #[inline]
    pub fn select_video_stream(&self, component_tag: u8) -> Result<()> {
        self.inner().select_video_stream(component_tag)?;
        Ok(())
    }

    #[inline]
    pub fn select_audio_stream(&self, component_tag: u8) -> Result<()> {
        self.inner().select_audio_stream(component_tag)?;
        Ok(())
    }

    #[inline]
    pub fn handle_event(&self, event: PlayerEvent) -> Result<()> {
        self.inner().handle_event(event)?;
        Ok(())
    }

    #[inline]
    pub fn play(&self) -> Result<()> {
        self.inner().play()?;
        Ok(())
    }

    #[inline]
    pub fn pause(&self) -> Result<()> {
        self.inner().pause()?;
        Ok(())
    }

    #[inline]
    pub fn play_or_pause(&self) -> Result<()> {
        self.inner().play_or_pause()?;
        Ok(())
    }

    #[inline]
    pub fn repaint(&self) -> Result<()> {
        self.inner().repaint()?;
        Ok(())
    }

    #[inline]
    pub fn resize_video(&self, width: u32, height: u32) -> Result<()> {
        self.inner().resize_video(width, height)?;
        Ok(())
    }

    #[inline]
    pub fn position(&self) -> Result<Duration> {
        let pos = self.inner().position()?;
        Ok(pos)
    }

    #[inline]
    pub fn set_position(&self, pos: Duration) -> Result<()> {
        self.inner().set_position(pos)?;
        Ok(())
    }

    #[inline]
    pub fn volume(&self) -> Result<f32> {
        let value = self.inner().volume()?;
        Ok(value)
    }

    #[inline]
    pub fn set_volume(&self, value: f32) -> Result<()> {
        self.inner().set_volume(value)?;
        Ok(())
    }

    #[inline]
    pub fn rate(&self) -> Result<f32> {
        let value = self.inner().rate()?;
        Ok(value)
    }

    #[inline]
    pub fn set_rate(&self, value: f32) -> Result<()> {
        self.inner().set_rate(value)?;
        Ok(())
    }

    #[inline]
    pub fn shutdown(&self) -> Result<()> {
        self.inner().shutdown()?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Closed,
    Ready,
    OpenPending,
    Started,
    Paused,
    Stopped,
    Closing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Command {
    Stop,
    Start,
    Pause,
}

#[derive(Debug)]
struct OpRequest {
    command: Cell<Option<Command>>,
    rate: Cell<Option<f32>>,
    pos: Cell<Option<Duration>>,
}

#[implement(MF::IMFAsyncCallback)]
struct PlayerInner {
    // セッションが閉じられたらロック解除されるミューテックス
    close_mutex: parking_lot::RawMutex,

    session: RefCell<Option<MF::IMFMediaSession>>,
    source: RefCell<Option<MF::IMFMediaSource>>,
    presentation_clock: RefCell<Option<MF::IMFPresentationClock>>,
    video_display: RefCell<Option<MF::IMFVideoDisplayControl>>,
    audio_volume: RefCell<Option<MF::IMFAudioStreamVolume>>,
    rate_control: RefCell<Option<MF::IMFRateControl>>,

    state: Cell<State>,
    /// 現在の再生速度
    current_rate: Cell<f32>,
    /// シーク待ち中のシーク位置
    seeking_pos: Cell<Option<Duration>>,
    /// 再生・停止や速度変更等の処理待ち
    is_pending: Cell<bool>,
    /// 処理待ち中に受け付けた操作要求
    op_request: OpRequest,

    hwnd_video: F::HWND,
    event_loop: EventLoopWrapper,
}

impl PlayerInner {
    pub fn new(
        hwnd_video: F::HWND,
        event_loop: EventLoopWrapper,
    ) -> WinResult<MF::IMFAsyncCallback> {
        unsafe {
            MF::MFStartup(
                (MF::MF_SDK_VERSION << 16) | MF::MF_API_VERSION,
                MF::MFSTARTUP_NOSOCKET,
            )?;

            let close_mutex = parking_lot::RawMutex::INIT;
            close_mutex.lock();
            Ok(PlayerInner {
                close_mutex,

                session: RefCell::new(None),
                source: RefCell::new(None),
                presentation_clock: RefCell::new(None),
                video_display: RefCell::new(None),
                audio_volume: RefCell::new(None),
                rate_control: RefCell::new(None),

                state: Cell::new(State::Closed),
                current_rate: Cell::new(1.),
                seeking_pos: Cell::new(None),
                is_pending: Cell::new(false),
                op_request: OpRequest {
                    command: Cell::new(None),
                    rate: Cell::new(None),
                    pos: Cell::new(None),
                },

                hwnd_video,
                event_loop,
            }
            .into())
        }
    }

    fn intf(&self) -> MF::IMFAsyncCallback {
        unsafe { self.cast().unwrap() }
    }

    fn create_session(&self) -> WinResult<()> {
        unsafe {
            self.close_session()?;

            debug_assert_eq!(self.state.get(), State::Closed);

            let mut session = self.session.borrow_mut();
            let session = session.insert(MF::MFCreateMediaSession(None)?);
            session.BeginGetEvent(&self.intf(), None)?;

            *self.presentation_clock.borrow_mut() = Some(session.GetClock()?.cast()?);

            self.state.set(State::Ready);

            Ok(())
        }
    }

    fn close_session(&self) -> WinResult<()> {
        unsafe {
            let mut r = Ok(());

            self.presentation_clock.take();
            self.video_display.take();
            self.audio_volume.take();
            self.rate_control.take();

            if let Some(session) = self.session.borrow().as_ref() {
                self.state.set(State::Closing);
                r = session.Close();
                if r.is_ok() {
                    let wait_result = self.close_mutex.try_lock_for(Duration::from_secs(5));
                    if !wait_result {
                        log::trace!("close_session timed out");
                    }
                }
            }

            if r.is_ok() {
                if let Some(source) = self.source.borrow().as_ref() {
                    let _ = source.Shutdown();
                }
                if let Some(session) = self.session.borrow().as_ref() {
                    let _ = session.Shutdown();
                }
            }

            self.source.take();
            self.session.take();
            self.state.set(State::Closed);

            r
        }
    }

    fn shutdown(&self) -> WinResult<()> {
        unsafe {
            let r = self.close_session();
            let _ = MF::MFShutdown();
            self.close_mutex.unlock();

            r
        }
    }

    fn create_media_sink_activate(
        source_sd: &MF::IMFStreamDescriptor,
        hwnd_video: F::HWND,
    ) -> WinResult<MF::IMFActivate> {
        unsafe {
            let handler = source_sd.GetMediaTypeHandler()?;
            let major_type = handler.GetMajorType()?;

            if log::log_enabled!(log::Level::Debug) {
                let media_type = handler.GetCurrentMediaType()?;
                log::debug!(
                    "codec: {}",
                    match media_type.GetGUID(&MF::MF_MT_SUBTYPE)? {
                        MF::MFVideoFormat_MPEG2 => "MPEG-2",
                        MF::MFVideoFormat_H264 => "H.264",
                        MF::MFVideoFormat_H265 => "H.265",
                        MF::MFAudioFormat_MPEG => "MPEG Audio",
                        MF::MFAudioFormat_AAC => "AAC",
                        MF::MFAudioFormat_Dolby_AC3 => "AC3",
                        _ => "Unknown",
                    }
                );
                if let Ok(size) = media_type.GetUINT64(&MF::MF_MT_FRAME_SIZE) {
                    log::debug!("size: {}x{}", (size >> 32) as u32, size as u32);
                }
                if let Ok(ratio) = media_type.GetUINT64(&MF::MF_MT_PIXEL_ASPECT_RATIO) {
                    log::debug!("ratio: {}/{}", (ratio >> 32) as u32, ratio as u32);
                }
            }

            let activate = match major_type {
                MF::MFMediaType_Audio => MF::MFCreateAudioRendererActivate()?,
                MF::MFMediaType_Video => MF::MFCreateVideoRendererActivate(hwnd_video)?,
                _ => return Err(F::E_FAIL.into()),
            };

            Ok(activate)
        }
    }

    fn add_source_node(
        topology: &MF::IMFTopology,
        source: &MF::IMFMediaSource,
        pd: &MF::IMFPresentationDescriptor,
        sd: &MF::IMFStreamDescriptor,
    ) -> WinResult<MF::IMFTopologyNode> {
        unsafe {
            let node = MF::MFCreateTopologyNode(MF::MF_TOPOLOGY_SOURCESTREAM_NODE)?;
            node.SetUnknown(&MF::MF_TOPONODE_SOURCE, source)?;
            node.SetUnknown(&MF::MF_TOPONODE_PRESENTATION_DESCRIPTOR, pd)?;
            node.SetUnknown(&MF::MF_TOPONODE_STREAM_DESCRIPTOR, sd)?;
            topology.AddNode(&node)?;

            Ok(node)
        }
    }

    fn add_output_node(
        topology: &MF::IMFTopology,
        activate: &MF::IMFActivate,
        id: u32,
    ) -> WinResult<MF::IMFTopologyNode> {
        unsafe {
            let node = MF::MFCreateTopologyNode(MF::MF_TOPOLOGY_OUTPUT_NODE)?;
            node.SetObject(activate)?;
            node.SetUINT32(&MF::MF_TOPONODE_STREAMID, id)?;
            node.SetUINT32(&MF::MF_TOPONODE_NOSHUTDOWN_ON_REMOVE, F::FALSE.0 as u32)?;
            topology.AddNode(&node)?;

            Ok(node)
        }
    }

    fn add_branch_to_partial_topology(
        topology: &MF::IMFTopology,
        source: &MF::IMFMediaSource,
        pd: &MF::IMFPresentationDescriptor,
        i: u32,
        hwnd_video: F::HWND,
    ) -> WinResult<()> {
        unsafe {
            let (selected, sd) = get_stream_descriptor_by_index(pd, i)?;

            if selected {
                let sink_activate = Self::create_media_sink_activate(&sd, hwnd_video)?;
                let source_node = Self::add_source_node(topology, source, pd, &sd)?;
                let output_node = Self::add_output_node(topology, &sink_activate, 0)?;
                source_node.ConnectOutput(0, &output_node, 0)?;
            }

            Ok(())
        }
    }

    fn create_playback_topology(
        source: &MF::IMFMediaSource,
        pd: &MF::IMFPresentationDescriptor,
        hwnd_video: F::HWND,
    ) -> WinResult<MF::IMFTopology> {
        unsafe {
            let topology = MF::MFCreateTopology()?;

            let c_source_streams = pd.GetStreamDescriptorCount()?;
            for i in 0..c_source_streams {
                Self::add_branch_to_partial_topology(&topology, source, pd, i, hwnd_video)?;
            }

            Ok(topology)
        }
    }

    pub fn open(&self, path: &Path) -> WinResult<()> {
        fn open(this: &PlayerInner, path: &Path) -> WinResult<()> {
            unsafe {
                this.create_session()?;

                let mut source = this.source.borrow_mut();
                let source = source.insert(TransportStream::new(path)?);
                let source_pd = source.CreatePresentationDescriptor()?;

                let topology =
                    PlayerInner::create_playback_topology(source, &source_pd, this.hwnd_video)?;
                this.session
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .SetTopology(0, &topology)?;

                Ok(())
            }
        }

        let r = open(self, path);

        self.state.set(if r.is_ok() {
            State::OpenPending
        } else {
            State::Closed
        });

        r
    }

    fn run_pending_ops(&self, new_state: State) -> WinResult<()> {
        if self.state.get() == new_state && self.is_pending.take() {
            match self.op_request.command.take() {
                None => {}
                Some(Command::Start) => self.start_playback()?,
                Some(Command::Pause) => self.pause()?,
                Some(Command::Stop) => self.stop()?,
            }

            if let Some(rate) = self.op_request.rate.take() {
                if rate != self.current_rate.get() {
                    self.set_rate(rate)?;
                }
            }

            self.seeking_pos.take();
            if let Some(pos) = self.op_request.pos.take() {
                self.set_position_internal(pos)?;
            }
        }

        Ok(())
    }

    pub fn handle_event(&self, event: MF::IMFMediaEvent) -> WinResult<()> {
        unsafe {
            let me_type = event.GetType()?;
            let status = event.GetStatus()?;

            match MF::MF_EVENT_TYPE(me_type as i32) {
                MF::MESessionStarted => self.on_session_started(status, &event)?,
                MF::MESessionPaused => self.on_session_paused(status, &event)?,
                MF::MESessionRateChanged => self.on_session_rate_changed(status, &event)?,

                MF::MESessionTopologyStatus => self.on_topology_status(status, &event)?,
                MF::MEEndOfPresentation => self.on_presentation_ended(status, &event)?,
                MF::MENewPresentation => self.on_new_presentation(status, &event)?,

                me => {
                    log::debug!("media event: {:?}", me);
                    status.ok()?;
                }
            }

            Ok(())
        }
    }

    fn on_session_started(&self, status: C::HRESULT, _event: &MF::IMFMediaEvent) -> WinResult<()> {
        log::debug!("on_session_started");
        status.ok()?;

        self.run_pending_ops(State::Started)?;
        Ok(())
    }

    fn on_session_paused(&self, status: C::HRESULT, _event: &MF::IMFMediaEvent) -> WinResult<()> {
        log::debug!("on_session_paused");
        status.ok()?;

        self.run_pending_ops(State::Paused)?;
        Ok(())
    }

    fn on_session_rate_changed(
        &self,
        status: C::HRESULT,
        event: &MF::IMFMediaEvent,
    ) -> WinResult<()> {
        unsafe {
            log::debug!("on_session_rate_changed");

            // 速度変更が成功した場合は既に速度をキャッシュ済み
            // 失敗した場合は実際の速度に更新
            if status.is_err() {
                if let Ok(PropVariant::F32(rate)) = event.GetValue()?.try_into() {
                    self.current_rate.set(rate);
                }
            }

            Ok(())
        }
    }

    fn on_topology_status(&self, status: C::HRESULT, event: &MF::IMFMediaEvent) -> WinResult<()> {
        unsafe fn get_service<T: Interface>(
            session: &MF::IMFMediaSession,
            guid: &C::GUID,
        ) -> WinResult<T> {
            let mut ptr = std::ptr::null_mut();
            MF::MFGetService(session, guid, &T::IID, &mut ptr)?;
            debug_assert!(!ptr.is_null());
            Ok(T::from_raw(ptr))
        }

        unsafe {
            status.ok()?;

            let status = event.GetUINT32(&MF::MF_EVENT_TOPOLOGY_STATUS)?;
            if status == MF::MF_TOPOSTATUS_READY.0 as u32 {
                log::debug!("on_topology_ready");

                let session = self.session.borrow();
                let Some(session) = session.as_ref() else {
                    return Err(MF::MF_E_INVALIDREQUEST.into());
                };

                *self.video_display.borrow_mut() =
                    get_service(session, &MF::MR_VIDEO_RENDER_SERVICE).ok();
                *self.audio_volume.borrow_mut() =
                    get_service(session, &MF::MR_STREAM_VOLUME_SERVICE).ok();
                *self.rate_control.borrow_mut() =
                    get_service(session, &MF::MF_RATE_CONTROL_SERVICE).ok();

                self.start_playback()?;
            }
            Ok(())
        }
    }

    fn on_presentation_ended(&self, status: C::HRESULT, _: &MF::IMFMediaEvent) -> WinResult<()> {
        log::debug!("on_presentation_ended");
        status.ok()?;

        self.state.set(State::Stopped);
        Ok(())
    }

    fn on_new_presentation(&self, status: C::HRESULT, event: &MF::IMFMediaEvent) -> WinResult<()> {
        unsafe fn get_event_object<T: C::Interface>(event: &MF::IMFMediaEvent) -> WinResult<T> {
            let Ok(PropVariant::IUnknown(unk)) = PropVariant::try_from(event.GetValue()?) else {
                return Err(MF::MF_E_INVALIDTYPE.into());
            };

            unk.cast()
        }

        unsafe {
            log::debug!("on_new_presentation");
            status.ok()?;

            let session = self.session.borrow();
            let session = session.as_ref().expect("session closed");

            let source = self.source.borrow();
            let source = source.as_ref().expect("session closed");

            let pd = get_event_object(event)?;
            let topology = Self::create_playback_topology(source, &pd, self.hwnd_video)?;
            session.SetTopology(0, &topology)?;

            self.state.set(State::OpenPending);

            Ok(())
        }
    }

    fn start_playback(&self) -> WinResult<()> {
        unsafe {
            log::debug!("start_playback");

            let session = self.session.borrow();
            let session = session.as_ref().expect("session closed");

            session.Start(&GUID_NULL, &Default::default())?;

            self.state.set(State::Started);
            self.is_pending.set(true);

            Ok(())
        }
    }

    pub fn play(&self) -> WinResult<()> {
        if !matches!(self.state.get(), State::Paused | State::Stopped) {
            return Err(MF::MF_E_INVALIDREQUEST.into());
        }
        if self.session.borrow().is_none() || self.source.borrow().is_none() {
            return Err(F::E_UNEXPECTED.into());
        }

        if self.is_pending.get() {
            self.op_request.command.set(Some(Command::Start));
        } else {
            self.start_playback()?;
        }

        Ok(())
    }

    pub fn pause(&self) -> WinResult<()> {
        unsafe {
            if self.state.get() != State::Started {
                return Err(MF::MF_E_INVALIDREQUEST.into());
            }

            let session = self.session.borrow();
            let Some(session) = session.as_ref() else {
                return Err(F::E_UNEXPECTED.into());
            };

            if self.is_pending.get() {
                self.op_request.command.set(Some(Command::Pause));
            } else {
                session.Pause()?;

                self.state.set(State::Paused);
                self.is_pending.set(true);
            }

            Ok(())
        }
    }

    pub fn stop(&self) -> WinResult<()> {
        unsafe {
            if matches!(self.state.get(), State::Started | State::Paused) {
                return Err(MF::MF_E_INVALIDREQUEST.into());
            }

            let session = self.session.borrow();
            let Some(session) = session.as_ref() else {
                return Err(F::E_UNEXPECTED.into());
            };

            if self.is_pending.get() {
                self.op_request.command.set(Some(Command::Stop));
            } else {
                session.Stop()?;

                self.state.set(State::Stopped);
                self.is_pending.set(true);
            }

            Ok(())
        }
    }

    pub fn play_or_pause(&self) -> WinResult<()> {
        match self.state.get() {
            State::Started => self.pause()?,
            State::Paused => self.play()?,
            _ => return Err(MF::MF_E_INVALIDREQUEST.into()),
        }

        Ok(())
    }

    pub fn repaint(&self) -> WinResult<()> {
        unsafe {
            if let Some(video_display) = &*self.video_display.borrow() {
                video_display.RepaintVideo()
            } else {
                Ok(())
            }
        }
    }

    pub fn resize_video(&self, width: u32, height: u32) -> WinResult<()> {
        unsafe {
            if let Some(video_display) = &*self.video_display.borrow() {
                let mut size = F::SIZE { cx: 0, cy: 0 };
                video_display.GetNativeVideoSize(&mut size, std::ptr::null_mut())?;

                let src = MF::MFVideoNormalizedRect {
                    left: 0.,
                    top: 0.,
                    right: 1.,
                    bottom: if size.cy == 1088 { 1080. / 1088. } else { 1. },
                };
                let dst = F::RECT {
                    left: 0,
                    top: 0,
                    right: width as i32,
                    bottom: height as i32,
                };
                video_display.SetVideoPosition(&src, &dst)?;
            }

            Ok(())
        }
    }

    pub fn position(&self) -> WinResult<Duration> {
        unsafe {
            let presentation_clock = self.presentation_clock.borrow();
            let Some(presentation_clock) = presentation_clock.as_ref() else {
                return Err(MF::MF_E_NO_CLOCK.into());
            };

            let pos =
                if let Some(pos) = self.op_request.pos.get().or_else(|| self.seeking_pos.get()) {
                    pos
                } else {
                    Duration::from_nanos(presentation_clock.GetTime()? as u64 * 100)
                };

            Ok(pos)
        }
    }

    fn set_position_internal(&self, pos: Duration) -> WinResult<()> {
        unsafe {
            let session = self.session.borrow();
            let Some(session) = session.as_ref() else {
                return Err(MF::MF_E_INVALIDREQUEST.into());
            };

            let time = (pos.as_nanos() / 100) as i64;
            session.Start(&GUID_NULL, &PropVariant::I64(time).to_raw())?;
            if self.state.get() == State::Paused {
                session.Pause()?
            }

            self.seeking_pos.set(Some(pos));
            self.is_pending.set(true);

            Ok(())
        }
    }

    pub fn set_position(&self, pos: Duration) -> WinResult<()> {
        if self.is_pending.get() {
            self.op_request.pos.set(Some(pos));
        } else {
            self.set_position_internal(pos)?;
        }

        Ok(())
    }

    pub fn volume(&self) -> WinResult<f32> {
        unsafe {
            let audio_volume = self.audio_volume.borrow();
            let Some(audio_volume) = audio_volume.as_ref() else {
                return Err(MF::MF_E_INVALIDREQUEST.into());
            };

            if audio_volume.GetChannelCount()? < 1 {
                return Err(MF::MF_E_INVALIDREQUEST.into());
            }

            let value = audio_volume.GetChannelVolume(0)?;
            Ok(value)
        }
    }

    pub fn set_volume(&self, value: f32) -> WinResult<()> {
        unsafe {
            let audio_volume = self.audio_volume.borrow();
            let Some(audio_volume) = audio_volume.as_ref() else {
                return Err(MF::MF_E_INVALIDREQUEST.into());
            };

            let nch = audio_volume.GetChannelCount()? as usize;
            audio_volume.SetAllVolumes(&*vec![value; nch])?;
            Ok(())
        }
    }

    pub fn rate(&self) -> WinResult<f32> {
        unsafe {
            let rate_control = self.rate_control.borrow();
            let Some(rate_control) = rate_control.as_ref() else {
                return Err(MF::MF_E_INVALIDREQUEST.into());
            };

            let mut rate = 0.;
            rate_control.GetRate(std::ptr::null_mut(), &mut rate)?;
            Ok(rate)
        }
    }

    pub fn set_rate(&self, value: f32) -> WinResult<()> {
        unsafe {
            let rate_control = self.rate_control.borrow();
            let Some(rate_control) = rate_control.as_ref() else {
                return Err(MF::MF_E_INVALIDREQUEST.into());
            };

            if self.is_pending.get() {
                self.op_request.rate.set(Some(value));
            } else {
                rate_control.SetRate(F::FALSE, value)?;
                self.current_rate.set(value);
            }

            Ok(())
        }
    }

    pub fn selected_service(&self) -> Option<isdb::filters::sorter::Service> {
        self.source
            .borrow()
            .as_ref()
            .and_then(|source| source.as_impl().selected_service())
    }

    pub fn active_video_tag(&self) -> Option<u8> {
        self.source
            .borrow()
            .as_ref()
            .and_then(|source| source.as_impl().active_video_tag())
    }

    pub fn active_audio_tag(&self) -> Option<u8> {
        self.source
            .borrow()
            .as_ref()
            .and_then(|source| source.as_impl().active_audio_tag())
    }

    pub fn services(&self) -> isdb::filters::sorter::ServiceMap {
        let source = self.source.borrow();
        let Some(source) = source.as_ref() else {
            return Default::default();
        };
        source.as_impl().services()
    }

    pub fn select_service(&self, service_id: Option<ServiceId>) -> WinResult<()> {
        let source = self.source.borrow();
        let Some(source) = source.as_ref() else {
            return Err(MF::MF_E_INVALIDREQUEST.into());
        };

        source.as_impl().select_service(service_id)?;
        Ok(())
    }

    pub fn select_video_stream(&self, component_tag: u8) -> WinResult<()> {
        let source = self.source.borrow();
        let Some(source) = source.as_ref() else {
            return Err(MF::MF_E_INVALIDREQUEST.into());
        };

        source.as_impl().select_video_stream(component_tag)?;
        Ok(())
    }

    pub fn select_audio_stream(&self, component_tag: u8) -> WinResult<()> {
        let source = self.source.borrow();
        let Some(source) = source.as_ref() else {
            return Err(MF::MF_E_INVALIDREQUEST.into());
        };

        source.as_impl().select_audio_stream(component_tag)?;
        Ok(())
    }
}

#[allow(non_snake_case)]
impl MF::IMFAsyncCallback_Impl for PlayerInner {
    fn GetParameters(&self, _: *mut u32, _: *mut u32) -> WinResult<()> {
        log::trace!("Player::GetParameters");
        Err(F::E_NOTIMPL.into())
    }

    fn Invoke(&self, presult: &Option<MF::IMFAsyncResult>) -> WinResult<()> {
        log::trace!("Player::Invoke");
        unsafe {
            let session = self.session.borrow();
            let session = session.as_ref().expect("session closed");

            let event = session.EndGetEvent(presult.as_ref())?;
            let me_type = event.GetType()?;
            if me_type == MF::MESessionClosed.0 as u32 {
                self.close_mutex.unlock();
            } else {
                session.BeginGetEvent(&self.intf(), None)?;
            }

            if self.state.get() != State::Closing {
                let _ = self.event_loop.0.send_player_event(event);
            }

            Ok(())
        }
    }
}
