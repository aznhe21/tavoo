use super::time::UnixTime;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stream {
    stream_type: u8,
    component_tag: Option<u8>,
}

impl From<&isdb::filters::sorter::Stream> for Stream {
    #[inline]
    fn from(stream: &isdb::filters::sorter::Stream) -> Stream {
        Stream {
            stream_type: stream.stream_type().0,
            component_tag: stream.component_tag(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtendedEventItem {
    item: String,
    description: String,
}

impl From<&isdb::filters::sorter::ExtendedEventItem> for ExtendedEventItem {
    fn from(item: &isdb::filters::sorter::ExtendedEventItem) -> ExtendedEventItem {
        ExtendedEventItem {
            item: item.item.to_string(Default::default()),
            description: item.description.to_string(Default::default()),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoComponent {
    stream_content: u8,
    component_type: u8,
    component_tag: u8,
    lang_code: String,
    text: String,
}

impl From<&isdb::filters::sorter::VideoComponent> for VideoComponent {
    fn from(component: &isdb::filters::sorter::VideoComponent) -> VideoComponent {
        VideoComponent {
            stream_content: component.stream_content,
            component_type: component.component_type,
            component_tag: component.component_tag,
            lang_code: component.lang_code.to_string(),
            text: component.text.to_string(Default::default()),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioComponent {
    stream_content: u8,
    component_type: u8,
    component_tag: u8,
    stream_type: u8,
    simulcast_group_tag: u8,
    main_component_flag: bool,
    quality_indicator: u8,
    sampling_rate: u16,
    lang_code: String,
    lang_code_2: Option<String>,
    text: String,
}

impl From<&isdb::filters::sorter::AudioComponent> for AudioComponent {
    fn from(component: &isdb::filters::sorter::AudioComponent) -> AudioComponent {
        use isdb::psi::desc::{QualityIndicator, SamplingFrequency};

        AudioComponent {
            stream_content: component.stream_content,
            component_type: component.component_type,
            component_tag: component.component_tag,
            stream_type: component.stream_type.0,
            simulcast_group_tag: component.simulcast_group_tag,
            main_component_flag: component.main_component_flag,
            quality_indicator: match component.quality_indicator {
                QualityIndicator::Reserved => 0,
                QualityIndicator::Mode1 => 1,
                QualityIndicator::Mode2 => 2,
                QualityIndicator::Mode3 => 3,
            },
            sampling_rate: match component.sampling_rate {
                SamplingFrequency::Reserved => 0,
                SamplingFrequency::SF16k => 1600,
                SamplingFrequency::SF22_05k => 2205,
                SamplingFrequency::SF24k => 2400,
                SamplingFrequency::SF32k => 3200,
                SamplingFrequency::SF44_1k => 4410,
                SamplingFrequency::SF48k => 4800,
            },
            lang_code: component.lang_code.to_string(),
            lang_code_2: component.lang_code_2.map(|code| code.to_string()),
            text: component.text.to_string(Default::default()),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentGenre {
    pub large_genre_classification: u8,
    pub middle_genre_classification: u8,
    pub user_genre_1: u8,
    pub user_genre_2: u8,
}

impl From<&isdb::psi::desc::ContentGenre> for ContentGenre {
    fn from(genre: &isdb::psi::desc::ContentGenre) -> ContentGenre {
        ContentGenre {
            large_genre_classification: genre.large_genre_classification,
            middle_genre_classification: genre.middle_genre_classification,
            user_genre_1: genre.user_genre_1,
            user_genre_2: genre.user_genre_2,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    event_id: u16,
    // 日本時間だけな上精度は秒なのでUNIX時間で特に問題は無い
    start_time: UnixTime,
    duration: u32,
    name: Option<String>,
    text: Option<String>,
    extended_items: Vec<ExtendedEventItem>,
    video_components: Vec<VideoComponent>,
    audio_components: Vec<AudioComponent>,
    genres: Option<Vec<ContentGenre>>,
}

impl From<&isdb::filters::sorter::EventInfo> for Event {
    fn from(event: &isdb::filters::sorter::EventInfo) -> Event {
        Event {
            event_id: event.event_id.get(),
            start_time: event.start_time.into(),
            duration: event.duration,
            name: event.name.as_ref().map(|s| s.to_string(Default::default())),
            text: event.text.as_ref().map(|s| s.to_string(Default::default())),
            extended_items: event.extended_items.iter().map(Into::into).collect(),
            video_components: event.video_components.iter().map(Into::into).collect(),
            audio_components: event.audio_components.iter().map(Into::into).collect(),
            genres: event
                .genres
                .as_deref()
                .map(|genres| genres.iter().map(Into::into).collect()),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Service {
    service_id: u16,
    is_oneseg: bool,
    video_streams: Vec<Stream>,
    audio_streams: Vec<Stream>,
    provider_name: String,
    service_name: String,
    present_event: Option<Event>,
    following_event: Option<Event>,
}

impl From<&isdb::filters::sorter::Service> for Service {
    #[inline]
    fn from(service: &isdb::filters::sorter::Service) -> Service {
        Service {
            service_id: service.service_id().get(),
            is_oneseg: service.is_oneseg(),
            video_streams: service.video_streams().iter().map(Into::into).collect(),
            audio_streams: service.audio_streams().iter().map(Into::into).collect(),
            provider_name: service.provider_name().to_string(Default::default()),
            service_name: service.service_name().to_string(Default::default()),
            present_event: service.present_event().map(Into::into),
            following_event: service.following_event().map(Into::into),
        }
    }
}
