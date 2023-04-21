use std::time::Duration;

use parking_lot::{Mutex, MutexGuard};
use windows::core::{self as C, implement, AsImpl, ComInterface, Result as WinResult};
use windows::Win32::Foundation as F;
use windows::Win32::Media::KernelStreaming::GUID_NULL;
use windows::Win32::Media::MediaFoundation as MF;

use crate::extract::ExtractHandler;
use crate::sys::com::PropVariant;
use crate::sys::wrap;

use super::dummy;
use super::queue::AsyncQueue;
use super::stream::ElementaryStream;

struct PresentationDescriptor(MF::IMFPresentationDescriptor);
// Safety: C++のサンプルではスレッドをまたいで使っているので安全なはず
unsafe impl Send for PresentationDescriptor {}

// IMFStreamDescriptorのストリーム識別子。
const SID_VIDEO: u32 = 0;
const SID_AUDIO: u32 = 1;

fn create_video_sd(stream: &isdb::filters::sorter::Stream) -> WinResult<MF::IMFStreamDescriptor> {
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

        let stream_descriptor =
            MF::MFCreateStreamDescriptor(SID_VIDEO, &[Some(media_type.clone())])?;

        let handler = stream_descriptor.GetMediaTypeHandler()?;
        handler.SetCurrentMediaType(&media_type)?;

        Ok(stream_descriptor)
    }
}

fn create_audio_sd(stream: &isdb::filters::sorter::Stream) -> WinResult<MF::IMFStreamDescriptor> {
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

        let stream_descriptor =
            MF::MFCreateStreamDescriptor(SID_AUDIO, &[Some(media_type.clone())])?;

        let handler = stream_descriptor.GetMediaTypeHandler()?;
        handler.SetCurrentMediaType(&media_type)?;

        Ok(stream_descriptor)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Stopped,
    Paused,
    Started,
    Shutdown,
}

#[derive(Debug, Clone)]
pub struct TransportStream(MF::IMFMediaSource);

// Safety: 内包するIMFMediaSourceはOuterであり、OuterはSendであるため安全
unsafe impl Send for TransportStream {}

impl TransportStream {
    pub fn new(
        handler: ExtractHandler,
        video_stream: &isdb::filters::sorter::Stream,
        audio_stream: &isdb::filters::sorter::Stream,
    ) -> WinResult<TransportStream> {
        unsafe {
            let video_sd = create_video_sd(video_stream)?;
            let audio_sd = create_audio_sd(audio_stream)?;
            let presentation_descriptor = MF::MFCreatePresentationDescriptor(Some(&[
                Some(video_sd.clone()),
                Some(audio_sd.clone()),
            ]))?;
            presentation_descriptor.SelectStream(SID_VIDEO)?;
            presentation_descriptor.SelectStream(SID_AUDIO)?;
            if let Some(duration) = handler.duration() {
                presentation_descriptor
                    .SetUINT64(&MF::MF_PD_DURATION, (duration.as_nanos() / 100) as u64)?;
            }

            let event_queue = MF::MFCreateEventQueue()?;
            let dummy_stream: MF::IMFMediaStream = dummy::DummyStream.into();

            let inner = Mutex::new(Inner {
                handler,

                state: State::Stopped,

                event_queue,
                presentation_descriptor,
                video_stream: dummy_stream.clone(),
                audio_stream: dummy_stream,

                rate: 1.,
                pending_eos: 0,
            });
            let this = TransportStream(
                Outer {
                    inner,
                    queue: AsyncQueue::new(),
                }
                .into(),
            );

            let video_stream = ElementaryStream::new(&this, video_sd)?;
            let audio_stream = ElementaryStream::new(&this, audio_sd)?;

            {
                let mut inner = this.outer().inner.lock();
                inner.video_stream = video_stream.intf().clone();
                inner.audio_stream = audio_stream.intf().clone();
            }

            Ok(this)
        }
    }

    #[inline]
    pub unsafe fn from_source(source: MF::IMFMediaSource) -> TransportStream {
        TransportStream(source)
    }

    #[inline]
    pub fn intf(&self) -> &MF::IMFMediaSource {
        &self.0
    }

    #[inline]
    fn outer(&self) -> &Outer {
        self.0.as_impl()
    }

    #[inline]
    fn inner(&self) -> parking_lot::MutexGuard<Inner> {
        self.outer().inner.lock()
    }

