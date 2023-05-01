use std::ops::RangeInclusive;
use std::ptr;
use std::sync::Arc;
use std::time::Duration;

use parking_lot::lock_api::{RawMutex, RawMutexTimed};
use parking_lot::{Mutex, MutexGuard};
use windows::core::{self as C, implement, AsImpl, ComInterface, Result as WinResult};
use windows::Win32::Foundation as F;
use windows::Win32::Media::KernelStreaming::GUID_NULL;
use windows::Win32::Media::MediaFoundation as MF;

use crate::extract::ExtractHandler;
use crate::player::{EventHandler, PlayerEvent};
use crate::sys::com::PropVariant;
use crate::sys::wrap;

use super::source::TransportStream;
use super::PlayerState;

fn create_media_sink_activate(
    source_sd: &MF::IMFStreamDescriptor,
    hwnd_video: F::HWND,
) -> WinResult<MF::IMFActivate> {
    let handler = unsafe { source_sd.GetMediaTypeHandler()? };
    let major_type = unsafe { handler.GetMajorType()? };

    if log::log_enabled!(log::Level::Trace) {
        let media_type = unsafe { handler.GetCurrentMediaType()? };
        log::trace!(
            "codec: {}",
            match unsafe { media_type.GetGUID(&MF::MF_MT_SUBTYPE)? } {
                MF::MFVideoFormat_MPEG2 => "MPEG-2",
                MF::MFVideoFormat_H264 => "H.264",
                MF::MFVideoFormat_H265 => "H.265",
                MF::MFAudioFormat_MPEG => "MPEG Audio",
                MF::MFAudioFormat_AAC => "AAC",
                MF::MFAudioFormat_Dolby_AC3 => "AC3",
                _ => "Unknown",
            }
        );
        if let Ok(size) = unsafe { media_type.GetUINT64(&MF::MF_MT_FRAME_SIZE) } {
            log::trace!("size: {}x{}", (size >> 32) as u32, size as u32);
        }
        if let Ok(ratio) = unsafe { media_type.GetUINT64(&MF::MF_MT_PIXEL_ASPECT_RATIO) } {
            log::trace!("ratio: {}/{}", (ratio >> 32) as u32, ratio as u32);
        }
    }

    let activate = match major_type {
        MF::MFMediaType_Audio => unsafe { MF::MFCreateAudioRendererActivate()? },
        MF::MFMediaType_Video => unsafe { MF::MFCreateVideoRendererActivate(hwnd_video)? },
        _ => return Err(F::E_FAIL.into()),
    };

    Ok(activate)
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
        node.SetUINT32(&MF::MF_TOPONODE_NOSHUTDOWN_ON_REMOVE, F::TRUE.0 as u32)?;
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
    let (selected, sd) = wrap::wrap2(|a, b| unsafe { pd.GetStreamDescriptorByIndex(i, a, b) })?;
    let sd = sd.unwrap();
    if selected {
        let sink_activate = create_media_sink_activate(&sd, hwnd_video)?;
        let source_node = add_source_node(topology, source, pd, &sd)?;
        let output_node = add_output_node(topology, &sink_activate, 0)?;
        unsafe { source_node.ConnectOutput(0, &output_node, 0)? };
    }

    Ok(())
}

fn create_playback_topology(
    source: &MF::IMFMediaSource,
    pd: &MF::IMFPresentationDescriptor,
    hwnd_video: F::HWND,
) -> WinResult<MF::IMFTopology> {
    let topology = unsafe { MF::MFCreateTopology()? };

    let c_source_streams = unsafe { pd.GetStreamDescriptorCount()? };
    for i in 0..c_source_streams {
        add_branch_to_partial_topology(&topology, source, pd, i, hwnd_video)?;
    }

    Ok(topology)
}

/// IMFMediaSessionのラッパー。
#[derive(Debug, Clone)]
pub struct Session(MF::IMFAsyncCallback);

// Safety: 内包するIMFAsyncCallbackはOuterであり、OuterはSendであるため安全
unsafe impl Send for Session {}

