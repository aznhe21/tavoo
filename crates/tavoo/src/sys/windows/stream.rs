use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use windows::core::{self as C, implement, AsImpl, Interface};
use windows::Win32::Foundation as F;
use windows::Win32::Media::KernelStreaming::GUID_NULL;
use windows::Win32::Media::MediaFoundation as MF;

use crate::extract::ExtractHandler;

use super::dummy;
use super::utils::{get_stream_descriptor_by_index, PropVariant, WinResult};

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
// IMFStreamDescriptorのストリーム識別子。
const SID_VIDEO: u32 = 0;
const SID_AUDIO: u32 = 1;

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
    handler: ExtractHandler,

    queue: MF::IMFAsyncCallback,
    state: Cell<SourceState>,

    event_queue: MF::IMFMediaEventQueue,
    presentation_descriptor: MF::IMFPresentationDescriptor,
    video_stream: RefCell<MF::IMFMediaStream>,
    audio_stream: RefCell<MF::IMFMediaStream>,

    rate: Cell<f32>,
    first_pts: Cell<Option<isdb::time::Timestamp>>,
    pending_eos: Cell<usize>,
}

impl TransportStream {
    pub fn new(
        handler: ExtractHandler,
        video_stream: &isdb::filters::sorter::Stream,
        audio_stream: &isdb::filters::sorter::Stream,
    ) -> WinResult<MF::IMFMediaSource> {
        unsafe {
            let video_sd = Self::create_video_sd(video_stream)?;
            let audio_sd = Self::create_audio_sd(audio_stream)?;
            let presentation_descriptor =
                MF::MFCreatePresentationDescriptor(Some(&[video_sd.clone(), audio_sd.clone()]))?;
            presentation_descriptor.SelectStream(SID_VIDEO)?;
            presentation_descriptor.SelectStream(SID_AUDIO)?;

            let event_queue = MF::MFCreateEventQueue()?;
            let dummy_stream: MF::IMFMediaStream = dummy::DummyStream.into();

            let source: MF::IMFMediaSource = TransportStream {
                mutex: Arc::new(parking_lot::ReentrantMutex::new(())),
                handler,

                queue: AsyncQueue::new(),
                state: Cell::new(SourceState::Stopped),

                event_queue,
                presentation_descriptor,
                video_stream: RefCell::new(dummy_stream.clone()),
                audio_stream: RefCell::new(dummy_stream),

                rate: Cell::new(1.),
                first_pts: Cell::new(None),
                pending_eos: Cell::new(0),
            }
            .into();
            let this = source.as_impl();

            *this.video_stream.borrow_mut() = ElementaryStream::new(this, video_sd)?;
            *this.audio_stream.borrow_mut() = ElementaryStream::new(this, audio_sd)?;

            Ok(source)
        }
    }

