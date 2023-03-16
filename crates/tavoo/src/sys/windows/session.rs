use std::cell::{Cell, RefCell};
use std::time::Duration;

use parking_lot::lock_api::{RawMutex, RawMutexTimed};
use windows::core::{self as C, implement, AsImpl, Interface};
use windows::Win32::Foundation as F;
use windows::Win32::Media::KernelStreaming::GUID_NULL;
use windows::Win32::Media::MediaFoundation as MF;

use crate::extract::ExtractHandler;

use super::utils::{get_stream_descriptor_by_index, PropVariant, WinResult};

// ジェネリクスと#[implement]を併用できないので型消去
pub struct EventLoopWrapper(Box<dyn EventLoop>);

impl EventLoopWrapper {
    #[inline]
    pub fn new<E: EventLoop + 'static>(event_loop: E) -> EventLoopWrapper {
        EventLoopWrapper(Box::new(event_loop))
    }
}

pub trait EventLoop {
    fn send_player_event(
        &self,
        event: C::AgileReference<MF::IMFMediaEvent>,
    ) -> Result<(), winit::event_loop::EventLoopClosed<()>>;
}

impl<E: From<crate::player::PlayerEvent>> EventLoop for winit::event_loop::EventLoopProxy<E> {
    fn send_player_event(
        &self,
        event: C::AgileReference<MF::IMFMediaEvent>,
    ) -> Result<(), winit::event_loop::EventLoopClosed<()>> {
        match self.send_event(crate::player::PlayerEvent(event).into()) {
            Ok(()) => Ok(()),
            Err(_) => Err(winit::event_loop::EventLoopClosed(())),
        }
    }
}

/// IMFMediaSessionのラッパー。
#[derive(Debug, Clone)]
pub struct Session {
    agile: C::AgileReference<MF::IMFAsyncCallback>,
}

impl Session {
    #[inline]
    pub fn new(
        hwnd_video: F::HWND,
        event_loop: EventLoopWrapper,
        handler: ExtractHandler,
        source: MF::IMFMediaSource,
        volume: Option<f32>,
        rate: Option<f32>,
    ) -> WinResult<Session> {
        Ok(Session {
            agile: C::AgileReference::new(&Inner::new(
                hwnd_video, event_loop, handler, source, volume, rate,
            )?)?,
        })
    }
}

impl Session {
    #[inline]
    fn with_inner<T, F: FnOnce(&Inner) -> T>(&self, f: F) -> T {
        let callback = self.agile.resolve().unwrap();
        f(callback.as_impl())
    }

    #[inline]
    pub fn source(&self) -> MF::IMFMediaSource {
        self.with_inner(|inner| inner.source.clone())
    }

    #[inline]
    pub fn close(&self) -> WinResult<()> {
        self.with_inner(|inner| inner.close())
    }

    #[inline]
    pub fn handle_event(&self, event: C::AgileReference<MF::IMFMediaEvent>) -> WinResult<()> {
        self.with_inner(|inner| inner.handle_event(event))
    }

    #[inline]
    pub fn play(&self) -> WinResult<()> {
        self.with_inner(|inner| inner.play())
    }

    #[inline]
    pub fn pause(&self) -> WinResult<()> {
        self.with_inner(|inner| inner.pause())
    }

    #[inline]
    pub fn play_or_pause(&self) -> WinResult<()> {
        self.with_inner(|inner| inner.play_or_pause())
    }

    #[inline]
    pub fn stop(&self) -> WinResult<()> {
        self.with_inner(|inner| inner.stop())
    }

    #[inline]
    pub fn repaint(&self) -> WinResult<()> {
        self.with_inner(|inner| inner.repaint())
    }

    #[inline]
    pub fn resize_video(&self, width: u32, height: u32) -> WinResult<()> {
        self.with_inner(|inner| inner.resize_video(width, height))
    }

    #[inline]
    pub fn position(&self) -> WinResult<Duration> {
        self.with_inner(|inner| inner.position())
    }

    #[inline]
    pub fn set_position(&self, pos: Duration) -> WinResult<()> {
        self.with_inner(|inner| inner.set_position(pos))
    }

    #[inline]
    pub fn volume(&self) -> WinResult<f32> {
        self.with_inner(|inner| inner.volume())
    }