impl Session {
    pub(super) fn new<H: EventHandler>(
        player_state: Arc<Mutex<PlayerState>>,
        event_handler: H,
        extract_handler: ExtractHandler,
        source: TransportStream,
    ) -> WinResult<Session> {
        let close_mutex = Arc::new(parking_lot::RawMutex::INIT);
        close_mutex.lock();

        let session = unsafe { MF::MFCreateMediaSession(None)? };
        let presentation_clock = unsafe { session.GetClock()?.cast()? };

        let source_pd = unsafe { source.intf().CreatePresentationDescriptor()? };

        let topology =
            create_playback_topology(source.intf(), &source_pd, player_state.lock().hwnd_video)?;
        unsafe { session.SetTopology(0, &topology)? };

        let inner = Mutex::new(Inner {
            close_mutex,
            player_state,
            extract_handler,

            session,
            source,
            presentation_clock,
            video_display: None,
            audio_volume: None,
            rate_control: None,
            rate_support: None,

            state: State::Closed,
            status: Status::Closed,
            seeking_pos: None,
            is_pending: false,
            op_request: OpRequest {
                command: None,
                rate: None,
                pos: None,
            },

            event_handler: Box::new(event_handler),
        });
        let this = Session(Outer { inner }.into());

        unsafe { this.inner().session.BeginGetEvent(&this.0, None)? };

        Ok(this)
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
    pub fn stop(&self) -> WinResult<()> {
        self.inner().stop()
    }

    #[inline]
    pub fn repaint(&self) -> WinResult<()> {
        self.inner().repaint()
    }

    #[inline]
    pub fn set_bounds(&self, left: u32, top: u32, right: u32, bottom: u32) -> WinResult<()> {
        self.inner().set_bounds(left, top, right, bottom)
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
    pub fn set_volume(&self, value: f32) -> WinResult<()> {
        self.inner().set_volume(value)
    }

    #[inline]
    pub fn set_muted(&self, mute: bool) -> WinResult<()> {
        self.inner().set_muted(mute)
    }

    #[inline]
    pub fn rate_range(&self) -> WinResult<RangeInclusive<f32>> {
        self.inner().rate_range()
    }

    #[inline]
    pub fn set_rate(&self, value: f32) -> WinResult<()> {
        self.inner().set_rate(value)
    }
}

/// セッションに要求している状態。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Closed,
    OpenPending,
    Started,
    Paused,
    Stopped,
    Closing,
}

/// セッションから報告された現在の状態。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
    Closed,
    Ready,
    Started,
    Paused,
    Stopped,
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
    player_state: Arc<Mutex<PlayerState>>,
    extract_handler: ExtractHandler,

    session: MF::IMFMediaSession,
    source: TransportStream,
    presentation_clock: MF::IMFPresentationClock,
    video_display: Option<MF::IMFVideoDisplayControl>,
    audio_volume: Option<MF::IMFSimpleAudioVolume>,
    rate_control: Option<MF::IMFRateControl>,
    rate_support: Option<MF::IMFRateSupport>,

    state: State,
    status: Status,
    /// シーク待ち中のシーク位置
    seeking_pos: Option<Duration>,
    /// 再生・停止や速度変更等の処理待ち
    is_pending: bool,
    /// 処理待ち中に受け付けた操作要求
    op_request: OpRequest,

    // #[implement]の制約でOuterをジェネリクスにできないのでBox化
    event_handler: Box<dyn EventHandler>,
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
        this.video_display.take();
        this.audio_volume.take();
        this.rate_control.take();
        this.rate_support.take();

        let r = 'r: {
            this.state = State::Closing;
            unsafe { tri!('r, this.session.Close()) };

            // IMFMediaSession::Close()の呼び出しでOuter::Invokeが呼ばれるため、
            // 閉じるのを待つ間はロックを解除する
            let close_mutex = this.close_mutex.clone();
            let wait_result =
                MutexGuard::unlocked(this, || close_mutex.try_lock_for(Duration::from_secs(5)));
            if !wait_result {
                log::trace!("Session::shutdown: timeout");
            }

            let _ = unsafe { this.source.intf().Shutdown() };
            let _ = unsafe { this.session.Shutdown() };

            Ok(())
        };

        unsafe { this.close_mutex.unlock() };
        this.state = State::Closed;
        this.status = Status::Closed;

        r
    }

    fn set_status(&mut self, new_status: Status) {
        if self.status != new_status {
            self.status = new_status;

            match new_status {
                Status::Closed => unreachable!(),
                Status::Ready => self.event_handler.on_ready(),
                Status::Started => self.event_handler.on_started(),
                Status::Paused => self.event_handler.on_paused(),
                Status::Stopped => self.event_handler.on_stopped(),
            }
        }
    }