    fn create_video_sd(
        stream: &isdb::filters::sorter::Stream,
    ) -> WinResult<MF::IMFStreamDescriptor> {
        unsafe {
            use isdb::psi::desc::StreamType;

            let vef = stream
                .video_encode_format()
                .unwrap_or_else(|| isdb::psi::desc::VideoEncodeFormat::from(0b0001));

            let media_type = MF::MFCreateMediaType()?;
            match stream.stream_type() {
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

            let stream_descriptor = MF::MFCreateStreamDescriptor(SID_VIDEO, &[media_type.clone()])?;

            let handler = stream_descriptor.GetMediaTypeHandler()?;
            handler.SetCurrentMediaType(&media_type)?;

            Ok(stream_descriptor)
        }
    }

    fn create_audio_sd(
        stream: &isdb::filters::sorter::Stream,
    ) -> WinResult<MF::IMFStreamDescriptor> {
        const SAMPLES_PER_SEC: u32 = 48000;
        const NUM_CHANNELS: u32 = 2;
        const BITS_PER_SAMPLE: u32 = 16;
        const BLOCK_ALIGNMENT: u32 = BITS_PER_SAMPLE * NUM_CHANNELS / 8;
        const AVG_BYTES_PER_SECOND: u32 = SAMPLES_PER_SEC * BLOCK_ALIGNMENT;

        unsafe {
            let media_type = MF::MFCreateMediaType()?;

            use isdb::psi::desc::StreamType;
            match stream.stream_type() {
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

            let stream_descriptor = MF::MFCreateStreamDescriptor(SID_AUDIO, &[media_type.clone()])?;

            let handler = stream_descriptor.GetMediaTypeHandler()?;
            handler.SetCurrentMediaType(&media_type)?;

            Ok(stream_descriptor)
        }
    }

    fn intf(&self) -> MF::IMFMediaSource {
        unsafe { self.cast().unwrap() }
    }

    fn queue(&self) -> &AsyncQueue {
        self.queue.as_impl()
    }

    fn video_stream(&self) -> std::cell::Ref<ElementaryStream> {
        std::cell::Ref::map(self.video_stream.borrow(), |s| s.as_impl())
    }

    fn audio_stream(&self) -> std::cell::Ref<ElementaryStream> {
        std::cell::Ref::map(self.audio_stream.borrow(), |s| s.as_impl())
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
            let c_streams = pd.GetStreamDescriptorCount()?;
            if c_streams != 2 {
                return Err(F::E_INVALIDARG.into());
            }

            let all_selected = get_stream_descriptor_by_index(pd, 0)?.0
                && get_stream_descriptor_by_index(pd, 1)?.0;
            if !all_selected {
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

                if let &PropVariant::I64(start_pos) = start_pos {
                    // ExtractHandlerに指定する開始時刻はPTS基準
                    let mut pos = (start_pos as u64) * 100;
                    if let Some(first_pts) = this.first_pts.get() {
                        pos += first_pts.as_nanos()
                    }
                    this.handler.set_position(Duration::from_nanos(pos).into());
                }

                this.select_streams(pd, Some(start_pos))?;

                this.state.set(SourceState::Started);

                this.event_queue.QueueEventParamVar(
                    MF::MESourceStarted.0 as u32,
                    &GUID_NULL,
                    F::S_OK,
                    &start_pos.to_raw(),
                )?;

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

            self.video_stream().stop()?;
            self.audio_stream().stop()?;

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

            self.video_stream().pause()?;
            self.audio_stream().pause()?;

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
        _pd: &MF::IMFPresentationDescriptor,
        start_pos: Option<&PropVariant>,
    ) -> WinResult<()> {
        unsafe {
            let activate = |es: &ElementaryStream| -> WinResult<()> {
                let was_selected = es.is_active();
                es.activate(true);

                if was_selected {
                    log::trace!("TransportStream: MEUpdatedStream");
                    self.event_queue.QueueEventParamUnk(
                        MF::MEUpdatedStream.0 as u32,
                        &GUID_NULL,
                        F::S_OK,
                        &es.intf(),
                    )?;
                } else {
                    log::trace!("TransportStream: MENewStream");
                    self.event_queue.QueueEventParamUnk(
                        MF::MENewStream.0 as u32,
                        &GUID_NULL,
                        F::S_OK,
                        &es.intf(),
                    )?;
                }

                es.start(start_pos)?;
                Ok(())
            };

            self.pending_eos.set(0);
            activate(&*self.video_stream())?;
            activate(&*self.audio_stream())?;
            self.pending_eos.set(2);
            Ok(())
        }
    }

    pub fn streams_need_data(&self) -> bool {
        let _lock = self.mutex.lock();

        match self.state.get() {
            SourceState::Shutdown => false,
            _ => self.video_stream().needs_data() || self.audio_stream().needs_data(),
        }
    }

    pub fn end_of_mpeg_stream(&self) -> WinResult<()> {
        let _lock = self.mutex.lock();

        self.video_stream().end_of_stream()?;
        self.audio_stream().end_of_stream()?;
        Ok(())
    }

    fn create_sample(
        &self,
        payload: &[u8],
        pts: Option<isdb::time::Timestamp>,
    ) -> WinResult<MF::IMFSample> {
        unsafe {
            let buffer = MF::MFCreateMemoryBuffer(payload.len() as u32)?;
            let mut data = std::ptr::null_mut();
            buffer.Lock(&mut data, None, None)?;
            std::ptr::copy_nonoverlapping(payload.as_ptr(), data, payload.len());
            buffer.Unlock()?;
            buffer.SetCurrentLength(payload.len() as u32)?;

            let sample = MF::MFCreateSample()?;
            sample.AddBuffer(&buffer)?;

            if let Some(pts) = pts {
                // FIXME: 映像切り替え時にその時点を`first_pts`にしてしまう
                // let pts = if let Some(first_pts) = self.first_pts.get() {
                //     // TODO: ラップアラウンドを考慮する
                //     // pts - first_pts
                //     pts.saturating_sub(first_pts)
                // } else {
                //     self.first_pts.set(Some(pts));
                //     isdb::time::Timestamp(0)
                // };
                sample.SetSampleTime((pts.as_nanos() / 100) as i64)?;
            }

            Ok(sample)
        }
    }

    fn deliver_packet(
        &self,
        es: &ElementaryStream,
        pts: Option<isdb::time::Timestamp>,
        payload: &[u8],
    ) {
        let _lock = self.mutex.lock();

        let r = self
            .create_sample(payload, pts)
            .and_then(|sample| es.deliver_payload(sample));
        if let Err(e) = r {
            log::error!("error[deliver_packet]: {}", e);
            self.streaming_error(e);
        }
    }

    #[inline]
    pub fn deliver_video_packet(&self, pts: Option<isdb::time::Timestamp>, payload: &[u8]) {
        self.deliver_packet(&*self.video_stream(), pts, payload);
    }

    #[inline]
    pub fn deliver_audio_packet(&self, pts: Option<isdb::time::Timestamp>, payload: &[u8]) {
        self.deliver_packet(&*self.audio_stream(), pts, payload);
    }

    fn request_sample(&self) -> WinResult<()> {
        self.handler.request_es();
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

            let pd = self.presentation_descriptor.Clone()?;
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
            self.enqueue_op(move |this| this.do_start(&pd, &start_pos))?;

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

            let _ = self.video_stream().shutdown();
            let _ = self.audio_stream().shutdown();
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

    stream_descriptor: MF::IMFStreamDescriptor,

    state: Cell<StreamState>,
    is_active: Cell<bool>,
    is_eos: Cell<bool>,

    samples: RefCell<VecDeque<MF::IMFSample>>,
    requests: RefCell<VecDeque<Option<C::IUnknown>>>,
}

impl ElementaryStream {
    fn new(
        source: &TransportStream,
        stream_descriptor: MF::IMFStreamDescriptor,
    ) -> WinResult<MF::IMFMediaStream> {
        unsafe {
            let event_queue = MF::MFCreateEventQueue()?;
            Ok(ElementaryStream {
                mutex: source.mutex.clone(),

                source: source.intf().downgrade().unwrap(),
                event_queue,

                stream_descriptor,

                state: Cell::new(StreamState::Stopped),
                is_active: Cell::new(false),
                is_eos: Cell::new(false),

                samples: RefCell::new(VecDeque::new()),
                requests: RefCell::new(VecDeque::new()),
            }
            .into())
        }
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
                    this.source().as_impl().request_sample()?;
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
        Ok(self.stream_descriptor.clone())
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
