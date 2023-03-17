use std::sync::Arc;
use std::time::Duration;

use parking_lot::lock_api::{RawMutex, RawMutexTimed};
use parking_lot::{Mutex, MutexGuard};
use windows::core::{self as C, implement, AsImpl, Interface};
use windows::Win32::Foundation as F;
use windows::Win32::Media::KernelStreaming::GUID_NULL;
use windows::Win32::Media::MediaFoundation as MF;

use crate::extract::ExtractHandler;

use super::source::TransportStream;
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
        event: MF::IMFMediaEvent,
    ) -> Result<(), winit::event_loop::EventLoopClosed<()>>;
}

impl<E: From<crate::player::PlayerEvent>> EventLoop for winit::event_loop::EventLoopProxy<E> {
    fn send_player_event(
        &self,
        event: MF::IMFMediaEvent,
    ) -> Result<(), winit::event_loop::EventLoopClosed<()>> {
        match self.send_event(crate::player::PlayerEvent(event.into()).into()) {
            Ok(()) => Ok(()),
            Err(_) => Err(winit::event_loop::EventLoopClosed(())),
        }
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
            let sink_activate = create_media_sink_activate(&sd, hwnd_video)?;
            let source_node = add_source_node(topology, source, pd, &sd)?;
            let output_node = add_output_node(topology, &sink_activate, 0)?;
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
            add_branch_to_partial_topology(&topology, source, pd, i, hwnd_video)?;
        }

        Ok(topology)
    }
}

/// IMFMediaSessionのラッパー。
#[derive(Debug, Clone)]
pub struct Session(MF::IMFAsyncCallback);

// Safety: 内包するIMFAsyncCallbackはOuterであり、OuterはSendであるため安全
unsafe impl Send for Session {}

impl Session {
    pub fn new(
        hwnd_video: F::HWND,
        event_loop: EventLoopWrapper,
        handler: ExtractHandler,
        source: TransportStream,
        initial_volume: Option<f32>,
        initial_rate: Option<f32>,
    ) -> WinResult<Session> {
        unsafe {
            let close_mutex = Arc::new(parking_lot::RawMutex::INIT);
            close_mutex.lock();

            let session = MF::MFCreateMediaSession(None)?;
            let presentation_clock = session.GetClock()?.cast()?;

            let source_pd = source.intf().CreatePresentationDescriptor()?;

            let topology = create_playback_topology(source.intf(), &source_pd, hwnd_video)?;
            session.SetTopology(0, &topology)?;

            let inner = Mutex::new(Inner {
                close_mutex,
                handler,

                session,
                source,
                presentation_clock,
                video_display: None,
                audio_volume: None,
                rate_control: None,

                state: State::Ready,
                initial_volume,
                current_rate: initial_rate.unwrap_or(1.),
                seeking_pos: None,
                is_pending: false,
                op_request: OpRequest {
                    command: None,
                    rate: None,
                    pos: None,
                },

                hwnd_video,
                event_loop,
            });
            let this = Session(Outer { inner }.into());

            this.inner().session.BeginGetEvent(&this.0, None)?;

            Ok(this)
        }
    }
}

impl Session {
    #[inline]
    fn inner(&self) -> parking_lot::MutexGuard<Inner> {
        let outer: &Outer = self.0.as_impl();
        outer.inner.lock()
    }

    #[inline]
    pub fn source(&self) -> TransportStream {
        self.inner().source.clone()
    }

    #[inline]
    pub fn close(&self) -> WinResult<()> {
        Inner::close(&mut self.inner())
    }

    #[inline]
    pub fn handle_event(&self, event: MF::IMFMediaEvent) -> WinResult<()> {
        self.inner().handle_event(event)
    }

    #[inline]
    pub fn play(&self) -> WinResult<()> {
        self.inner().play()
    }

    #[inline]
    pub fn pause(&self) -> WinResult<()> {
        self.inner().pause()
    }

    #[inline]
    pub fn play_or_pause(&self) -> WinResult<()> {
        self.inner().play_or_pause()
    }

    #[inline]
    pub fn stop(&self) -> WinResult<()> {
        self.inner().stop()
    }

    #[inline]
    pub fn repaint(&self) -> WinResult<()> {
        self.inner().repaint()
    }