    pub fn deliver_video_packet(&self, pos: Option<Duration>, payload: &[u8]) {
        let outer = self.outer();
        let r = Inner::deliver_video_packet(&mut outer.inner.lock(), pos, payload);
        if let Err(e) = r {
            log::debug!("error[deliver_video_packet]: {}", e);
            outer.streaming_error(e);
        }
    }

    pub fn deliver_audio_packet(&self, pos: Option<Duration>, payload: &[u8]) {
        let outer = self.outer();
        let r = Inner::deliver_audio_packet(&mut outer.inner.lock(), pos, payload);
        if let Err(e) = r {
            log::debug!("error[deliver_audio_packet]: {}", e);
            outer.streaming_error(e);
        }
    }

    #[inline]
    pub fn request_sample(&self) {
        self.inner().handler.request_es();
    }

    pub fn enqueue_end_of_stream(&self) -> WinResult<()> {
        let outer = self.outer();
        outer.inner.lock().check_shutdown()?;
        outer.enqueue_op(|outer| Inner::end_of_stream(&mut outer.inner.lock()))
    }

    pub fn end_of_mpeg_stream(&self) -> WinResult<()> {
        let mut inner = self.inner();
        inner.check_shutdown()?;
        Inner::end_of_mpeg_stream(&mut inner)
    }

    pub fn streams_need_data(&self) -> bool {
        let mut inner = self.inner();
        inner.state != State::Shutdown && Inner::streams_need_data(&mut inner)
    }

    pub fn streaming_error(&self, error: C::Error) {
        self.outer().streaming_error(error)
    }
}

#[implement(
    MF::IMFGetService,
    MF::IMFMediaSource,
    MF::IMFRateControl,
    MF::IMFRateSupport
)]
struct Outer {
    inner: Mutex<Inner>,
    // ロックする必要がないのでOuter側で持つ
    queue: AsyncQueue,
}

struct Inner {
    handler: ExtractHandler,

    state: State,

    event_queue: MF::IMFMediaEventQueue,
    presentation_descriptor: MF::IMFPresentationDescriptor,
    // ストリーム操作中はロック解除を強制したいため、あえて使いにくい方で格納
    video_stream: MF::IMFMediaStream,
    audio_stream: MF::IMFMediaStream,

    rate: f32,
    pending_eos: usize,
}

// Safety: C++のサンプルではスレッドをまたいで使っているので安全なはず
unsafe impl Send for Inner {}

impl Outer {
    #[inline]
    fn ts(&self) -> TransportStream {
        unsafe { TransportStream(self.cast().unwrap()) }
    }

    fn streaming_error(&self, error: C::Error) {
        let (state, event_queue) = {
            let inner = self.inner.lock();
            (inner.state, inner.event_queue.clone())
        };
        if state != State::Shutdown {
            unsafe {
                let _ = event_queue.QueueEventParamVar(
                    MF::MEError.0 as u32,
                    &GUID_NULL,
                    error.into(),
                    std::ptr::null(),
                );
            }
        }
    }

    #[track_caller]
    fn enqueue_op<F: FnOnce(&Outer) -> WinResult<()> + Send + 'static>(
        &self,
        f: F,
    ) -> WinResult<()> {
        let location = std::panic::Location::caller();

        let ts = self.ts();
        let queue = self.queue.clone();
        self.queue.enqueue(move || {
            let outer = ts.outer();
            let r = f(outer).and_then(|()| queue.process_queue());
            if let Err(e) = r {
                log::debug!("error[enqueue_op]: {} at {}", e, location);
                outer.streaming_error(e);
            }
        })
    }
}

impl Inner {
    fn check_shutdown(&self) -> WinResult<()> {
        if self.state == State::Shutdown {
            Err(MF::MF_E_SHUTDOWN.into())
        } else {
            Ok(())
        }
    }

    #[inline]
    fn video_stream_unlocked<T, F>(this: &mut MutexGuard<Self>, f: F) -> T
    where
        F: FnOnce(ElementaryStream) -> T,
    {
        // Safety: this.video_streamはElementaryStreamから得たもの
        let es = unsafe { ElementaryStream::from_stream(this.video_stream.clone()) };
        MutexGuard::unlocked(this, || f(es))
    }