    #[inline]
    pub fn set_volume(&self, value: f32) -> WinResult<()> {
        self.with_inner(|inner| inner.set_volume(value))
    }

    #[inline]
    pub fn rate(&self) -> WinResult<f32> {
        self.with_inner(|inner| inner.rate())
    }

    #[inline]
    pub fn set_rate(&self, value: f32) -> WinResult<()> {
        self.with_inner(|inner| inner.set_rate(value))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
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
struct Inner {
    // セッションが閉じられたらロック解除されるミューテックス
    close_mutex: parking_lot::RawMutex,
    handler: ExtractHandler,

    session: MF::IMFMediaSession,
    source: MF::IMFMediaSource,
    presentation_clock: MF::IMFPresentationClock,
    video_display: RefCell<Option<MF::IMFVideoDisplayControl>>,
    audio_volume: RefCell<Option<MF::IMFAudioStreamVolume>>,
    rate_control: RefCell<Option<MF::IMFRateControl>>,

    state: Cell<State>,
    /// 再生開始時の音量。
    initial_volume: Option<f32>,
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

impl Inner {
    pub fn new(
        hwnd_video: F::HWND,
        event_loop: EventLoopWrapper,
        handler: ExtractHandler,
        source: MF::IMFMediaSource,
        initial_volume: Option<f32>,
        initial_rate: Option<f32>,
    ) -> WinResult<MF::IMFAsyncCallback> {
        unsafe {
            let close_mutex = parking_lot::RawMutex::INIT;
            close_mutex.lock();

            let session = MF::MFCreateMediaSession(None)?;
            let presentation_clock = session.GetClock()?.cast()?;

            let source_pd = source.CreatePresentationDescriptor()?;

            let topology = Self::create_playback_topology(&source, &source_pd, hwnd_video)?;
            session.SetTopology(0, &topology)?;

            let callback: MF::IMFAsyncCallback = Inner {
                close_mutex,
                handler,

                session,
                source,
                presentation_clock,
                video_display: RefCell::new(None),
                audio_volume: RefCell::new(None),
                rate_control: RefCell::new(None),

                state: Cell::new(State::Ready),
                initial_volume,
                current_rate: Cell::new(initial_rate.unwrap_or(1.)),
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
            .into();
            let this: &Inner = callback.as_impl();
            this.session.BeginGetEvent(&callback, None)?;

            Ok(callback)
        }
    }

    fn intf(&self) -> MF::IMFAsyncCallback {
        unsafe { self.cast().unwrap() }
    }

    fn close(&self) -> WinResult<()> {
        fn shutdown(this: &Inner) -> WinResult<()> {
            unsafe {
                this.state.set(State::Closing);
                this.session.Close()?;
                let wait_result = this.close_mutex.try_lock_for(Duration::from_secs(5));
                if !wait_result {
                    log::trace!("Session::shutdown timed out");
                }

                let _ = this.source.Shutdown();
                let _ = this.session.Shutdown();

                Ok(())
            }
        }

        self.video_display.take();
        self.audio_volume.take();
        self.rate_control.take();

        let r = shutdown(self);

        unsafe { self.close_mutex.unlock() }

        r
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

    pub fn handle_event(&self, event: C::AgileReference<MF::IMFMediaEvent>) -> WinResult<()> {
        unsafe {
            let event = event.resolve()?;
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
                    log::trace!("media event: {:?}", me);
                    status.ok()?;
                }
            }

            Ok(())
        }
    }

    fn on_session_started(&self, status: C::HRESULT, _event: &MF::IMFMediaEvent) -> WinResult<()> {
        log::debug!("Session::on_session_started");
        status.ok()?;

        self.run_pending_ops(State::Started)?;
        Ok(())
    }

    fn on_session_paused(&self, status: C::HRESULT, _event: &MF::IMFMediaEvent) -> WinResult<()> {
        log::debug!("Session::on_session_paused");
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
            log::debug!("Session::on_session_rate_changed");

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

    unsafe fn get_service<T: Interface>(&self, guid: &C::GUID) -> WinResult<T> {
        let mut ptr = std::ptr::null_mut();
        MF::MFGetService(&self.session, guid, &T::IID, &mut ptr)?;
        debug_assert!(!ptr.is_null());
        Ok(T::from_raw(ptr))
    }

    fn on_topology_status(&self, status: C::HRESULT, event: &MF::IMFMediaEvent) -> WinResult<()> {
        unsafe {
            status.ok()?;

            let status = event.GetUINT32(&MF::MF_EVENT_TOPOLOGY_STATUS)?;
            if status == MF::MF_TOPOSTATUS_READY.0 as u32 {
                log::debug!("Session::on_topology_ready");

                *self.video_display.borrow_mut() =
                    self.get_service(&MF::MR_VIDEO_RENDER_SERVICE).ok();
                *self.audio_volume.borrow_mut() =
                    self.get_service(&MF::MR_STREAM_VOLUME_SERVICE).ok();
                *self.rate_control.borrow_mut() =
                    self.get_service(&MF::MF_RATE_CONTROL_SERVICE).ok();

                if let Some(volume) = self.initial_volume {
                    if let Some(audio_volume) = self.audio_volume.borrow_mut().as_ref() {
                        if let Ok(nch) = audio_volume.GetChannelCount() {
                            let _ = audio_volume.SetAllVolumes(&*vec![volume; nch as usize]);
                        }
                    }
                }
                if let Some(rate_control) = self.rate_control.borrow_mut().as_ref() {
                    let _ = rate_control.SetRate(F::FALSE, self.current_rate.get());
                }

                self.start_playback()?;
            }
            Ok(())
        }
    }

    fn on_presentation_ended(&self, status: C::HRESULT, _: &MF::IMFMediaEvent) -> WinResult<()> {
        log::debug!("Session::on_presentation_ended");
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
            log::debug!("Session::on_new_presentation");
            status.ok()?;

            let pd = get_event_object(event)?;
            let topology = Self::create_playback_topology(&self.source, &pd, self.hwnd_video)?;
            self.session.SetTopology(0, &topology)?;

            self.state.set(State::OpenPending);

            Ok(())
        }
    }

    fn start_playback(&self) -> WinResult<()> {
        unsafe {
            log::debug!("Session::start_playback");

            self.session.Start(&GUID_NULL, &Default::default())?;

            self.state.set(State::Started);
            self.is_pending.set(true);

            Ok(())
        }
    }

    pub fn play(&self) -> WinResult<()> {
        if !matches!(self.state.get(), State::Paused | State::Stopped) {
            return Err(MF::MF_E_INVALIDREQUEST.into());
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

            if self.is_pending.get() {
                self.op_request.command.set(Some(Command::Pause));
            } else {
                self.session.Pause()?;

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

            if self.is_pending.get() {
                self.op_request.command.set(Some(Command::Stop));
            } else {
                self.session.Stop()?;
                self.handler.reset();

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
            let pos =
                if let Some(pos) = self.op_request.pos.get().or_else(|| self.seeking_pos.get()) {
                    pos
                } else {
                    Duration::from_nanos(self.presentation_clock.GetTime()? as u64 * 100)
                };

            Ok(pos)
        }
    }

    fn set_position_internal(&self, pos: Duration) -> WinResult<()> {
        unsafe {
            let time = (pos.as_nanos() / 100) as i64;
            self.session
                .Start(&GUID_NULL, &PropVariant::I64(time).to_raw())?;
            if self.state.get() == State::Paused {
                self.session.Pause()?
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
}

#[allow(non_snake_case)]
impl MF::IMFAsyncCallback_Impl for Inner {
    fn GetParameters(&self, _: *mut u32, _: *mut u32) -> WinResult<()> {
        log::trace!("Session::GetParameters");
        Err(F::E_NOTIMPL.into())
    }

    fn Invoke(&self, presult: &Option<MF::IMFAsyncResult>) -> WinResult<()> {
        log::trace!("Session::Invoke");
        unsafe {
            let event = self.session.EndGetEvent(presult.as_ref())?;
            let me_type = event.GetType()?;
            if me_type == MF::MESessionClosed.0 as u32 {
                self.close_mutex.unlock();
            } else {
                self.session.BeginGetEvent(&self.intf(), None)?;
            }

            if self.state.get() != State::Closing {
                let _ = self
                    .event_loop
                    .0
                    .send_player_event(C::AgileReference::new(&event)?);
            }

            Ok(())
        }
    }
}