    #[inline]
    pub fn resize_video(&self, width: u32, height: u32) -> WinResult<()> {
        self.inner().resize_video(width, height)
    }

    #[inline]
    pub fn position(&self) -> WinResult<Duration> {
        self.inner().position()
    }

    #[inline]
    pub fn set_position(&self, pos: Duration) -> WinResult<()> {
        self.inner().set_position(pos)
    }

    #[inline]
    pub fn volume(&self) -> WinResult<f32> {
        self.inner().volume()
    }

    #[inline]
    pub fn set_volume(&self, value: f32) -> WinResult<()> {
        self.inner().set_volume(value)
    }

    #[inline]
    pub fn rate(&self) -> WinResult<f32> {
        self.inner().rate()
    }

    #[inline]
    pub fn set_rate(&self, value: f32) -> WinResult<()> {
        self.inner().set_rate(value)
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
    command: Option<Command>,
    rate: Option<f32>,
    pos: Option<Duration>,
}

#[implement(MF::IMFAsyncCallback)]
struct Outer {
    inner: Mutex<Inner>,
}

struct Inner {
    // セッションが閉じられたらロック解除されるミューテックス
    close_mutex: Arc<parking_lot::RawMutex>,
    handler: ExtractHandler,

    session: MF::IMFMediaSession,
    source: TransportStream,
    presentation_clock: MF::IMFPresentationClock,
    video_display: Option<MF::IMFVideoDisplayControl>,
    audio_volume: Option<MF::IMFAudioStreamVolume>,
    rate_control: Option<MF::IMFRateControl>,

    state: State,
    /// 再生開始時の音量。
    initial_volume: Option<f32>,
    /// 現在の再生速度
    current_rate: f32,
    /// シーク待ち中のシーク位置
    seeking_pos: Option<Duration>,
    /// 再生・停止や速度変更等の処理待ち
    is_pending: bool,
    /// 処理待ち中に受け付けた操作要求
    op_request: OpRequest,

    hwnd_video: F::HWND,
    event_loop: EventLoopWrapper,
}

// Safety: C++のサンプルではスレッドをまたいで使っているので安全なはず
unsafe impl Send for Inner {}

impl Outer {
    #[inline]
    fn intf(&self) -> MF::IMFAsyncCallback {
        unsafe { self.cast().unwrap() }
    }
}

impl Inner {
    fn close(this: &mut MutexGuard<Inner>) -> WinResult<()> {
        fn shutdown(this: &mut MutexGuard<Inner>) -> WinResult<()> {
            unsafe {
                this.state = State::Closing;
                this.session.Close()?;

                // IMFMediaSession::Close()の呼び出しでOuter::Invokeが呼ばれるため、
                // 閉じるのを待つ間はロックを解除する
                let close_mutex = this.close_mutex.clone();
                let wait_result =
                    MutexGuard::unlocked(this, || close_mutex.try_lock_for(Duration::from_secs(5)));
                if !wait_result {
                    log::trace!("Session::shutdown timed out");
                }

                let _ = this.source.intf().Shutdown();
                let _ = this.session.Shutdown();

                Ok(())
            }
        }

        this.video_display.take();
        this.audio_volume.take();
        this.rate_control.take();

        let r = shutdown(this);

        unsafe { this.close_mutex.unlock() }

        r
    }