    fn update_playback_status(&mut self, new_state: State, new_status: Status) -> WinResult<()> {
        if self.state == new_state && self.is_pending {
            self.is_pending = false;

            self.set_status(new_status);

            // 保留中の処理を実行

            match (self.op_request.pos.take(), self.op_request.command.take()) {
                (Some(pos), command) => self.set_position_internal(pos, command)?,
                (None, Some(Command::Start)) => self.start_playback()?,
                (None, Some(Command::Pause)) => self.pause()?,
                (None, Some(Command::Stop)) => self.stop()?,
                (None, None) => {}
            }

            if let Some(rate) = self.op_request.rate.take() {
                if rate != self.player_state.lock().rate {
                    self.set_rate(rate)?;
                }
            }
        }

        Ok(())
    }

    pub fn handle_event(&mut self, event: MF::IMFMediaEvent) -> WinResult<()> {
        let me_type = unsafe { event.GetType()? };
        let status = unsafe { event.GetStatus()? };

        match MF::MF_EVENT_TYPE(me_type as i32) {
            MF::MESessionStarted => self.on_session_started(status, &event)?,
            MF::MESessionPaused => self.on_session_paused(status, &event)?,
            MF::MESessionStopped => self.on_session_stopped(status, &event)?,
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

    fn on_session_started(
        &mut self,
        status: C::HRESULT,
        _event: &MF::IMFMediaEvent,
    ) -> WinResult<()> {
        log::trace!("Session::on_session_started");
        status.ok()?;

        if let Some(pos) = self.seeking_pos.take() {
            self.event_handler.on_seek_completed(pos);
        }

        self.update_playback_status(State::Started, Status::Started)?;
        Ok(())
    }

    fn on_session_paused(
        &mut self,
        status: C::HRESULT,
        _event: &MF::IMFMediaEvent,
    ) -> WinResult<()> {
        log::trace!("Session::on_session_paused");
        status.ok()?;

        self.update_playback_status(State::Paused, Status::Paused)?;
        Ok(())
    }

    fn on_session_stopped(
        &mut self,
        status: C::HRESULT,
        _event: &MF::IMFMediaEvent,
    ) -> WinResult<()> {
        log::trace!("Session::on_session_stopped");
        status.ok()?;

        self.update_playback_status(State::Stopped, Status::Stopped)?;
        Ok(())
    }

    fn on_session_rate_changed(
        &mut self,
        status: C::HRESULT,
        _event: &MF::IMFMediaEvent,
    ) -> WinResult<()> {
        log::trace!("Session::on_session_rate_changed");

        let mut player_state = self.player_state.lock();

        // 速度変更が成功した場合は既に速度をキャッシュ済み
        // 失敗した場合は実際の速度に更新
        if status.is_err() {
            // ドキュメント上は`event.GetValue()`に実際の速度が入っているようだが、
            // 実際は指定された値がそのまま入っているだけのようなので
            // IMFRateControlから実際の速度を取得する
            if let Some(rate_control) = &self.rate_control {
                let _ = unsafe { rate_control.GetRate(ptr::null_mut(), &mut player_state.rate) };
            }
        }

        self.event_handler.on_rate_changed(player_state.rate);

        Ok(())
    }

    fn get_service<T: ComInterface>(&self, guid: &C::GUID) -> WinResult<T> {
        let mut ptr = ptr::null_mut();
        unsafe { MF::MFGetService(&self.session, guid, &T::IID, &mut ptr)? };
        debug_assert!(!ptr.is_null());
        Ok(unsafe { T::from_raw(ptr) })
    }

    fn on_topology_status(
        &mut self,
        status: C::HRESULT,
        event: &MF::IMFMediaEvent,
    ) -> WinResult<()> {
        status.ok()?;

        let status = unsafe { event.GetUINT32(&MF::MF_EVENT_TOPOLOGY_STATUS)? };
        if status == MF::MF_TOPOSTATUS_READY.0 as u32 {
            log::trace!("Session::on_topology_ready");

            self.video_display = self.get_service(&MF::MR_VIDEO_RENDER_SERVICE).ok();
            self.audio_volume = self.get_service(&MF::MR_POLICY_VOLUME_SERVICE).ok();
            self.rate_control = self.get_service(&MF::MF_RATE_CONTROL_SERVICE).ok();
            self.rate_support = self.get_service(&MF::MF_RATE_CONTROL_SERVICE).ok();

            {
                let player_state = self.player_state.lock();

                let (left, top, right, bottom) = player_state.bounds;
                if let Err(e) = self.set_bounds_internal(left, top, right, bottom) {
                    log::warn!("映像領域を設定できない：{}", e);
                }
                if let Err(e) = self.set_volume_internal(player_state.volume) {
                    log::warn!("音量を設定できない：{}", e);
                }
                if let Err(e) = self.set_muted_internal(player_state.muted) {
                    log::warn!("ミュート状態を設定できない：{}", e);
                }
                if let Err(e) = self.set_rate_internal(player_state.rate) {
                    log::warn!("再生速度を設定できない：{}", e);
                }
            }

            self.set_status(Status::Ready);

            self.start_playback()?;
        }
        Ok(())
    }

    fn on_presentation_ended(
        &mut self,
        status: C::HRESULT,
        _: &MF::IMFMediaEvent,
    ) -> WinResult<()> {
        log::trace!("Session::on_presentation_ended");
        status.ok()?;

        self.state = State::Stopped;
        self.set_status(Status::Stopped);

        Ok(())
    }

    fn on_new_presentation(
        &mut self,
        status: C::HRESULT,
        event: &MF::IMFMediaEvent,
    ) -> WinResult<()> {
        fn get_event_object<T: ComInterface>(event: &MF::IMFMediaEvent) -> WinResult<T> {
            let Ok(PropVariant::IUnknown(unk)) = PropVariant::try_from(unsafe{ event.GetValue()? }) else {
                return Err(MF::MF_E_INVALIDTYPE.into());
            };

            unk.cast()
        }

        log::trace!("Session::on_new_presentation");
        status.ok()?;

        let pd = get_event_object(event)?;
        let topology =
            create_playback_topology(self.source.intf(), &pd, self.player_state.lock().hwnd_video)?;
        unsafe { self.session.SetTopology(0, &topology)? };

        self.state = State::OpenPending;

        Ok(())
    }

    fn start_playback(&mut self) -> WinResult<()> {
        log::trace!("Session::start_playback");

        let start_pos = if self.state == State::Stopped {
            // 停止状態からの再生は最初から
            self.extract_handler
                .reset()
                .map_err(|_| MF::MF_E_INVALIDREQUEST)?;
            PropVariant::I64(0)
        } else {
            // それ以外は位置を保持
            PropVariant::Empty
        };
        unsafe { self.session.Start(&GUID_NULL, &start_pos.to_raw())? };

        self.state = State::Started;
        self.is_pending = true;

        Ok(())
    }

    pub fn play(&mut self) -> WinResult<()> {
        match self.state {
            State::Paused | State::Stopped => {}
            State::Started => return Ok(()),
            _ => return Err(MF::MF_E_INVALIDREQUEST.into()),
        }

        if self.is_pending {
            self.op_request.command = Some(Command::Start);
        } else {
            self.start_playback()?;
        }

        Ok(())
    }

    pub fn pause(&mut self) -> WinResult<()> {
        match self.state {
            State::Started => {}
            State::Paused => return Ok(()),
            _ => return Err(MF::MF_E_INVALIDREQUEST.into()),
        }

        if self.is_pending {
            self.op_request.command = Some(Command::Pause);
        } else {
            unsafe { self.session.Pause()? };

            self.state = State::Paused;
            self.is_pending = true;
        }

        Ok(())
    }

    pub fn stop(&mut self) -> WinResult<()> {
        match self.state {
            State::Started | State::Paused => {}
            State::Stopped => return Ok(()),
            _ => return Err(MF::MF_E_INVALIDREQUEST.into()),
        }

        if self.is_pending {
            self.op_request.command = Some(Command::Stop);
        } else {
            unsafe { self.session.Stop()? };

            self.state = State::Stopped;
            self.is_pending = true;
        }

        Ok(())
    }

    pub fn repaint(&mut self) -> WinResult<()> {
        if let Some(video_display) = &self.video_display {
            unsafe { video_display.RepaintVideo() }
        } else {
            Ok(())
        }
    }

    fn set_bounds_internal(&self, left: u32, top: u32, right: u32, bottom: u32) -> WinResult<()> {
        let Some(video_display) = &self.video_display else {
            return Ok(());
        };

        let size = wrap::wrap(|a| unsafe { video_display.GetNativeVideoSize(a, ptr::null_mut()) })?;

        let src = MF::MFVideoNormalizedRect {
            left: 0.,
            top: 0.,
            right: 1.,
            bottom: if size.cy == 1088 { 1080. / 1088. } else { 1. },
        };
        let dst = F::RECT {
            left: left as i32,
            top: top as i32,
            right: right as i32,
            bottom: bottom as i32,
        };
        unsafe { video_display.SetVideoPosition(&src, &dst)? };

        Ok(())
    }

    pub fn set_bounds(&mut self, left: u32, top: u32, right: u32, bottom: u32) -> WinResult<()> {
        let r = self.set_bounds_internal(left, top, right, bottom);
        if r.is_ok() {
            self.player_state.lock().bounds = (left, top, right, bottom);
        }

        r
    }

    pub fn position(&self) -> WinResult<Duration> {
        let pos = if let Some(pos) = self.op_request.pos.or(self.seeking_pos) {
            pos
        } else {
            Duration::from_nanos(unsafe { self.presentation_clock.GetTime()? } as u64 * 100)
        };

        Ok(pos)
    }

    fn set_position_internal(&mut self, pos: Duration, command: Option<Command>) -> WinResult<()> {
        if command == Some(Command::Stop) {
            return self.stop();
        }

        let time = (pos.as_nanos() / 100) as i64;
        unsafe {
            self.session
                .Start(&GUID_NULL, &PropVariant::I64(time).to_raw())?
        };

        // 要求されている状態や現在の状態によって遷移
        match (command, self.state) {
            (Some(Command::Stop), _) => unreachable!(),

            (Some(Command::Start), _) | (None, State::Started) => {
                self.state = State::Started;
            }

            (Some(Command::Pause), _) | (None, State::Paused) => {
                unsafe { self.session.Pause()? };
                self.state = State::Paused;
            }

            (None, _) => log::debug!("シーク時に不明な状態：{:?}", self.state),
        }

        self.seeking_pos = Some(pos);
        self.is_pending = true;

        Ok(())
    }

    pub fn set_position(&mut self, pos: Duration) -> WinResult<()> {
        if self.is_pending {
            self.op_request.pos = Some(pos);
        } else {
            self.set_position_internal(pos, None)?;
        }

        Ok(())
    }

    fn set_volume_internal(&self, value: f32) -> WinResult<()> {
        let Some(audio_volume) = &self.audio_volume else {
            return Err(MF::MF_E_INVALIDREQUEST.into());
        };

        unsafe { audio_volume.SetMasterVolume(value)? };
        Ok(())
    }

    pub fn set_volume(&mut self, value: f32) -> WinResult<()> {
        let r = self.set_volume_internal(value);
        if r.is_ok() {
            self.player_state.lock().volume = value;
        }

        r
    }

    fn set_muted_internal(&self, mute: bool) -> WinResult<()> {
        let Some(audio_volume) = &self.audio_volume else {
            return Err(MF::MF_E_INVALIDREQUEST.into());
        };

        unsafe { audio_volume.SetMute(F::BOOL::from(mute))? };
        Ok(())
    }

    pub fn set_muted(&mut self, mute: bool) -> WinResult<()> {
        let r = self.set_muted_internal(mute);
        if r.is_ok() {
            self.player_state.lock().muted = mute;
        }

        r
    }

    pub fn rate_range(&self) -> WinResult<RangeInclusive<f32>> {
        let Some(rate_support) = &self.rate_support else {
            return Err(MF::MF_E_INVALIDREQUEST.into());
        };

        let slowest = unsafe { rate_support.GetSlowestRate(MF::MFRATE_FORWARD, F::FALSE)? };
        let fastest = unsafe { rate_support.GetFastestRate(MF::MFRATE_FORWARD, F::FALSE)? };
        Ok(slowest..=fastest)
    }

    fn set_rate_internal(&self, value: f32) -> WinResult<()> {
        let Some(rate_control) = &self.rate_control else {
            return Err(MF::MF_E_INVALIDREQUEST.into());
        };

        unsafe { rate_control.SetRate(F::FALSE, value)? };
        Ok(())
    }

    pub fn set_rate(&mut self, value: f32) -> WinResult<()> {
        if self.is_pending {
            self.op_request.rate = Some(value);
        } else {
            self.set_rate_internal(value)?;
        }
        self.player_state.lock().rate = value;

        Ok(())
    }
}

#[allow(non_snake_case)]
impl MF::IMFAsyncCallback_Impl for Outer {
    fn GetParameters(&self, _: *mut u32, _: *mut u32) -> WinResult<()> {
        log::trace!("Session::GetParameters");
        Err(F::E_NOTIMPL.into())
    }

    fn Invoke(&self, presult: Option<&MF::IMFAsyncResult>) -> WinResult<()> {
        log::trace!("Session::Invoke");
        let inner = self.inner.lock();

        let event = unsafe { inner.session.EndGetEvent(presult)? };
        let me_type = unsafe { event.GetType()? };
        if me_type == MF::MESessionClosed.0 as u32 {
            unsafe { inner.close_mutex.unlock() };
        } else {
            unsafe { inner.session.BeginGetEvent(&self.intf(), None)? };
        }

        if inner.state != State::Closing {
            inner
                .event_handler
                .on_player_event(PlayerEvent(event.into()));
        }

        Ok(())
    }
}