    #[inline]
    fn audio_stream_unlocked<T, F>(this: &mut MutexGuard<Self>, f: F) -> T
    where
        F: FnOnce(ElementaryStream) -> T,
    {
        // Safety: this.audio_streamはElementaryStreamから得たもの
        let es = unsafe { ElementaryStream::from_stream(this.audio_stream.clone()) };
        MutexGuard::unlocked(this, || f(es))
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

            let all_selected = wrap::wrap2(|a, b| pd.GetStreamDescriptorByIndex(0, a, b))?.0
                && wrap::wrap2(|a, b| pd.GetStreamDescriptorByIndex(1, a, b))?.0;
            if !all_selected {
                return Err(F::E_INVALIDARG.into());
            }

            Ok(())
        }
    }

    fn do_start(
        this: &mut MutexGuard<Self>,
        pd: &MF::IMFPresentationDescriptor,
        start_pos: Option<i64>,
    ) -> WinResult<()> {
        let r: WinResult<()> = 'r: {
            unsafe {
                log::trace!("TransportStream::do_start");

                let start_pos = if let Some(start_pos) = start_pos {
                    this.handler
                        .set_position(Duration::from_nanos((start_pos as u64) * 100));

                    PropVariant::I64(start_pos)
                } else {
                    PropVariant::Empty
                };

                tri!('r, Inner::select_streams(this, pd, Some(&start_pos)));

                this.state = State::Started;

                tri!('r, this.event_queue.QueueEventParamVar(
                    MF::MESourceStarted.0 as u32,
                    &GUID_NULL,
                    F::S_OK,
                    &start_pos.to_raw(),
                ));

                Ok(())
            }
        };
        if let Err(ref e) = r {
            log::debug!("error[do_start]: {}", e);
            unsafe {
                let _ = this.event_queue.QueueEventParamVar(
                    MF::MESourceStarted.0 as u32,
                    &GUID_NULL,
                    e.code(),
                    std::ptr::null(),
                );
            }
        }

        r
    }

    fn do_stop(this: &mut MutexGuard<Self>) -> WinResult<()> {
        unsafe {
            log::trace!("TransportStream::do_stop");

            Inner::video_stream_unlocked(this, |es| es.stop())?;
            Inner::audio_stream_unlocked(this, |es| es.stop())?;

            this.state = State::Stopped;

            this.event_queue.QueueEventParamVar(
                MF::MESourceStopped.0 as u32,
                &GUID_NULL,
                F::S_OK,
                std::ptr::null(),
            )?;

            Ok(())
        }
    }

    fn do_pause(this: &mut MutexGuard<Self>) -> WinResult<()> {
        unsafe {
            log::trace!("TransportStream::do_pause");

            Inner::video_stream_unlocked(this, |es| es.pause())?;
            Inner::audio_stream_unlocked(this, |es| es.pause())?;

            this.state = State::Paused;

            this.event_queue.QueueEventParamVar(
                MF::MESourcePaused.0 as u32,
                &GUID_NULL,
                F::S_OK,
                std::ptr::null(),
            )?;

            Ok(())
        }
    }

    fn end_of_stream(this: &mut MutexGuard<Self>) -> WinResult<()> {
        unsafe {
            log::trace!("TransportStream::end_of_stream");

            this.pending_eos -= 1;
            if this.pending_eos == 0 {
                this.event_queue.QueueEventParamVar(
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
        this: &mut MutexGuard<Self>,
        _pd: &MF::IMFPresentationDescriptor,
        start_pos: Option<&PropVariant>,
    ) -> WinResult<()> {
        macro_rules! activate {
            ($field:ident, $method:ident) => {
                let was_selected = Inner::$method(this, |es| {
                    let was_selected = es.is_active();
                    es.activate(true);
                    was_selected
                });

                if was_selected {
                    log::trace!("TransportStream: MEUpdatedStream");
                    this.event_queue.QueueEventParamUnk(
                        MF::MEUpdatedStream.0 as u32,
                        &GUID_NULL,
                        F::S_OK,
                        &this.$field,
                    )?;
                } else {
                    log::trace!("TransportStream: MENewStream");
                    this.event_queue.QueueEventParamUnk(
                        MF::MENewStream.0 as u32,
                        &GUID_NULL,
                        F::S_OK,
                        &this.$field,
                    )?;
                }

                Inner::$method(this, |es| es.start(start_pos))?;
                this.pending_eos += 1;
            };
        }

        unsafe {
            this.pending_eos = 0;
            activate!(video_stream, video_stream_unlocked);
            activate!(audio_stream, audio_stream_unlocked);
            Ok(())
        }
    }

    fn streams_need_data(this: &mut MutexGuard<Self>) -> bool {
        Inner::video_stream_unlocked(this, |es| es.needs_data())
            || Inner::audio_stream_unlocked(this, |es| es.needs_data())
    }

    fn end_of_mpeg_stream(this: &mut MutexGuard<Self>) -> WinResult<()> {
        Inner::video_stream_unlocked(this, |es| es.end_of_stream())?;
        Inner::audio_stream_unlocked(this, |es| es.end_of_stream())?;
        Ok(())
    }

    fn create_sample(&self, payload: &[u8], pos: Option<Duration>) -> WinResult<MF::IMFSample> {
        unsafe {
            let buffer = MF::MFCreateMemoryBuffer(payload.len() as u32)?;
            let mut data = std::ptr::null_mut();
            buffer.Lock(&mut data, None, None)?;
            std::ptr::copy_nonoverlapping(payload.as_ptr(), data, payload.len());
            buffer.Unlock()?;
            buffer.SetCurrentLength(payload.len() as u32)?;

            let sample = MF::MFCreateSample()?;
            sample.AddBuffer(&buffer)?;

            if let Some(pos) = pos {
                sample.SetSampleTime((pos.as_nanos() / 100) as i64)?;
            }

            Ok(sample)
        }
    }

    fn deliver_video_packet(
        this: &mut MutexGuard<Self>,
        pos: Option<Duration>,
        payload: &[u8],
    ) -> WinResult<()> {
        let sample = this.create_sample(payload, pos)?;
        Inner::video_stream_unlocked(this, |es| es.deliver_payload(sample))
    }

    fn deliver_audio_packet(
        this: &mut MutexGuard<Self>,
        pos: Option<Duration>,
        payload: &[u8],
    ) -> WinResult<()> {
        let sample = this.create_sample(payload, pos)?;
        Inner::audio_stream_unlocked(this, |es| es.deliver_payload(sample))
    }
}

#[allow(non_snake_case)]
impl MF::IMFGetService_Impl for Outer {
    fn GetService(
        &self,
        sid: *const windows::core::GUID,
        iid: *const windows::core::GUID,
        ppv: *mut *mut core::ffi::c_void,
    ) -> WinResult<()> {
        unsafe {
            use windows::core::Interface;

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
impl MF::IMFMediaEventGenerator_Impl for Outer {
    fn GetEvent(
        &self,
        dwflags: MF::MEDIA_EVENT_GENERATOR_GET_EVENT_FLAGS,
    ) -> WinResult<MF::IMFMediaEvent> {
        unsafe {
            log::trace!("TransportStream::GetEvent");

            let queue = {
                let inner = self.inner.lock();
                inner.check_shutdown()?;
                inner.event_queue.clone()
            };
            queue.GetEvent(dwflags.0)
        }
    }

    fn BeginGetEvent(
        &self,
        pcallback: Option<&MF::IMFAsyncCallback>,
        punkstate: Option<&C::IUnknown>,
    ) -> WinResult<()> {
        unsafe {
            log::trace!("TransportStream::BeginGetEvent");

            let inner = self.inner.lock();
            inner.check_shutdown()?;
            inner.event_queue.BeginGetEvent(pcallback, punkstate)
        }
    }

    fn EndGetEvent(&self, presult: Option<&MF::IMFAsyncResult>) -> WinResult<MF::IMFMediaEvent> {
        unsafe {
            log::trace!("TransportStream::EndGetEvent");

            let inner = self.inner.lock();
            inner.check_shutdown()?;
            inner.event_queue.EndGetEvent(presult)
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

            let inner = self.inner.lock();
            inner.check_shutdown()?;
            inner
                .event_queue
                .QueueEventParamVar(met, guidextendedtype, hrstatus, pvvalue)
        }
    }
}

#[allow(non_snake_case)]
impl MF::IMFMediaSource_Impl for Outer {
    fn GetCharacteristics(&self) -> WinResult<u32> {
        log::trace!("TransportStream::GetCharacteristics");

        let inner = self.inner.lock();
        inner.check_shutdown()?;

        // TODO: リアルタイム視聴では0？
        Ok(MF::MFMEDIASOURCE_CAN_PAUSE.0 as u32)
    }

    fn CreatePresentationDescriptor(&self) -> WinResult<MF::IMFPresentationDescriptor> {
        unsafe {
            log::trace!("TransportStream::CreatePresentationDescriptor");

            let inner = self.inner.lock();
            inner.check_shutdown()?;

            let pd = inner.presentation_descriptor.Clone()?;
            Ok(pd)
        }
    }

    fn Start(
        &self,
        pd: Option<&MF::IMFPresentationDescriptor>,
        time_format: *const C::GUID,
        start_pos: *const windows::Win32::System::Com::StructuredStorage::PROPVARIANT,
    ) -> WinResult<()> {
        unsafe {
            log::trace!(
                "TransportStream::Start: pd={:?}, time_format={:?}, start_pos={:?}",
                pd,
                time_format.as_ref(),
                start_pos.as_ref().and_then(PropVariant::new),
            );

            let inner = self.inner.lock();

            let pd = pd.ok_or(F::E_INVALIDARG)?;
            let Some(start_pos) = start_pos.as_ref() else {
                return Err(F::E_INVALIDARG.into());
            };
            if !time_format.is_null() && *time_format != GUID_NULL {
                return Err(MF::MF_E_UNSUPPORTED_TIME_FORMAT.into());
            }
            let start_pos = match PropVariant::new(start_pos) {
                Some(PropVariant::Empty) => None,

                Some(PropVariant::I64(v)) => {
                    if inner.state != State::Stopped {
                        return Err(MF::MF_E_INVALIDREQUEST.into());
                    }

                    Some(v)
                }

                _ => return Err(MF::MF_E_UNSUPPORTED_TIME_FORMAT.into()),
            };

            inner.check_shutdown()?;
            inner.validate_presentation_descriptor(pd)?;

            let pd = PresentationDescriptor(pd.clone());
            self.enqueue_op(move |outer| {
                let pd = pd;
                Inner::do_start(&mut outer.inner.lock(), &pd.0, start_pos)
            })?;

            Ok(())
        }
    }

    fn Stop(&self) -> WinResult<()> {
        log::trace!("TransportStream::Stop");

        let inner = self.inner.lock();
        inner.check_shutdown()?;

        self.enqueue_op(move |outer| Inner::do_stop(&mut outer.inner.lock()))?;

        Ok(())
    }

    fn Pause(&self) -> WinResult<()> {
        log::trace!("TransportStream::Pause");

        let inner = self.inner.lock();
        inner.check_shutdown()?;

        self.enqueue_op(move |outer| Inner::do_pause(&mut outer.inner.lock()))?;

        Ok(())
    }

    fn Shutdown(&self) -> WinResult<()> {
        unsafe {
            log::trace!("TransportStream::Shutdown");

            let mut inner = self.inner.lock();
            inner.check_shutdown()?;

            let _ = Inner::video_stream_unlocked(&mut inner, |es| es.shutdown());
            let _ = Inner::audio_stream_unlocked(&mut inner, |es| es.shutdown());
            let _ = inner.event_queue.Shutdown();

            inner.state = State::Shutdown;
            Ok(())
        }
    }
}

#[allow(non_snake_case)]
impl MF::IMFRateControl_Impl for Outer {
    fn SetRate(&self, thin: F::BOOL, rate: f32) -> WinResult<()> {
        unsafe {
            log::trace!("TransportStream::SetRate");

            let mut inner = self.inner.lock();
            inner.check_shutdown()?;

            // TODO: リアルタイム視聴では速度変更不可

            if rate < 0. {
                return Err(MF::MF_E_REVERSE_UNSUPPORTED.into());
            }
            if thin.as_bool() {
                return Err(MF::MF_E_THINNING_UNSUPPORTED.into());
            }

            inner.rate = rate;

            inner.event_queue.QueueEventParamVar(
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

            let inner = self.inner.lock();
            inner.check_shutdown()?;

            if let Some(thin) = thin.as_mut() {
                *thin = F::FALSE;
            }
            if let Some(rate) = rate.as_mut() {
                *rate = inner.rate;
            }

            Ok(())
        }
    }
}

#[allow(non_snake_case)]
impl MF::IMFRateSupport_Impl for Outer {
    fn GetSlowestRate(
        &self,
        dir: MF::MFRATE_DIRECTION,
        thin: F::BOOL,
    ) -> windows::core::Result<f32> {
        log::trace!("TransportStream::GetSlowestRate");

        let inner = self.inner.lock();
        inner.check_shutdown()?;

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

        let inner = self.inner.lock();
        inner.check_shutdown()?;

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

            let inner = self.inner.lock();
            inner.check_shutdown()?;

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