    fn run_pending_ops(&mut self, new_state: State) -> WinResult<()> {
        if self.state == new_state && self.is_pending {
            self.is_pending = false;

            match self.op_request.command.take() {
                None => {}
                Some(Command::Start) => self.start_playback()?,
                Some(Command::Pause) => self.pause()?,
                Some(Command::Stop) => self.stop()?,
            }

            if let Some(rate) = self.op_request.rate.take() {
                if rate != self.current_rate {
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

    pub fn handle_event(&mut self, event: MF::IMFMediaEvent) -> WinResult<()> {
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
                    log::trace!("media event: {:?}", me);
                    status.ok()?;
                }
            }

            Ok(())
        }
    }

    fn on_session_started(
        &mut self,
        status: C::HRESULT,
        _event: &MF::IMFMediaEvent,
    ) -> WinResult<()> {
        log::debug!("Session::on_session_started");
        status.ok()?;

        self.run_pending_ops(State::Started)?;
        Ok(())
    }

    fn on_session_paused(
        &mut self,
        status: C::HRESULT,
        _event: &MF::IMFMediaEvent,
    ) -> WinResult<()> {
        log::debug!("Session::on_session_paused");
        status.ok()?;

        self.run_pending_ops(State::Paused)?;
        Ok(())
    }

    fn on_session_rate_changed(
        &mut self,
        status: C::HRESULT,
        event: &MF::IMFMediaEvent,
    ) -> WinResult<()> {
        unsafe {
            log::debug!("Session::on_session_rate_changed");

            // 速度変更が成功した場合は既に速度をキャッシュ済み
            // 失敗した場合は実際の速度に更新
            if status.is_err() {
                if let Ok(PropVariant::F32(rate)) = event.GetValue()?.try_into() {
                    self.current_rate = rate;
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

    fn on_topology_status(
        &mut self,
        status: C::HRESULT,
        event: &MF::IMFMediaEvent,
    ) -> WinResult<()> {
        unsafe {
            status.ok()?;

            let status = event.GetUINT32(&MF::MF_EVENT_TOPOLOGY_STATUS)?;
            if status == MF::MF_TOPOSTATUS_READY.0 as u32 {
                log::debug!("Session::on_topology_ready");

                self.video_display = self.get_service(&MF::MR_VIDEO_RENDER_SERVICE).ok();
                self.audio_volume = self.get_service(&MF::MR_STREAM_VOLUME_SERVICE).ok();
                self.rate_control = self.get_service(&MF::MF_RATE_CONTROL_SERVICE).ok();

                if let Some(volume) = self.initial_volume {
                    if let Some(audio_volume) = &self.audio_volume {
                        if let Ok(nch) = audio_volume.GetChannelCount() {
                            let _ = audio_volume.SetAllVolumes(&*vec![volume; nch as usize]);
                        }
                    }
                }
                if let Some(rate_control) = &self.rate_control {
                    let _ = rate_control.SetRate(F::FALSE, self.current_rate);
                }

                self.start_playback()?;
            }
            Ok(())
        }
    }

    fn on_presentation_ended(
        &mut self,
        status: C::HRESULT,
        _: &MF::IMFMediaEvent,
    ) -> WinResult<()> {
        log::debug!("Session::on_presentation_ended");
        status.ok()?;

        self.state = State::Stopped;
        Ok(())
    }

    fn on_new_presentation(
        &mut self,
        status: C::HRESULT,
        event: &MF::IMFMediaEvent,
    ) -> WinResult<()> {
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
            let topology = create_playback_topology(self.source.intf(), &pd, self.hwnd_video)?;
            self.session.SetTopology(0, &topology)?;

            self.state = State::OpenPending;

            Ok(())
        }
    }

    fn start_playback(&mut self) -> WinResult<()> {
        unsafe {
            log::debug!("Session::start_playback");

            self.session.Start(&GUID_NULL, &Default::default())?;

            self.state = State::Started;
            self.is_pending = true;

            Ok(())
        }
    }

    pub fn play(&mut self) -> WinResult<()> {
        if !matches!(self.state, State::Paused | State::Stopped) {
            return Err(MF::MF_E_INVALIDREQUEST.into());
        }

        if self.is_pending {
            self.op_request.command = Some(Command::Start);
        } else {
            self.start_playback()?;
        }

        Ok(())
    }

    pub fn pause(&mut self) -> WinResult<()> {
        unsafe {
            if self.state != State::Started {
                return Err(MF::MF_E_INVALIDREQUEST.into());
            }

            if self.is_pending {
                self.op_request.command = Some(Command::Pause);
            } else {
                self.session.Pause()?;

                self.state = State::Paused;
                self.is_pending = true;
            }

            Ok(())
        }
    }

    pub fn stop(&mut self) -> WinResult<()> {
        unsafe {
            if matches!(self.state, State::Started | State::Paused) {
                return Err(MF::MF_E_INVALIDREQUEST.into());
            }

            if self.is_pending {
                self.op_request.command = Some(Command::Stop);
            } else {
                self.session.Stop()?;
                self.handler.reset();

                self.state = State::Stopped;
                self.is_pending = true;
            }

            Ok(())
        }
    }

    pub fn play_or_pause(&mut self) -> WinResult<()> {
        match self.state {
            State::Started => self.pause()?,
            State::Paused => self.play()?,
            _ => return Err(MF::MF_E_INVALIDREQUEST.into()),
        }

        Ok(())
    }

    pub fn repaint(&mut self) -> WinResult<()> {
        unsafe {
            if let Some(video_display) = &self.video_display {
                video_display.RepaintVideo()
            } else {
                Ok(())
            }
        }
    }

    pub fn resize_video(&mut self, width: u32, height: u32) -> WinResult<()> {
        unsafe {
            if let Some(video_display) = &self.video_display {
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
            let pos = if let Some(pos) = self.op_request.pos.or(self.seeking_pos) {
                pos
            } else {
                Duration::from_nanos(self.presentation_clock.GetTime()? as u64 * 100)
            };

            Ok(pos)
        }
    }

    fn set_position_internal(&mut self, pos: Duration) -> WinResult<()> {
        unsafe {
            let time = (pos.as_nanos() / 100) as i64;
            self.session
                .Start(&GUID_NULL, &PropVariant::I64(time).to_raw())?;
            if self.state == State::Paused {
                self.session.Pause()?
            }

            self.seeking_pos = Some(pos);
            self.is_pending = true;

            Ok(())
        }
    }

    pub fn set_position(&mut self, pos: Duration) -> WinResult<()> {
        if self.is_pending {
            self.op_request.pos = Some(pos);
        } else {
            self.set_position_internal(pos)?;
        }

        Ok(())
    }

    pub fn volume(&self) -> WinResult<f32> {
        unsafe {
            let Some(audio_volume) = &self.audio_volume else {
                return Err(MF::MF_E_INVALIDREQUEST.into());
            };

            if audio_volume.GetChannelCount()? < 1 {
                return Err(MF::MF_E_INVALIDREQUEST.into());
            }

            let value = audio_volume.GetChannelVolume(0)?;
            Ok(value)
        }
    }

    pub fn set_volume(&mut self, value: f32) -> WinResult<()> {
        unsafe {
            let Some(audio_volume) = &self.audio_volume else {
                return Err(MF::MF_E_INVALIDREQUEST.into());
            };

            let nch = audio_volume.GetChannelCount()? as usize;
            audio_volume.SetAllVolumes(&*vec![value; nch])?;
            Ok(())
        }
    }

    pub fn rate(&self) -> WinResult<f32> {
        unsafe {
            let Some(rate_control) = &self.rate_control else {
                return Err(MF::MF_E_INVALIDREQUEST.into());
            };

            let mut rate = 0.;
            rate_control.GetRate(std::ptr::null_mut(), &mut rate)?;
            Ok(rate)
        }
    }

    pub fn set_rate(&mut self, value: f32) -> WinResult<()> {
        unsafe {
            let Some(rate_control) = &self.rate_control else {
                return Err(MF::MF_E_INVALIDREQUEST.into());
            };

            if self.is_pending {
                self.op_request.rate = Some(value);
            } else {
                rate_control.SetRate(F::FALSE, value)?;
                self.current_rate = value;
            }

            Ok(())
        }
    }
}

#[allow(non_snake_case)]
impl MF::IMFAsyncCallback_Impl for Outer {
    fn GetParameters(&self, _: *mut u32, _: *mut u32) -> WinResult<()> {
        log::trace!("Session::GetParameters");
        Err(F::E_NOTIMPL.into())
    }

    fn Invoke(&self, presult: &Option<MF::IMFAsyncResult>) -> WinResult<()> {
        log::trace!("Session::Invoke");
        unsafe {
            let inner = self.inner.lock();

            let event = inner.session.EndGetEvent(presult.as_ref())?;
            let me_type = event.GetType()?;
            if me_type == MF::MESessionClosed.0 as u32 {
                inner.close_mutex.unlock();
            } else {
                inner.session.BeginGetEvent(&self.intf(), None)?;
            }

            if inner.state != State::Closing {
                let _ = inner.event_loop.0.send_player_event(event);
            }

            Ok(())
        }
    }
}
