//! 記述子。

use std::fmt;

use crate::pid::Pid;
use crate::time::{DateTime, MjdDate};
use crate::types::{ServiceType, StreamType};
use crate::utils::{BytesExt, SliceExt};

/// 記述子を表すトレイト。
pub trait Descriptor<'a>: Sized {
    /// この記述子のタグ。
    const TAG: u8;

    /// `data`から記述子を読み取る。
    ///
    /// `data`には`descriptor_tag`と`descriptor_length`は含まない。
    fn read(data: &'a [u8]) -> Option<Self>;
}

/// パース前の記述子。
pub struct RawDescriptor<'a> {
    /// 記述子のタグ。
    pub tag: u8,

    /// 記述子の内容。
    pub data: &'a [u8],
}

impl<'a> fmt::Debug for RawDescriptor<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        struct PrintBytes<'a>(&'a [u8]);
        impl<'a> fmt::Debug for PrintBytes<'a> {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "{} bytes", self.0.len())
            }
        }

        f.debug_struct("RawDescriptor")
            .field("tag", &crate::utils::UpperHex(self.tag))
            .field("data", &PrintBytes(self.data))
            .finish()
    }
}

/// 複数の記述子からなる記述子群。
#[derive(Clone)]
pub struct DescriptorBlock<'a>(&'a [u8]);

impl<'a> DescriptorBlock<'a> {
    /// 複数の記述子を含む`data`から`DescriptorBlock`を生成する。
    ///
    /// `block`の中身は`get`で初めてパースされる。
    #[inline]
    pub fn new(block: &'a [u8]) -> DescriptorBlock<'a> {
        DescriptorBlock(block)
    }

    /// 内包する記述子群のイテレーターを返す。
    #[inline]
    pub fn iter(&self) -> DescriptorIter<'a> {
        DescriptorIter(self.0)
    }

    /// 内包する記述子群から`T`のタグと一致する記述子を読み取って返す。
    ///
    /// `T`のタグと一致する記述子がない場合は`None`を返す。
    pub fn get<T: Descriptor<'a>>(&self) -> Option<T> {
        self.iter()
            .find(|d| d.tag == T::TAG)
            .and_then(|d| T::read(d.data))
    }

    /// 内包する記述子群から`T`のタグと一致する記述子をすべて読み取って返す。
    pub fn get_all<T: Descriptor<'a>>(&self) -> impl Iterator<Item = T> + 'a {
        self.iter().filter_map(|d| {
            if d.tag == T::TAG {
                T::read(d.data)
            } else {
                None
            }
        })
    }
}

impl<'a> fmt::Debug for DescriptorBlock<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("DescriptorBlock(")?;
        f.debug_list().entries(self).finish()?;
        f.write_str(")")
    }
}

impl<'a> IntoIterator for &DescriptorBlock<'a> {
    type Item = RawDescriptor<'a>;
    type IntoIter = DescriptorIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// [`DescriptorBlock`]のイテレーター。
#[derive(Clone)]
pub struct DescriptorIter<'a>(&'a [u8]);

impl<'a> Iterator for DescriptorIter<'a> {
    type Item = RawDescriptor<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let [tag, length, ref rem @ ..] = *self.0 else {
            return None;
        };
        let Some((data, tail)) = rem.split_at_checked(length as usize) else {
            return None;
        };

        self.0 = tail;
        Some(RawDescriptor { tag, data })
    }
}

impl<'a> std::iter::FusedIterator for DescriptorIter<'a> {}

impl<'a> fmt::Debug for DescriptorIter<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DescriptorIter(")?;
        f.debug_list().entries(self.clone()).finish()?;
        f.write_str(")")
    }
}

/// 限定受信方式記述子。
#[derive(Debug)]
pub struct ConditionalAccessDescriptor<'a> {
    /// CA_system_id
    pub ca_system_id: u16,
    /// CA_PID
    pub ca_pid: Pid,
    /// private_data_byte
    pub private_data: &'a [u8],
}

impl<'a> Descriptor<'a> for ConditionalAccessDescriptor<'a> {
    const TAG: u8 = 0x09;

    fn read(data: &'a [u8]) -> Option<ConditionalAccessDescriptor<'a>> {
        if data.len() < 4 {
            log::debug!("invalid ConditionalAccessDescriptor");
            return None;
        }

        let ca_system_id = data[0..=1].read_be_16();
        let ca_pid = Pid::read(&data[2..]);
        let private_data = &data[4..];

        Some(ConditionalAccessDescriptor {
            ca_system_id,
            ca_pid,
            private_data,
        })
    }
}

/// ネットワーク名記述子。
#[derive(Debug)]
pub struct NetworkNameDescriptor<'a> {
    /// network name
    pub network_name: &'a [u8],
}

impl<'a> Descriptor<'a> for NetworkNameDescriptor<'a> {
    const TAG: u8 = 0x40;

    fn read(data: &'a [u8]) -> Option<NetworkNameDescriptor<'a>> {
        Some(NetworkNameDescriptor { network_name: data })
    }
}

/// [`ServiceListDescriptor`]における`services`の要素。
#[derive(Debug)]
pub struct ServiceEntry {
    /// Service id
    pub service_id: u16,
    /// Service type
    pub service_type: ServiceType,
}

/// サービスリスト記述子。
#[derive(Debug)]
pub struct ServiceListDescriptor {
    /// [`ServiceEntry`]の配列。
    pub services: Vec<ServiceEntry>,
}

impl Descriptor<'_> for ServiceListDescriptor {
    const TAG: u8 = 0x41;

    fn read(data: &[u8]) -> Option<ServiceListDescriptor> {
        let services = data
            .chunks_exact(3)
            .map(|chunk| {
                let service_id = chunk[0..=1].read_be_16();
                let service_type = ServiceType(chunk[2]);
                ServiceEntry {
                    service_id,
                    service_type,
                }
            })
            .collect();

        Some(ServiceListDescriptor { services })
    }
}

/// 衛星分配システム記述子。
#[derive(Debug)]
pub struct SatelliteDeliverySystemDescriptor {
    /// frequency coded in 10kHz
    pub frequency: u32,
    /// orbital_position
    pub orbital_position: u16,
    /// west_east_flag
    pub west_east_flag: bool,
    /// polarization (2bit)
    pub polarization: u8,
    /// modulation (5bit)
    pub modulation: u8,
    /// system_rate (28bit)
    pub system_rate: u32,
    /// FEC_inner (4bit)
    pub fec_inner: u8,
}

impl Descriptor<'_> for SatelliteDeliverySystemDescriptor {
    const TAG: u8 = 0x43;

    fn read(data: &[u8]) -> Option<SatelliteDeliverySystemDescriptor> {
        if data.len() != 11 {
            log::debug!("invalid SatelliteDeliverySystemDescriptor");
            return None;
        }

        let frequency = data[0..=3].read_bcd(8);
        let orbital_position = data[4..=5].read_bcd(4);
        let west_east_flag = data[6] & 0b10000000 != 0;
        let polarization = (data[6] & 0b01100000) >> 5;
        let modulation = data[6] & 0b00011111;
        let system_rate = data[7..=10].read_bcd(7);
        let fec_inner = data[10] & 0b00001111;

        Some(SatelliteDeliverySystemDescriptor {
            frequency,
            orbital_position,
            west_east_flag,
            polarization,
            modulation,
            system_rate,
            fec_inner,
        })
    }
}

/// 有線分配システム記述子。
#[derive(Debug)]
pub struct CableDeliverySystemDescriptor {
    /// frequency coded in 100Hz
    pub frequency: u32,
    /// frame_type
    pub frame_type: u8,
    /// FEC_outer (4bit)
    pub fec_outer: u8,
    /// modulation
    pub modulation: u8,
    /// symbol_rate (28bit)
    pub symbol_rate: u32,
    /// FEC_inner (4bit)
    pub fec_inner: u8,
}

impl Descriptor<'_> for CableDeliverySystemDescriptor {
    const TAG: u8 = 0x44;

    fn read(data: &[u8]) -> Option<CableDeliverySystemDescriptor> {
        if data.len() != 11 {
            log::debug!("invalid CableDeliverySystemDescriptor");
            return None;
        }

        let frequency = data[0..=3].read_bcd(8);
        let frame_type = (data[5] & 0b11110000) >> 4;
        let fec_outer = data[5] & 0b00001111;
        let modulation = data[6];
        let symbol_rate = data[7..=10].read_bcd(7);
        let fec_inner = data[10] & 0b00001111;

        Some(CableDeliverySystemDescriptor {
            frequency,
            frame_type,
            fec_outer,
            modulation,
            symbol_rate,
            fec_inner,
        })
    }
}

/// サービス記述子。
#[derive(Debug)]
pub struct ServiceDescriptor<'a> {
    /// service_type
    pub service_type: ServiceType,
    /// service provider name
    pub service_provider_name: &'a [u8],
    /// service name
    pub service_name: &'a [u8],
}

impl<'a> Descriptor<'a> for ServiceDescriptor<'a> {
    const TAG: u8 = 0x48;

    fn read(data: &'a [u8]) -> Option<ServiceDescriptor<'a>> {
        let [service_type, service_provider_name_length, ref data @ ..] = *data else {
            log::debug!("invalid ServiceDescriptor");
            return None;
        };
        let Some((service_provider_name, data)) = data
            .split_at_checked(service_provider_name_length as usize)
        else {
            log::debug!("invalid ServiceDescriptor::service_provider_name");
            return None;
        };
        let [service_name_length, ref service_name @ ..] = *data else {
            log::debug!("invalid ServiceDescriptor::service_name_length");
            return None;
        };
        if service_name.len() != service_name_length as usize {
            log::debug!("invalid ServiceDescriptor::service_name");
            return None;
        }

        Some(ServiceDescriptor {
            service_type: ServiceType(service_type),
            service_provider_name,
            service_name,
        })
    }
}

/// リンク記述子。
#[derive(Debug)]
pub struct LinkageDescriptor<'a> {
    /// transport_stream_id
    pub transport_stream_id: u16,
    /// original_network_id
    pub original_network_id: u16,
    /// service_id
    pub service_id: u16,
    /// linkage_type
    pub linkage_type: u8,
    /// private_data_byte
    pub private_data: &'a [u8],
}

impl<'a> Descriptor<'a> for LinkageDescriptor<'a> {
    const TAG: u8 = 0x4A;

    fn read(data: &'a [u8]) -> Option<LinkageDescriptor<'a>> {
        if data.len() < 7 {
            log::debug!("invalid LinkageDescriptor");
            return None;
        }

        let transport_stream_id = data[0..=1].read_be_16();
        let original_network_id = data[2..=3].read_be_16();
        let service_id = data[4..=5].read_be_16();
        let linkage_type = data[6];
        let private_data = &data[7..];

        Some(LinkageDescriptor {
            transport_stream_id,
            original_network_id,
            service_id,
            linkage_type,
            private_data,
        })
    }
}

/// 短形式イベント記述子。
#[derive(Debug)]
pub struct ShortEventDescriptor<'a> {
    /// ISO 639-2で規定される3文字の国名コード。
    pub lang_code: [u8; 3],
    /// event_name_char
    pub event_name: &'a [u8],
    /// text_char
    pub text: &'a [u8],
}

impl<'a> Descriptor<'a> for ShortEventDescriptor<'a> {
    const TAG: u8 = 0x4D;

    fn read(data: &'a [u8]) -> Option<ShortEventDescriptor<'a>> {
        if data.len() < 4 {
            log::debug!("invalid ShortEventDescriptor");
            return None;
        }

        let lang_code = data[0..=2].try_into().unwrap();
        let event_name_length = data[3];
        let Some((event_name, data)) = data[4..].split_at_checked(event_name_length as usize) else {
            log::debug!("invalid ShortEventDescriptor::event_name");
            return None;
        };
        let [text_length, ref text @ ..] = *data else {
            log::debug!("invalid ShortEventDescriptor::text_length");
            return None;
        };
        if text.len() != text_length as usize {
            log::debug!("invalid ShortEventDescriptor::text");
            return None;
        }

        Some(ShortEventDescriptor {
            lang_code,
            event_name,
            text,
        })
    }
}

/// [`ExtendedEventDescriptor`]における`items`の要素。
#[derive(Debug)]
pub struct ExtendedEventItem<'a> {
    /// item_description_char
    pub item_description: &'a [u8],
    /// item_char
    pub item: &'a [u8],
}

/// 拡張形式イベント記述子。
#[derive(Debug)]
pub struct ExtendedEventDescriptor<'a> {
    /// descriptor_number (4bit)
    pub descriptor_number: u8,
    /// last_descriptor_number (4bit)
    pub last_descriptor_number: u8,
    /// ISO 639-2で規定される3文字の国名コード。
    pub lang_code: [u8; 3],
    /// [`ExtendedEventItem`]の配列。
    pub items: Vec<ExtendedEventItem<'a>>,
    /// text_char
    pub text: &'a [u8],
}

impl<'a> Descriptor<'a> for ExtendedEventDescriptor<'a> {
    const TAG: u8 = 0x4E;

    fn read(data: &'a [u8]) -> Option<ExtendedEventDescriptor<'a>> {
        if data.len() < 5 {
            log::debug!("invalid ExtendedEventDescriptor");
            return None;
        }

        let descriptor_number = (data[0] & 0b11110000) >> 4;
        let last_descriptor_number = data[0] & 0b00001111;
        let lang_code = data[1..=3].try_into().unwrap();
        let length_of_items = data[4];

        let mut data = &data[5..];
        let mut items = Vec::with_capacity(length_of_items as usize);
        for _ in 0..length_of_items {
            let [item_description_length, ref rem @ ..] = *data else {
                log::debug!("invalid ExtendedEventDescriptor::item_description_length");
                return None;
            };
            let Some((item_description, rem)) = rem
                .split_at_checked(item_description_length as usize)
            else {
                log::debug!("invalid ExtendedEventDescriptor::item_description");
                return None;
            };

            let [item_length, ref rem @ ..] = *rem else {
                log::debug!("invalid ExtendedEventDescriptor::item_length");
                return None;
            };
            let Some((item, rem)) = rem.split_at_checked(item_length as usize) else {
                log::debug!("invalid ExtendedEventDescriptor::item");
                return None;
            };
            data = rem;

            items.push(ExtendedEventItem {
                item_description,
                item,
            });
        }

        let [text_length, ref text @ ..] = *data else {
            log::debug!("invalid ExtendedEventDescriptor::text_length");
            return None;
        };
        if text.len() != text_length as usize {
            log::debug!("invalid ExtendedEventDescriptor::text");
            return None;
        }

        Some(ExtendedEventDescriptor {
            descriptor_number,
            last_descriptor_number,
            lang_code,
            items,
            text,
        })
    }
}

/// コンポーネント記述子。
#[derive(Debug)]
pub struct ComponentDescriptor<'a> {
    /// stream_content (4bit)
    pub stream_content: u8,
    /// component_type
    pub component_type: u8,
    /// component_tag
    pub component_tag: u8,
    /// ISO 639-2で規定される3文字の国名コード。
    pub lang_code: [u8; 3],
    /// text_char
    pub text: &'a [u8],
}

impl<'a> Descriptor<'a> for ComponentDescriptor<'a> {
    const TAG: u8 = 0x50;

    fn read(data: &'a [u8]) -> Option<ComponentDescriptor<'a>> {
        if data.len() < 6 {
            log::debug!("invalid ComponentDescriptor");
            return None;
        }

        let stream_content = data[0] & 0b00001111;
        let component_type = data[1];
        let component_tag = data[2];
        let lang_code = data[3..=5].try_into().unwrap();
        let text = &data[6..];

        Some(ComponentDescriptor {
            stream_content,
            component_type,
            component_tag,
            lang_code,
            text,
        })
    }
}

/// ストリーム識別記述子。
#[derive(Debug)]
pub struct StreamIdDescriptor {
    /// component_tag
    pub component_tag: u8,
}

impl Descriptor<'_> for StreamIdDescriptor {
    const TAG: u8 = 0x52;

    fn read(data: &[u8]) -> Option<StreamIdDescriptor> {
        let [component_tag] = *data else {
            log::debug!("invalid StreamIdDescriptor");
            return None;
        };

        Some(StreamIdDescriptor { component_tag })
    }
}

/// コンテント分類。
#[derive(Debug)]
pub struct ContentGenre {
    /// content_nibble_level_1 (4bit)
    pub large_genre_classification: u8,
    /// content_nibble_level_2 (4bit)
    pub middle_genre_classification: u8,
    /// user_nibble (4bit)
    pub user_genre_1: u8,
    /// user_nibble (4bit)
    pub user_genre_2: u8,
}

/// コンテント記述子。
#[derive(Debug)]
pub struct ContentDescriptor {
    /// [`ContentGenre`]の配列。
    pub genres: Vec<ContentGenre>,
}

impl Descriptor<'_> for ContentDescriptor {
    const TAG: u8 = 0x54;

    fn read(data: &[u8]) -> Option<ContentDescriptor> {
        // genresは7要素以下
        if data.len() > 2 * 7 {
            log::debug!("invalid ContentDescriptor");
            return None;
        }

        let genres = data
            .chunks_exact(2)
            .map(|chunk| {
                let large_genre_classification = (chunk[0] & 0b11110000) >> 4;
                let middle_genre_classification = chunk[0] & 0b00001111;
                let user_genre_1 = (chunk[1] & 0b11110000) >> 4;
                let user_genre_2 = chunk[1] & 0b00001111;

                ContentGenre {
                    large_genre_classification,
                    middle_genre_classification,
                    user_genre_1,
                    user_genre_2,
                }
            })
            .collect();

        Some(ContentDescriptor { genres })
    }
}

/// ローカル時間オフセット。
#[derive(Debug)]
pub struct LocalTimeOffsetEntry {
    /// ISO 3166-1で規定される3文字の国名コード。
    pub country_code: [u8; 3],
    /// country_region_id (6bit)
    pub country_region_id: u8,
    /// local_time_offset_polarity
    pub local_time_offset_polarity: bool,
    /// local_time_offset
    pub local_time_offset: u16,
    /// time_of_change
    pub time_of_change: DateTime,
    /// next_time_offset
    pub next_time_offset: u16,
}

/// ローカル時間オフセット記述子。
#[derive(Debug)]
pub struct LocalTimeOffsetDescriptor {
    /// [`LocalTimeOffsetEntry`]の配列。
    pub time_offsets: Vec<LocalTimeOffsetEntry>,
}

impl Descriptor<'_> for LocalTimeOffsetDescriptor {
    const TAG: u8 = 0x58;

    fn read(data: &[u8]) -> Option<LocalTimeOffsetDescriptor> {
        let time_offsets = data
            .chunks_exact(13)
            .map(|chunk| {
                let country_code = chunk[0..=2].try_into().unwrap();
                let country_region_id = (chunk[3] & 0b11111100) >> 2;
                let local_time_offset_polarity = (chunk[3] & 0b00000001) != 0;
                let local_time_offset = chunk[4..=5].read_be_16();
                let time_of_change = DateTime::read(chunk[6..=10].try_into().unwrap());
                let next_time_offset = chunk[11..=12].read_be_16();

                LocalTimeOffsetEntry {
                    country_code,
                    country_region_id,
                    local_time_offset_polarity,
                    local_time_offset,
                    time_of_change,
                    next_time_offset,
                }
            })
            .collect();

        Some(LocalTimeOffsetDescriptor { time_offsets })
    }
}

/// 階層伝送記述子。
#[derive(Debug)]
pub struct HierarchicalTransmissionDescriptor {
    /// quality_level
    pub high_quality: bool,
    /// reference_PID
    pub reference_pid: Pid,
}

impl Descriptor<'_> for HierarchicalTransmissionDescriptor {
    const TAG: u8 = 0xC0;

    fn read(data: &[u8]) -> Option<HierarchicalTransmissionDescriptor> {
        if data.len() != 3 {
            log::debug!("invalid HierarchicalTransmissionDescriptor");
            return None;
        }

        let high_quality = data[0] & 0b00000001 != 0;
        let reference_pid = Pid::read(&data[1..]);

        Some(HierarchicalTransmissionDescriptor {
            high_quality,
            reference_pid,
        })
    }
}

/// digital copy control information
#[derive(Debug)]
pub struct ComponentControlEntry {
    /// component_tag
    pub component_tag: u8,
    /// digital_recording_control_data (2bit)
    pub digital_recording_control_data: u8,
    /// copy_control_type (2bit)
    pub copy_control_type: u8,
    /// APS_control_data (2bit)
    pub aps_control_data: Option<u8>,
    /// maximum_bitrate
    pub maximum_bitrate: Option<u8>,
}

/// デジタルコピー制御記述子。
#[derive(Debug)]
pub struct DigitalCopyControlDescriptor {
    /// digital_recording_control_data (2bit)
    pub digital_recording_control_data: u8,
    /// copy_control_type (2bit)
    pub copy_control_type: u8,
    /// APS_control_data (2bit)
    pub aps_control_data: Option<u8>,
    /// maximum_bitrate
    pub maximum_bitrate: Option<u8>,
    /// digital copy control information in each component consisting event
    pub component_controls: Option<Vec<ComponentControlEntry>>,
}

impl Descriptor<'_> for DigitalCopyControlDescriptor {
    const TAG: u8 = 0xC1;

    fn read(data: &[u8]) -> Option<DigitalCopyControlDescriptor> {
        if data.len() < 1 {
            log::debug!("invalid DigitalCopyControlDescriptor");
            return None;
        }

        let digital_recording_control_data = (data[0] & 0b11000000) >> 6;
        let maximum_bitrate_flag = data[0] & 0b00100000 != 0;
        let component_control_flag = data[0] & 0b00010000 != 0;
        let copy_control_type = (data[0] & 0b00001100) >> 2;
        let aps_control_data = if copy_control_type == 0b01 {
            Some(data[0] & 0b00000011)
        } else {
            None
        };

        let mut data = &data[1..];
        let maximum_bitrate = if maximum_bitrate_flag {
            let [maximum_bitrate, ref rem @ ..] = *data else {
                log::debug!("invalid DigitalCopyControlDescriptor::maximum_bitrate_flag");
                return None;
            };
            data = rem;

            Some(maximum_bitrate)
        } else {
            None
        };

        let component_controls = if component_control_flag {
            let [component_control_length, ref data @ ..] = *data else {
                log::debug!("invalid DigitalCopyControlDescriptor::component_control_length");
                return None;
            };
            let Some(mut data) = data.get(..component_control_length as usize) else {
                log::debug!("invalid DigitalCopyControlDescriptor::component_controls");
                return None;
            };

            let mut component_controls = Vec::new();
            while data.len() >= 2 {
                let component_tag = data[0];
                let digital_recording_control_data = (data[1] & 0b11000000) >> 6;
                let maximum_bitrate_flag = data[1] & 0b00100000 != 0;
                let copy_control_type = (data[1] & 0b00001100) >> 2;
                let aps_control_data = if copy_control_type == 0b01 {
                    Some(data[1] & 0b00000011)
                } else {
                    None
                };

                data = &data[2..];
                let maximum_bitrate = if maximum_bitrate_flag {
                    let [maximum_bitrate, ref rem @ ..] = *data else {
                        break;
                    };
                    data = rem;

                    Some(maximum_bitrate)
                } else {
                    None
                };

                component_controls.push(ComponentControlEntry {
                    component_tag,
                    digital_recording_control_data,
                    copy_control_type,
                    aps_control_data,
                    maximum_bitrate,
                });
            }

            Some(component_controls)
        } else {
            None
        };

        Some(DigitalCopyControlDescriptor {
            digital_recording_control_data,
            copy_control_type,
            aps_control_data,
            maximum_bitrate,
            component_controls,
        })
    }
}

/// [`AudioComponentDescriptor`]における`quality_indicator`。
#[derive(Debug)]
pub enum QualityIndicator {
    /// Reserved for future use
    Reserved,
    /// Mode 1
    Mode1,
    /// Mode 2
    Mode2,
    /// Mode 3
    Mode3,
}

/// [`AudioComponentDescriptor`]における`sampling_rate`。
#[derive(Debug)]
pub enum SamplingFrequency {
    /// Reserved for future use (0b000)
    Reserved1,
    /// 16kHZ
    SF16k,
    /// 22.05kHZ
    SF22_05k,
    /// 24kHZ
    SF24k,
    /// Reserved (0b100)
    Reserved2,
    /// 32kHZ
    SF32k,
    /// 44.1kHZ
    SF44_1k,
    /// 48kHZ
    SF48k,
}

/// 音声コンポーネント記述子。
#[derive(Debug)]
pub struct AudioComponentDescriptor<'a> {
    /// stream_content (4bit)
    pub stream_content: u8,
    /// component_type
    pub component_type: u8,
    /// component_tag
    pub component_tag: u8,
    /// stream_type
    pub stream_type: StreamType,
    /// simulcast_group_tag
    pub simulcast_group_tag: u8,
    /// main_component_flag
    pub main_component_flag: bool,
    /// quality_indicator
    pub quality_indicator: QualityIndicator,
    /// sampling_rate
    pub sampling_rate: SamplingFrequency,
    /// ISO 639-2で規定される3文字の国名コード。
    pub lang_code: [u8; 3],
    /// ISO 639-2で規定される3文字の国名コード。
    pub lang_code_2: Option<[u8; 3]>,
    /// text
    pub text: &'a [u8],
}

impl<'a> Descriptor<'a> for AudioComponentDescriptor<'a> {
    const TAG: u8 = 0xC4;

    fn read(data: &'a [u8]) -> Option<AudioComponentDescriptor<'a>> {
        if data.len() < 9 {
            log::debug!("invalid AudioComponentDescriptor");
            return None;
        }

        let stream_content = data[0] & 0b00001111;
        let component_type = data[1];
        let component_tag = data[2];
        let stream_type = StreamType(data[3]);
        let simulcast_group_tag = data[4];
        let es_multi_lingual_flag = (data[5] & 0b10000000) != 0;
        let main_component_flag = (data[5] & 0b01000000) != 0;
        let quality_indicator = match (data[5] & 0b00110000) >> 4 {
            0b00 => QualityIndicator::Reserved,
            0b01 => QualityIndicator::Mode1,
            0b10 => QualityIndicator::Mode2,
            0b11 => QualityIndicator::Mode3,
            _ => unreachable!(),
        };
        let sampling_rate = match (data[5] & 0b00001110) >> 1 {
            0b000 => SamplingFrequency::Reserved1,
            0b001 => SamplingFrequency::SF16k,
            0b010 => SamplingFrequency::SF22_05k,
            0b011 => SamplingFrequency::SF24k,
            0b100 => SamplingFrequency::Reserved2,
            0b101 => SamplingFrequency::SF32k,
            0b110 => SamplingFrequency::SF44_1k,
            0b111 => SamplingFrequency::SF48k,
            _ => unreachable!(),
        };
        let lang_code = data[6..=8].try_into().unwrap();

        let mut data = &data[9..];
        let lang_code_2 = if es_multi_lingual_flag {
            let Some((lang_code, rem)) = data.split_at_checked(3) else {
                log::debug!("invalid AudioComponentDescriptor::ISO_639_language_code_2");
                return None;
            };
            let lang_code = lang_code.try_into().unwrap();
            data = rem;

            Some(lang_code)
        } else {
            None
        };

        let text = data;

        Some(AudioComponentDescriptor {
            stream_content,
            component_type,
            component_tag,
            stream_type,
            simulcast_group_tag,
            main_component_flag,
            quality_indicator,
            sampling_rate,
            lang_code,
            lang_code_2,
            text,
        })
    }
}

/// [`HyperlinkDescriptor`]における`selector`の値。
#[derive(Debug)]
pub enum SelectorInfo<'a> {
    /// link_service_info
    LinkServiceInfo(LinkServiceInfo),
    /// link_event_info
    LinkEventInfo(LinkEventInfo),
    /// link_module_info
    LinkModuleInfo(LinkModuleInfo),
    /// link_content_info
    LinkContentInfo(LinkContentInfo),
    /// link_content_module_info
    LinkContentModuleInfo(LinkContentModuleInfo),
    /// link_ert_node_info
    LinkErtNodeInfo(LinkErtNodeInfo),
    /// link_stored_content_info
    LinkStoredContentInfo(LinkStoredContentInfo<'a>),
    /// 不明。
    Unknown(LinkUnknown<'a>),
}

/// link_service_info
#[derive(Debug)]
pub struct LinkServiceInfo {
    /// original_network_id
    pub original_network_id: u16,
    /// transport_stream_id
    pub transport_stream_id: u16,
    /// service_id
    pub service_id: u16,
}

/// link_event_info
#[derive(Debug)]
pub struct LinkEventInfo {
    /// original_network_id
    pub original_network_id: u16,
    /// transport_stream_id
    pub transport_stream_id: u16,
    /// service_id
    pub service_id: u16,
    /// event_id
    pub event_id: u16,
}

/// link_module_info
#[derive(Debug)]
pub struct LinkModuleInfo {
    /// original_network_id
    pub original_network_id: u16,
    /// transport_stream_id
    pub transport_stream_id: u16,
    /// service_id
    pub service_id: u16,
    /// event_id
    pub event_id: u16,
    /// component_tag
    pub component_tag: u8,
    /// moduleId
    pub module_id: u16,
}

/// link_content_info
#[derive(Debug)]
pub struct LinkContentInfo {
    /// original_network_id
    pub original_network_id: u16,
    /// transport_stream_id
    pub transport_stream_id: u16,
    /// service_id
    pub service_id: u16,
    /// content_id
    pub content_id: u32,
}

/// link_content_module_info
#[derive(Debug)]
pub struct LinkContentModuleInfo {
    /// original_network_id
    pub original_network_id: u16,
    /// transport_stream_id
    pub transport_stream_id: u16,
    /// service_id
    pub service_id: u16,
    /// content_id
    pub content_id: u32,
    /// component_tag
    pub component_tag: u8,
    /// moduleId
    pub module_id: u16,
}

/// link_ert_node_info
#[derive(Debug)]
pub struct LinkErtNodeInfo {
    /// information_provider_id
    pub information_provider_id: u16,
    /// event_relation_id
    pub event_relation_id: u16,
    /// node_id
    pub node_id: u16,
}

/// link_stored_content_info
#[derive(Debug)]
pub struct LinkStoredContentInfo<'a> {
    /// uri
    pub uri: &'a [u8],
}

/// 不明。
#[derive(Debug)]
pub struct LinkUnknown<'a> {
    /// 不明なセレクタの種類。
    pub link_destination_type: u8,
    /// 不明なセレクタのデータ。
    pub selector: &'a [u8],
}

/// ハイパーリンク記述子。
#[derive(Debug)]
pub struct HyperlinkDescriptor<'a> {
    /// hyper_linkage_type
    pub hyper_linkage_type: u8,
    /// link_destination_type + selector_byte
    pub selector: SelectorInfo<'a>,
    /// private_data
    pub private_data: &'a [u8],
}

impl<'a> Descriptor<'a> for HyperlinkDescriptor<'a> {
    const TAG: u8 = 0xC5;

    fn read(data: &'a [u8]) -> Option<HyperlinkDescriptor<'a>> {
        let [hyper_linkage_type, link_destination_type, selector_length, ref data @ ..] = *data
        else {
            log::debug!("invalid HyperlinkDescriptor");
            return None;
        };
        let Some((selector, private_data)) = data.split_at_checked(selector_length as usize) else {
            log::debug!("invalid HyperlinkDescriptor::selector");
            return None;
        };
        let selector = match link_destination_type {
            0x01 => {
                if selector.len() != 6 {
                    log::debug!("invalid LinkServiceInfo");
                    return None;
                }

                let original_network_id = selector[0..=1].read_be_16();
                let transport_stream_id = selector[2..=3].read_be_16();
                let service_id = selector[4..=5].read_be_16();

                SelectorInfo::LinkServiceInfo(LinkServiceInfo {
                    original_network_id,
                    transport_stream_id,
                    service_id,
                })
            }
            0x02 => {
                if selector.len() != 8 {
                    log::debug!("invalid LinkEventInfo");
                    return None;
                }

                let original_network_id = selector[0..=1].read_be_16();
                let transport_stream_id = selector[2..=3].read_be_16();
                let service_id = selector[4..=5].read_be_16();
                let event_id = selector[6..=7].read_be_16();

                SelectorInfo::LinkEventInfo(LinkEventInfo {
                    original_network_id,
                    transport_stream_id,
                    service_id,
                    event_id,
                })
            }
            0x03 => {
                if selector.len() != 11 {
                    log::debug!("invalid LinkModuleInfo");
                    return None;
                }

                let original_network_id = selector[0..=1].read_be_16();
                let transport_stream_id = selector[2..=3].read_be_16();
                let service_id = selector[4..=5].read_be_16();
                let event_id = selector[6..=7].read_be_16();
                let component_tag = selector[8];
                let module_id = selector[9..=10].read_be_16();

                SelectorInfo::LinkModuleInfo(LinkModuleInfo {
                    original_network_id,
                    transport_stream_id,
                    service_id,
                    event_id,
                    component_tag,
                    module_id,
                })
            }
            0x04 => {
                if selector.len() != 10 {
                    log::debug!("invalid LinkContentInfo");
                    return None;
                }

                let original_network_id = selector[0..=1].read_be_16();
                let transport_stream_id = selector[2..=3].read_be_16();
                let service_id = selector[4..=5].read_be_16();
                let content_id = selector[6..=9].read_be_32();

                SelectorInfo::LinkContentInfo(LinkContentInfo {
                    original_network_id,
                    transport_stream_id,
                    service_id,
                    content_id,
                })
            }
            0x05 => {
                if selector.len() != 13 {
                    log::debug!("invalid LinkContentModuleInfo");
                    return None;
                }

                let original_network_id = selector[0..=1].read_be_16();
                let transport_stream_id = selector[2..=3].read_be_16();
                let service_id = selector[4..=5].read_be_16();
                let content_id = selector[6..=9].read_be_32();
                let component_tag = selector[10];
                let module_id = selector[11..=12].read_be_16();

                SelectorInfo::LinkContentModuleInfo(LinkContentModuleInfo {
                    original_network_id,
                    transport_stream_id,
                    service_id,
                    content_id,
                    component_tag,
                    module_id,
                })
            }
            0x06 => {
                if selector.len() != 6 {
                    log::debug!("invalid LinkErtNodeInfo");
                    return None;
                }

                let information_provider_id = selector[0..=1].read_be_16();
                let event_relation_id = selector[2..=3].read_be_16();
                let node_id = selector[4..=5].read_be_16();

                SelectorInfo::LinkErtNodeInfo(LinkErtNodeInfo {
                    information_provider_id,
                    event_relation_id,
                    node_id,
                })
            }
            0x07 => SelectorInfo::LinkStoredContentInfo(LinkStoredContentInfo { uri: selector }),
            _ => SelectorInfo::Unknown(LinkUnknown {
                link_destination_type,
                selector,
            }),
        };

        Some(HyperlinkDescriptor {
            hyper_linkage_type,
            selector,
            private_data,
        })
    }
}

/// [`TargetRegionDescriptor`]における`target_region_spec`の値。
#[derive(Debug)]
pub enum TargetRegionSpec<'a> {
    /// bs_prefecture_spec
    BsPrefectureSpec(BsPrefectureSpec),
    /// 不明な`target_region_spec`。
    Unknown(&'a [u8]),
}

/// Region designator in prefecture designation
#[derive(Debug)]
pub struct BsPrefectureSpec {
    /// prefecture_bitmap
    pub prefecture_bitmap: [u8; 7],
}

/// 対象地域記述子。
#[derive(Debug)]
pub struct TargetRegionDescriptor<'a> {
    /// target_region_spec
    pub target_region_spec: TargetRegionSpec<'a>,
}

impl<'a> Descriptor<'a> for TargetRegionDescriptor<'a> {
    const TAG: u8 = 0xC6;

    fn read(data: &'a [u8]) -> Option<TargetRegionDescriptor<'a>> {
        let [region_spec_type, ref data @ ..] = *data else {
            log::debug!("invalid TargetRegionDescriptor");
            return None;
        };

        let target_region_spec = match region_spec_type {
            0x01 => {
                let Ok(prefecture_bitmap) = data.try_into() else {
                    log::debug!("invalid TargetRegionDescriptor::BsPrefectureSpec");
                    return None;
                };

                TargetRegionSpec::BsPrefectureSpec(BsPrefectureSpec { prefecture_bitmap })
            }
            _ => TargetRegionSpec::Unknown(data),
        };

        Some(TargetRegionDescriptor { target_region_spec })
    }
}

/// ビデオデコードコントロール記述子。
#[derive(Debug)]
pub struct VideoDecodeControlDescriptor {
    /// still_picture_flag
    pub still_picture_flag: bool,
    /// sequence_end_code_flag
    pub sequence_end_code_flag: bool,
    /// video_encode_format (4bit)
    pub video_encode_format: u8,
}

impl Descriptor<'_> for VideoDecodeControlDescriptor {
    const TAG: u8 = 0xC8;

    fn read(data: &[u8]) -> Option<VideoDecodeControlDescriptor> {
        if data.len() != 1 {
            log::debug!("invalid VideoDecodeControlDescriptor");
            return None;
        }

        let still_picture_flag = data[0] & 0b10000000 != 0;
        let sequence_end_code_flag = data[0] & 0b01000000 != 0;
        let video_encode_format = (data[0] & 0b00111100) >> 2;

        Some(VideoDecodeControlDescriptor {
            still_picture_flag,
            sequence_end_code_flag,
            video_encode_format,
        })
    }
}

/// SubDescriptor
#[derive(Debug)]
pub struct SubDescriptor<'a> {
    /// SubDescriptorType
    pub sub_descriptor_type: u8,
    /// additionalInformation
    pub additional_information: &'a [u8],
}

/// [`DownloadContentDescriptor`]における`compatibility_descriptors`。
#[derive(Debug)]
pub struct CompatibilityDescriptor<'a> {
    /// descriptorType
    pub descriptor_type: u8,
    /// `specifierType`
    pub specifier_type: u8,
    /// `specifierData()`
    pub specifier_data: [u8; 3],
    /// `model`
    pub model: u16,
    /// `version`
    pub version: u16,
    /// `subDescriptor()`の内容。
    pub sub_descriptors: Vec<SubDescriptor<'a>>,
}

/// [`DownloadContentDescriptor`]における`modules`。
#[derive(Debug)]
pub struct ModuleInfo<'a> {
    /// module_id
    pub module_id: u16,
    /// module_size
    pub module_size: u32,
    /// module_info_byte
    pub module_info: &'a [u8],
}

/// [`DownloadContentDescriptor`]における`text_info`。
#[derive(Debug)]
pub struct TextInfo<'a> {
    /// ISO 639-2で規定される3文字の国名コード。
    pub lang_code: [u8; 3],
    /// text_char,
    pub text: &'a [u8],
}

/// ダウンロードコンテンツ記述子。
#[derive(Debug)]
pub struct DownloadContentDescriptor<'a> {
    /// reboot,
    pub reboot: bool,
    /// add_on,
    pub add_on: bool,
    /// text_info_flag,
    pub text_info_flag: bool,
    /// component_size,
    pub component_size: u32,
    /// download_id,
    pub download_id: u32,
    /// time_out_value_DII,
    pub time_out_value_dii: u32,
    /// leak_rate,
    pub leak_rate: u32,
    /// component_tag,
    pub component_tag: u8,
    /// compatibilityDescriptor,
    pub compatibility_descriptors: Option<Vec<CompatibilityDescriptor<'a>>>,
    /// information for each module in the descriptor
    pub modules: Option<Vec<ModuleInfo<'a>>>,
    /// private_data_byte,
    pub private_data: &'a [u8],
    /// ISO_639_language_code + text_char
    pub text_info: Option<TextInfo<'a>>,
}

impl<'a> Descriptor<'a> for DownloadContentDescriptor<'a> {
    const TAG: u8 = 0xC9;

    fn read(data: &'a [u8]) -> Option<DownloadContentDescriptor<'a>> {
        if data.len() < 17 {
            log::debug!("invalid DownloadContentDescriptor");
            return None;
        }

        let reboot = data[0] & 0b10000000 != 0;
        let add_on = data[0] & 0b01000000 != 0;
        let compatibility_flag = data[0] & 0b00100000 != 0;
        let module_info_flag = data[0] & 0b00010000 != 0;
        let text_info_flag = data[0] & 0b00001000 != 0;
        let component_size = data[1..=4].read_be_32();
        let download_id = data[5..=8].read_be_32();
        let time_out_value_dii = data[9..=12].read_be_32();
        let leak_rate = data[13..=16].read_be_32() >> 10; // 22bit
        let component_tag = data[16];

        let mut data = &data[17..];
        let compatibility_descriptors = if compatibility_flag {
            if data.len() < 4 {
                log::debug!("invalid DownloadContentDescriptor::compatibility_flag");
                return None;
            }

            let compatibility_descriptor_length = data[0..=1].read_be_16();
            let descriptor_count = data[2..=3].read_be_16();
            data = &data[4..];
            if data.len() < compatibility_descriptor_length as usize {
                log::debug!("invalid DownloadContentDescriptor::compatibility_descriptor_length");
                return None;
            }

            let mut descriptors = Vec::with_capacity(descriptor_count as usize);
            for _ in 0..descriptor_count {
                if data.len() < 11 {
                    log::debug!("invalid CompatibilityDescriptor");
                    return None;
                }

                let descriptor_type = data[0];
                // let descriptor_length = data[1];
                let specifier_type = data[2];
                let specifier_data = data[3..=5].try_into().unwrap();
                let model = data[6..=7].read_be_16();
                let version = data[8..=9].read_be_16();
                let sub_descriptor_count = data[10];
                data = &data[11..];

                let mut sub_descriptors = Vec::with_capacity(sub_descriptor_count as usize);
                for _ in 0..sub_descriptor_count {
                    let [sub_descriptor_type, sub_descriptor_length, ref rem @ ..] = *data else {
                        log::debug!("invalid SubDescriptor");
                        return None;
                    };
                    let Some((additional_information, rem)) = rem
                        .split_at_checked(sub_descriptor_length as usize)
                    else {
                        log::debug!("invalid SubDescriptor::additional_information");
                        return None;
                    };
                    data = rem;

                    sub_descriptors.push(SubDescriptor {
                        sub_descriptor_type,
                        additional_information,
                    });
                }

                descriptors.push(CompatibilityDescriptor {
                    descriptor_type,
                    specifier_type,
                    specifier_data,
                    model,
                    version,
                    sub_descriptors,
                });
            }

            Some(descriptors)
        } else {
            None
        };

        let modules = if module_info_flag {
            if data.len() < 2 {
                log::debug!("invalid DownloadContentDescriptor::num_of_modules");
                return None;
            }

            let num_of_modules = data[0..=1].read_be_16();
            let mut modules = Vec::with_capacity(num_of_modules as usize);
            for _ in 0..num_of_modules {
                if data.len() < 7 {
                    log::debug!("invalid DownloadContentDescriptor::modules");
                    return None;
                }

                let module_id = data[0..=1].read_be_16();
                let module_size = data[2..=5].read_be_32();
                let module_info_length = data[6];
                let Some((module_info, rem)) = data[7..]
                    .split_at_checked(module_info_length as usize)
                else {
                    log::debug!("invalid DownloadContentDescriptor::module_info");
                    return None;
                };
                data = rem;

                modules.push(ModuleInfo {
                    module_id,
                    module_size,
                    module_info,
                });
            }

            Some(modules)
        } else {
            None
        };

        let [private_data_length, ref data @ ..] = *data else {
            log::debug!("invalid DownloadContentDescriptor::private_data_length");
            return None;
        };
        let Some((private_data, data)) = data.split_at_checked(private_data_length as usize) else {
            log::debug!("invalid DownloadContentDescriptor::private_data");
            return None;
        };

        let text_info = if text_info_flag {
            let Some((lang_code, rem)) = data.split_at_checked(3) else {
                log::debug!("invalid DownloadContentDescriptor::lang_code");
                return None;
            };
            let lang_code = lang_code.try_into().unwrap();

            let [text_length, ref rem @ ..] = *rem else {
                log::debug!("invalid DownloadContentDescriptor::text_length");
                return None;
            };
            let Some((text, _rem)) = rem.split_at_checked(text_length as usize) else {
                log::debug!("invalid DownloadContentDescriptor::text");
                return None;
            };
            // data = _rem;

            Some(TextInfo { lang_code, text })
        } else {
            None
        };

        Some(DownloadContentDescriptor {
            reboot,
            add_on,
            text_info_flag,
            component_size,
            download_id,
            time_out_value_dii,
            leak_rate,
            component_tag,
            compatibility_descriptors,
            modules,
            private_data,
            text_info,
        })
    }
}

/// CA_EMM_TS記述子。
#[derive(Debug)]
pub struct CaEmmTsDescriptor {
    /// CA_system_id
    pub ca_system_id: u16,
    /// transport_stream_id
    pub transport_stream_id: u16,
    /// original_network_id
    pub original_network_id: u16,
    /// power_supply_period
    pub power_supply_period: u8,
}

impl Descriptor<'_> for CaEmmTsDescriptor {
    const TAG: u8 = 0xCA;

    fn read(data: &[u8]) -> Option<CaEmmTsDescriptor> {
        if data.len() != 7 {
            log::debug!("invalid CaEmmTsDescriptor");
            return None;
        }

        let ca_system_id = data[0..=1].read_be_16();
        let transport_stream_id = data[2..=3].read_be_16();
        let original_network_id = data[4..=5].read_be_16();
        let power_supply_period = data[6];

        Some(CaEmmTsDescriptor {
            ca_system_id,
            transport_stream_id,
            original_network_id,
            power_supply_period,
        })
    }
}

/// CA契約情報記述子。
#[derive(Debug)]
pub struct CaContractInfoDescriptor<'a> {
    /// CA_system_id
    pub ca_system_id: u16,
    /// CA_unit_id (4bit)
    pub ca_unit_id: u8,
    /// component_tag
    pub component_tag: &'a [u8],
    /// contract_verification_info
    pub contract_verification_info: &'a [u8],
    /// fee_name
    pub fee_name: &'a [u8],
}

impl<'a> Descriptor<'a> for CaContractInfoDescriptor<'a> {
    const TAG: u8 = 0xCB;

    fn read(data: &'a [u8]) -> Option<CaContractInfoDescriptor<'a>> {
        if data.len() < 3 {
            log::debug!("invalid CaContractInfoDescriptor");
            return None;
        }

        let ca_system_id = data[0..=1].read_be_16();
        let ca_unit_id = (data[2] & 0b11110000) >> 4;
        let num_of_component = data[2] & 0b00001111;
        let Some((component_tag, data)) = data[3..].split_at_checked(num_of_component as usize)
        else {
            log::debug!("invalid CaContractInfoDescriptor::component_tag");
            return None;
        };
        let [contract_verification_info_length, ref data @ ..] = *data else {
            log::debug!("invalid CaContractInfoDescriptor::contract_verification_info_length");
            return None;
        };
        let Some((contract_verification_info, data)) = data
            .split_at_checked(contract_verification_info_length as usize)
        else {
            log::debug!("invalid CaContractInfoDescriptor::contract_verification_info");
            return None;
        };
        let [fee_name_length, ref data @ ..] = *data else {
            log::debug!("invalid CaContractInfoDescriptor::fee_name_length");
            return None;
        };
        let Some((fee_name, _)) = data.split_at_checked(fee_name_length as usize) else {
            log::debug!("invalid CaContractInfoDescriptor::fee_name");
            return None;
        };

        Some(CaContractInfoDescriptor {
            ca_system_id,
            ca_unit_id,
            component_tag,
            contract_verification_info,
            fee_name,
        })
    }
}

/// CAサービス記述子。
#[derive(Debug)]
pub struct CaServiceDescriptor {
    /// CA_system_id
    pub ca_system_id: u16,
    /// ca_broadcaster_group_id
    pub ca_broadcaster_group_id: u8,
    /// message_control
    pub message_control: u8,
    /// service_id
    pub service_ids: Vec<u16>,
}

impl Descriptor<'_> for CaServiceDescriptor {
    const TAG: u8 = 0xCC;

    fn read(data: &[u8]) -> Option<CaServiceDescriptor> {
        if data.len() < 4 {
            log::debug!("invalid CaServiceDescriptor");
            return None;
        }

        let ca_system_id = data[0..=1].read_be_16();
        let ca_broadcaster_group_id = data[2];
        let message_control = data[3];
        let service_ids = data[4..].chunks_exact(2).map(<[u8]>::read_be_16).collect();

        Some(CaServiceDescriptor {
            ca_system_id,
            ca_broadcaster_group_id,
            message_control,
            service_ids,
        })
    }
}

/// [`TsInformationDescriptor`]における`transmissions`の要素。
#[derive(Debug)]
pub struct TsInformationTransmission {
    /// transmission_type_info
    pub transmission_type_info: u8,
    /// service_id
    pub service_ids: Vec<u16>,
}

/// TS情報記述子。
#[derive(Debug)]
pub struct TsInformationDescriptor<'a> {
    /// remote_control_key_id
    pub remote_control_key_id: u8,
    /// ts_name_char
    pub ts_name: &'a [u8],
    /// transmission_type_info + service_id
    pub transmissions: Vec<TsInformationTransmission>,
}

impl<'a> Descriptor<'a> for TsInformationDescriptor<'a> {
    const TAG: u8 = 0xCD;

    fn read(data: &'a [u8]) -> Option<TsInformationDescriptor<'a>> {
        if data.len() < 3 {
            log::debug!("invalid TsInformationDescriptor");
            return None;
        }

        let remote_control_key_id = data[0];
        let length_of_ts_name = (data[1] & 0b11111100) >> 2;
        let transmission_type_count = data[1] & 0b00000011;
        let Some((ts_name, mut data)) = data[2..].split_at_checked(length_of_ts_name as usize)
        else {
            log::debug!("invalid TsInformationDescriptor::ts_name");
            return None;
        };

        let mut transmissions = Vec::with_capacity(transmission_type_count as usize);
        for _ in 0..transmission_type_count {
            if data.len() < 2 {
                log::debug!("invalid TsInformationTransmission");
                return None;
            }

            let transmission_type_info = data[0];
            let num_of_service = data[1];
            let Some((service_ids, rem)) = data[2..].split_at_checked(num_of_service as usize)
            else {
                log::debug!("invalid TsInformationTransmission::service_ids");
                return None;
            };
            let service_ids = service_ids
                .chunks_exact(2)
                .map(<[u8]>::read_be_16)
                .collect();
            data = rem;

            transmissions.push(TsInformationTransmission {
                transmission_type_info,
                service_ids,
            });
        }

        Some(TsInformationDescriptor {
            remote_control_key_id,
            ts_name,
            transmissions,
        })
    }
}

/// broadcaster identifier
#[derive(Debug)]
pub struct BroadcasterId {
    /// original_network_id
    pub original_network_id: u16,
    /// broadcaster_id
    pub broadcaster_id: u8,
}

/// Digital terrestrial television broadcast
#[derive(Debug)]
pub struct DigitalTerrestrialTelevisionBroadcast<'a> {
    /// terrestrial_broadcaster_id
    pub terrestrial_broadcaster_id: u16,
    /// affiliation_id
    pub affiliation_id: &'a [u8],
    /// original_network_id + broadcaster_id
    pub broadcaster_ids: Vec<BroadcasterId>,
    /// private_data_byte
    pub private_data: &'a [u8],
}

/// Digital terrestrial sound broadcast
#[derive(Debug)]
pub struct DigitalTerrestrialSoundBroadcast<'a> {
    /// terrestrial_sound_broadcaster_id
    pub terrestrial_sound_broadcaster_id: u16,
    /// sound_broadcast_affiliation_id
    pub sound_broadcast_affiliation_id: &'a [u8],
    /// original_network_id + broadcaster_id
    pub broadcaster_ids: Vec<BroadcasterId>,
    /// private_data
    pub private_data: &'a [u8],
}

/// 拡張ブロードキャスタ記述子。
#[derive(Debug)]
pub enum ExtendedBroadcasterDescriptor<'a> {
    /// Digital terrestrial television broadcast
    DigitalTerrestrialTelevisionBroadcast(DigitalTerrestrialTelevisionBroadcast<'a>),
    /// Digital terrestrial sound broadcast
    DigitalTerrestrialSoundBroadcast(DigitalTerrestrialSoundBroadcast<'a>),
    /// Not defined
    Unknown(&'a [u8]),
}

impl<'a> Descriptor<'a> for ExtendedBroadcasterDescriptor<'a> {
    const TAG: u8 = 0xCE;

    fn read(data: &'a [u8]) -> Option<ExtendedBroadcasterDescriptor<'a>> {
        fn read_broadcaster_ids(broadcaster_ids: &[u8]) -> Vec<BroadcasterId> {
            broadcaster_ids
                .chunks_exact(3)
                .map(|chunk| {
                    let original_network_id = chunk[0..=1].read_be_16();
                    let broadcaster_id = chunk[2];

                    BroadcasterId {
                        original_network_id,
                        broadcaster_id,
                    }
                })
                .collect()
        }

        if data.len() < 1 {
            log::debug!("invalid ExtendedBroadcasterDescriptor");
            return None;
        }

        let broadcaster_type = (data[0] & 0b11110000) >> 4;
        let data = &data[1..];

        let descriptor = match broadcaster_type {
            0x1 => {
                if data.len() < 3 {
                    log::debug!("invalid DigitalTerrestrialTelevisionBroadcast");
                    return None;
                }

                let terrestrial_broadcaster_id = data[0..=1].read_be_16();
                let number_of_affiliation_id_loop = (data[2] & 0b11110000) >> 4;
                let number_of_broadcaster_id_loop = data[2] & 0b00001111;

                let Some((affiliation_id, data)) = data[3..]
                    .split_at_checked(number_of_affiliation_id_loop as usize)
                else {
                    log::debug!("invalid DigitalTerrestrialTelevisionBroadcast::affiliation_id");
                    return None;
                };
                let Some((broadcaster_ids, data)) = data
                    .split_at_checked(3 * number_of_broadcaster_id_loop as usize)
                else {
                    log::debug!("invalid DigitalTerrestrialTelevisionBroadcast::broadcaster_ids");
                    return None;
                };
                let broadcaster_ids = read_broadcaster_ids(broadcaster_ids);
                let private_data = data;

                ExtendedBroadcasterDescriptor::DigitalTerrestrialTelevisionBroadcast(
                    DigitalTerrestrialTelevisionBroadcast {
                        terrestrial_broadcaster_id,
                        affiliation_id,
                        broadcaster_ids,
                        private_data,
                    },
                )
            }
            0x2 => {
                if data.len() < 3 {
                    log::debug!("invalid DigitalTerrestrialSoundBroadcast");
                    return None;
                }

                let terrestrial_sound_broadcaster_id = data[0..=1].read_be_16();
                let number_of_sound_broadcast_affiliation_id_loop = (data[2] & 0b11110000) >> 4;
                let number_of_broadcaster_id_loop = data[2] & 0b00001111;

                let Some((sound_broadcast_affiliation_id, data)) = data[3..]
                    .split_at_checked(number_of_sound_broadcast_affiliation_id_loop as usize)
                else {
                    log::debug!("invalid DigitalTerrestrialSoundBroadcast::sound_broadcast_affiliation_id");
                    return None;
                };
                let Some((broadcaster_ids, data)) = data
                    .split_at_checked(3 * number_of_broadcaster_id_loop as usize)
                else {
                    log::debug!("invalid DigitalTerrestrialSoundBroadcast::broadcaster_ids");
                    return None;
                };
                let broadcaster_ids = read_broadcaster_ids(broadcaster_ids);
                let private_data = data;

                ExtendedBroadcasterDescriptor::DigitalTerrestrialSoundBroadcast(
                    DigitalTerrestrialSoundBroadcast {
                        terrestrial_sound_broadcaster_id,
                        sound_broadcast_affiliation_id,
                        broadcaster_ids,
                        private_data,
                    },
                )
            }
            _ => ExtendedBroadcasterDescriptor::Unknown(data),
        };
        Some(descriptor)
    }
}

/// CDT transmission scheme 1
///
/// when referring to CDT directly with download data identification
#[derive(Debug)]
pub struct LogoTransmissionCdt1 {
    /// logo_id (9bit)
    pub logo_id: u16,
    /// logo_version (12bit)
    pub logo_version: u16,
    /// download_data_id
    pub download_data_id: u16,
}

/// CDT transmission scheme 2
///
/// when referring to CDT using logo identification indirectly with download data identification
#[derive(Debug)]
pub struct LogoTransmissionCdt2 {
    /// logo_id (9bit)
    pub logo_id: u16,
}

/// ロゴ伝送記述子。
#[derive(Debug)]
pub enum LogoTransmissionDescriptor<'a> {
    /// CDT transmission scheme 1
    Cdt1(LogoTransmissionCdt1),
    /// CDT transmission scheme 2
    Cdt2(LogoTransmissionCdt2),
    /// Simple logo system
    Simple(&'a [u8]),
    /// Reserved for future
    Unknown(&'a [u8]),
}

impl<'a> Descriptor<'a> for LogoTransmissionDescriptor<'a> {
    const TAG: u8 = 0xCF;

    fn read(data: &'a [u8]) -> Option<LogoTransmissionDescriptor<'a>> {
        let [logo_transmission_type, ref data @ ..] = *data else {
            log::debug!("invalid LogoTransmissionDescriptor");
            return None;
        };

        let descriptor = match logo_transmission_type {
            0x01 => {
                if data.len() != 6 {
                    log::debug!("invalid LogoTransmissionCdt1");
                    return None;
                }

                let logo_id = data[0..=1].read_be_16() & 0b0000_0001_1111_1111;
                let logo_version = data[2..=3].read_be_16() & 0b0000_1111_1111_1111;
                let download_data_id = data[4..=5].read_be_16();

                LogoTransmissionDescriptor::Cdt1(LogoTransmissionCdt1 {
                    logo_id,
                    logo_version,
                    download_data_id,
                })
            }
            0x02 => {
                if data.len() != 2 {
                    log::debug!("invalid LogoTransmissionCdt2");
                    return None;
                }

                let logo_id = data[0..=1].read_be_16() & 0b0000_0001_1111_1111;

                LogoTransmissionDescriptor::Cdt2(LogoTransmissionCdt2 { logo_id })
            }
            0x03 => LogoTransmissionDescriptor::Simple(data),
            _ => LogoTransmissionDescriptor::Unknown(data),
        };
        Some(descriptor)
    }
}

/// [`SeriesDescriptor`]における`program_pattern`の値。
#[derive(Debug)]
pub enum ProgramPattern {
    /// Nonscheduled
    Nonscheduled,
    /// Regular program (every day, every day except week-end, only weekends, etc.),
    /// programmed several days a week
    Regular,
    /// Programmed about once a week
    OnceAWeek,
    /// Programmed about once a month,
    OnceAMonth,
    /// Programmed several events in a day
    SeveralEventsInADay,
    /// Division of long hour program
    Division,
    /// Program for regular or irregular accumulation
    Accumulation,
    /// Undefined
    Undefined,
}

/// シリーズ記述子。
#[derive(Debug)]
pub struct SeriesDescriptor<'a> {
    /// series_id
    pub series_id: u16,
    /// repeat_label (4bit)
    pub repeat_label: u8,
    /// program_pattern
    pub program_pattern: ProgramPattern,
    /// expire_date
    pub expire_date: Option<MjdDate>,
    /// episode_number (12bit)
    pub episode_number: u16,
    /// last_episode_number (12bit)
    pub last_episode_number: u16,
    /// series_name_char
    pub series_name: &'a [u8],
}

impl<'a> Descriptor<'a> for SeriesDescriptor<'a> {
    const TAG: u8 = 0xD5;

    fn read(data: &'a [u8]) -> Option<SeriesDescriptor<'a>> {
        if data.len() < 8 {
            log::debug!("invalid SeriesDescriptor");
            return None;
        }

        let series_id = data[0..=1].read_be_16();
        let repeat_label = (data[2] & 0b11110000) >> 4;
        let program_pattern = match (data[2] & 0b00001110) >> 1 {
            0x0 => ProgramPattern::Nonscheduled,
            0x1 => ProgramPattern::Regular,
            0x2 => ProgramPattern::OnceAWeek,
            0x3 => ProgramPattern::OnceAMonth,
            0x4 => ProgramPattern::SeveralEventsInADay,
            0x5 => ProgramPattern::Division,
            0x6 => ProgramPattern::Accumulation,
            0x7 => ProgramPattern::Undefined,
            _ => unreachable!(),
        };
        let expire_date_valid_flag = data[2] & 0b00000001 != 0;
        let expire_date =
            expire_date_valid_flag.then(|| MjdDate::read(&data[3..=4].try_into().unwrap()));
        let episode_number = data[5..=6].read_be_16() >> 4; // 12bit
        let last_episode_number = data[6..=7].read_be_16() & 0b0000_1111_1111_1111; // 12bit
        let series_name = &data[8..];

        Some(SeriesDescriptor {
            series_id,
            repeat_label,
            program_pattern,
            expire_date,
            episode_number,
            last_episode_number,
            series_name,
        })
    }
}

/// [`EventGroupDescriptor`]における`event`。
#[derive(Debug)]
pub struct ActualEvent {
    /// service_id
    pub service_id: u16,
    /// event_id
    pub event_id: u16,
}

/// [`EventGroup`]における`RelayToOtherNetworks`か`MovementFromOtherNetworks`に入る値。
#[derive(Debug)]
pub struct OtherNetwork {
    /// original_network_id
    pub original_network_id: u16,
    /// transport_stream_id
    pub transport_stream_id: u16,
    /// service_id
    pub service_id: u16,
    /// event_id
    pub event_id: u16,
}

/// [`EventGroupDescriptor`]における`group`。
#[derive(Debug)]
pub enum EventGroup<'a> {
    /// Event common
    Common(&'a [u8]),
    /// Event relay
    Relay(&'a [u8]),
    /// Event movement
    Movement(&'a [u8]),
    /// Event relay to other networks
    RelayToOtherNetworks(Vec<OtherNetwork>),
    /// Event movement from other networks
    MovementFromOtherNetworks(Vec<OtherNetwork>),
    /// Undefined
    Undefined(&'a [u8]),
}

/// イベントグループ記述子。
#[derive(Debug)]
pub struct EventGroupDescriptor<'a> {
    /// service_id + event_id
    pub events: Vec<ActualEvent>,
    /// group_type + ...
    pub group: EventGroup<'a>,
}

impl<'a> Descriptor<'a> for EventGroupDescriptor<'a> {
    const TAG: u8 = 0xD6;

    fn read(data: &'a [u8]) -> Option<EventGroupDescriptor<'a>> {
        fn read_other_networks(data: &[u8]) -> Vec<OtherNetwork> {
            data.chunks_exact(8)
                .map(|chunk| {
                    let original_network_id = chunk[0..=1].read_be_16();
                    let transport_stream_id = chunk[2..=3].read_be_16();
                    let service_id = chunk[4..=5].read_be_16();
                    let event_id = chunk[6..=7].read_be_16();
                    OtherNetwork {
                        original_network_id,
                        transport_stream_id,
                        service_id,
                        event_id,
                    }
                })
                .collect()
        }

        if data.len() < 1 {
            log::debug!("invalid EventGroupDescriptor");
            return None;
        }

        let group_type = (data[0] & 0b11110000) >> 4;
        let event_count = data[0] & 0b00001111;
        let Some((events, data)) = data[1..].split_at_checked(4 * event_count as usize) else {
            log::debug!("invalid EventGroupDescriptor::events");
            return None;
        };
        let events = events
            .chunks_exact(4)
            .map(|chunk| {
                let service_id = chunk[0..=1].read_be_16();
                let event_id = chunk[2..=3].read_be_16();

                ActualEvent {
                    service_id,
                    event_id,
                }
            })
            .collect();

        let group = match group_type {
            0x1 => EventGroup::Common(data),
            0x2 => EventGroup::Relay(data),
            0x3 => EventGroup::Movement(data),
            0x4 => EventGroup::RelayToOtherNetworks(read_other_networks(data)),
            0x5 => EventGroup::MovementFromOtherNetworks(read_other_networks(data)),
            _ => EventGroup::Undefined(data),
        };

        Some(EventGroupDescriptor { events, group })
    }
}

/// [`SiParameterDescriptor`]における`tables`の要素。
#[derive(Debug)]
pub struct SiParameterTable<'a> {
    /// table_id
    pub table_id: u8,
    /// table_description_byte
    pub table_description: &'a [u8],
}

/// SI伝送パラメータ記述子。
#[derive(Debug)]
pub struct SiParameterDescriptor<'a> {
    /// parameter_version
    pub parameter_version: u8,
    /// update_time
    pub update_time: MjdDate,
    /// table_id + table_description_byte
    pub tables: Vec<SiParameterTable<'a>>,
}

impl<'a> Descriptor<'a> for SiParameterDescriptor<'a> {
    const TAG: u8 = 0xD7;

    fn read(data: &'a [u8]) -> Option<SiParameterDescriptor<'a>> {
        if data.len() < 3 {
            log::debug!("invalid SiParameterDescriptor");
            return None;
        }

        let parameter_version = data[0];
        let update_time = MjdDate::read(&data[1..=2].try_into().unwrap());

        let mut data = &data[3..];
        let mut tables = Vec::new();
        while !data.is_empty() {
            let [table_id, table_description_length, ref rem @ ..] = *data else {
                log::debug!("invalid SiParameterDescriptor::table_id");
                return None;
            };
            let Some((table_description, rem)) = rem
                .split_at_checked(table_description_length as usize)
            else {
                log::debug!("invalid SiParameterDescriptor::table_description");
                return None;
            };
            data = rem;

            tables.push(SiParameterTable {
                table_id,
                table_description,
            });
        }

        Some(SiParameterDescriptor {
            parameter_version,
            update_time,
            tables,
        })
    }
}

/// ブロードキャスタ名記述子。
#[derive(Debug)]
pub struct BroadcasterNameDescriptor<'a> {
    /// char
    pub broadcaster_name: &'a [u8],
}

impl<'a> Descriptor<'a> for BroadcasterNameDescriptor<'a> {
    const TAG: u8 = 0xD8;

    fn read(data: &'a [u8]) -> Option<BroadcasterNameDescriptor<'a>> {
        Some(BroadcasterNameDescriptor {
            broadcaster_name: data,
        })
    }
}

/// [`ComponentGroup`]における`ca_units`の各要素。
#[derive(Debug)]
pub struct CaUnit<'a> {
    /// CA_unit_id (4bit)
    pub ca_unit_id: u8,
    /// component_tag
    pub component_tag: &'a [u8],
}

/// [`ComponentGroupDescriptor`]における`groups`の各要素。
#[derive(Debug)]
pub struct ComponentGroup<'a> {
    /// component_group_id (4bit)
    pub component_group_id: u8,
    /// [`CaUnit`]の配列。
    pub ca_units: Vec<CaUnit<'a>>,
    /// total_bit_rate
    pub total_bit_rate: Option<u8>,
    /// text_char
    pub text: &'a [u8],
}

/// コンポーネントグループ記述子。
#[derive(Debug)]
pub struct ComponentGroupDescriptor<'a> {
    /// component_group_type
    pub component_group_type: u8,
    /// num_of_group
    pub groups: Vec<ComponentGroup<'a>>,
}

impl<'a> Descriptor<'a> for ComponentGroupDescriptor<'a> {
    const TAG: u8 = 0xD9;

    fn read(data: &'a [u8]) -> Option<ComponentGroupDescriptor<'a>> {
        if data.len() < 1 {
            log::debug!("invalid ComponentGroupDescriptor");
            return None;
        }

        let component_group_type = (data[0] & 0b11100000) >> 5;
        let total_bit_rate_flag = data[0] & 0b00010000 != 0;
        let num_of_group = data[0] & 0b00001111;

        let mut data = &data[1..];
        let mut groups = Vec::with_capacity(num_of_group as usize);
        for _ in 0..num_of_group {
            if data.len() < 1 {
                log::debug!("invalid ComponentGroup");
                return None;
            }

            let component_group_id = (data[0] & 0b11110000) >> 4;
            let num_of_ca_unit = data[0] & 0b00001111;

            let mut ca_units = Vec::with_capacity(num_of_ca_unit as usize);
            for _ in 0..num_of_ca_unit {
                if data.len() < 1 {
                    log::debug!("invalid CaUnit");
                    return None;
                }

                let ca_unit_id = (data[0] & 0b11110000) >> 4;
                let num_of_component = data[0] & 0b00001111;
                let Some((component_tag, rem)) = data.split_at_checked(num_of_component as usize)
                else {
                    log::debug!("invalid CaUnit::component_tag");
                    return None;
                };
                data = rem;

                ca_units.push(CaUnit {
                    ca_unit_id,
                    component_tag,
                });
            }

            let total_bit_rate = if total_bit_rate_flag {
                let [total_bit_rate, ref rem @ ..] = *data else {
                    log::debug!("invalid ComponentGroup::total_bit_rate");
                    return None;
                };
                data = rem;

                Some(total_bit_rate)
            } else {
                None
            };

            let [text_length, ref rem @ ..] = *data else {
                log::debug!("invalid ComponentGroup::text_length");
                return None;
            };
            let Some((text, rem)) = rem.split_at_checked(text_length as usize) else {
                log::debug!("invalid ComponentGroup::text");
                return None
            };
            data = rem;

            groups.push(ComponentGroup {
                component_group_id,
                ca_units,
                total_bit_rate,
                text,
            });
        }

        Some(ComponentGroupDescriptor {
            component_group_type,
            groups,
        })
    }
}

/// [`LdtLinkageDescriptor`]における`linked_descriptors`の要素。
#[derive(Debug)]
pub struct LdtLinkedDescriptor {
    /// description_id
    pub description_id: u16,
    /// description_type (4bit)
    pub description_type: u8,
    /// user_defined
    pub user_defined: u8,
}

/// LDTリンク記述子。
#[derive(Debug)]
pub struct LdtLinkageDescriptor {
    /// original_service_id
    pub original_service_id: u16,
    /// transport_stream_id
    pub transport_stream_id: u16,
    /// original_network_id
    pub original_network_id: u16,
    /// description_id + description_type + user_defined
    pub descriptors: Vec<LdtLinkedDescriptor>,
}

impl Descriptor<'_> for LdtLinkageDescriptor {
    const TAG: u8 = 0xDC;

    fn read(data: &[u8]) -> Option<LdtLinkageDescriptor> {
        if data.len() < 6 {
            log::debug!("invalid LdtLinkageDescriptor");
            return None;
        }

        let original_service_id = data[0..=1].read_be_16();
        let transport_stream_id = data[2..=3].read_be_16();
        let original_network_id = data[4..=5].read_be_16();
        let descriptors = data[6..]
            .chunks_exact(4)
            .map(|chunk| {
                let description_id = chunk[0..=1].read_be_16();
                let description_type = chunk[2] & 0b00001111;
                let user_defined = chunk[3];

                LdtLinkedDescriptor {
                    description_id,
                    description_type,
                    user_defined,
                }
            })
            .collect();

        Some(LdtLinkageDescriptor {
            original_service_id,
            transport_stream_id,
            original_network_id,
            descriptors,
        })
    }
}

/// アクセス制御記述子。
#[derive(Debug)]
pub struct AccessControlDescriptor<'a> {
    /// CA_system_id
    pub ca_system_id: u16,
    /// Transmission_type
    pub transmission_type: u8,
    /// PID
    pub pid: Pid,
    /// private_data_byte
    pub private_data: &'a [u8],
}

impl<'a> Descriptor<'a> for AccessControlDescriptor<'a> {
    const TAG: u8 = 0xF6;

    fn read(data: &'a [u8]) -> Option<AccessControlDescriptor<'a>> {
        if data.len() < 4 {
            log::debug!("invalid AccessControlDescriptor");
            return None;
        }

        let ca_system_id = data[0..=1].read_be_16();
        let transmission_type = (data[2] & 0b11100000) >> 5;
        let pid = Pid::read(&data[2..]);
        let private_data = &data[4..];

        Some(AccessControlDescriptor {
            ca_system_id,
            transmission_type,
            pid,
            private_data,
        })
    }
}

/// [`TerrestrialDeliverySystemDescriptor`]の`guard_interval`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuardInterval {
    /// 1/32
    Guard1_32,
    /// 1/16
    Guard1_16,
    /// 1/8
    Guard1_8,
    /// 1/4
    Guard1_4,
}

/// [`TerrestrialDeliverySystemDescriptor`]の`transmission_mode`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransmissionMode {
    /// Mode 1
    Mode1,
    /// Mode 2
    Mode2,
    /// Mode 3
    Mode3,
    /// Undefined
    Undefined,
}

/// 地上分配システム記述子。
#[derive(Debug)]
pub struct TerrestrialDeliverySystemDescriptor {
    /// area_code
    pub area_code: u16,
    /// guard_interval
    pub guard_interval: GuardInterval,
    /// transmission_mode
    pub transmission_mode: TransmissionMode,
    /// frequency
    pub frequencies: Vec<u16>,
}

impl Descriptor<'_> for TerrestrialDeliverySystemDescriptor {
    const TAG: u8 = 0xFA;

    fn read(data: &[u8]) -> Option<TerrestrialDeliverySystemDescriptor> {
        if data.len() < 2 {
            log::debug!("invalid TerrestrialDeliverySystemDescriptor");
            return None;
        }

        let area_code = data[0..=1].read_be_16() >> 4; // 12bit
        let guard_interval = match (data[1] & 0b00001100) >> 2 {
            0b00 => GuardInterval::Guard1_32,
            0b01 => GuardInterval::Guard1_16,
            0b10 => GuardInterval::Guard1_8,
            0b11 => GuardInterval::Guard1_4,
            _ => unreachable!(),
        };
        let transmission_mode = match data[1] & 0b00000011 {
            0b00 => TransmissionMode::Mode1,
            0b01 => TransmissionMode::Mode2,
            0b10 => TransmissionMode::Mode3,
            0b11 => TransmissionMode::Undefined,
            _ => unreachable!(),
        };
        let frequencies = data[2..].chunks_exact(2).map(<[u8]>::read_be_16).collect();

        Some(TerrestrialDeliverySystemDescriptor {
            area_code,
            guard_interval,
            transmission_mode,
            frequencies,
        })
    }
}

/// 部分受信記述子。
#[derive(Debug)]
pub struct PartialReceptionDescriptor {
    /// service_id
    pub service_ids: Vec<u16>,
}

impl Descriptor<'_> for PartialReceptionDescriptor {
    const TAG: u8 = 0xFB;

    fn read(data: &[u8]) -> Option<PartialReceptionDescriptor> {
        let service_ids = data.chunks_exact(2).map(<[u8]>::read_be_16).collect();

        Some(PartialReceptionDescriptor { service_ids })
    }
}

/// [`Emergency`]における`signal_level`の値。
#[derive(Debug)]
pub enum SignalType {
    /// 1st type start signal
    First,
    /// 2nd type start signal
    Second,
}

/// [`EmergencyInformationDescriptor`]における`emergencies`の要素。
#[derive(Debug)]
pub struct Emergency {
    /// service_id
    pub service_id: u16,
    /// start_end_flag
    pub start_end_flag: bool,
    /// signal_level
    pub signal_level: SignalType,
    /// area_code (12bit)
    pub area_code: Vec<u16>,
}

/// 緊急情報記述子。
#[derive(Debug)]
pub struct EmergencyInformationDescriptor {
    /// [`Emergency`]の配列。
    pub emergencies: Vec<Emergency>,
}

impl Descriptor<'_> for EmergencyInformationDescriptor {
    const TAG: u8 = 0xFC;

    fn read(mut data: &[u8]) -> Option<EmergencyInformationDescriptor> {
        let mut emergencies = Vec::new();

        while !data.is_empty() {
            if data.len() < 4 {
                log::debug!("invalid EmergencyInformationDescriptor");
                return None;
            }

            let service_id = data[0..=1].read_be_16();
            let start_end_flag = data[2] & 0b10000000 != 0;
            let signal_level = match (data[2] & 0b01000000) >> 6 {
                0 => SignalType::First,
                1 => SignalType::Second,
                _ => unreachable!(),
            };
            let area_code_length = data[3];
            let Some((area_code, rem)) = data[4..].split_at_checked(2 * area_code_length as usize)
            else {
                log::debug!("invalid EmergencyInformationDescriptor::area_code");
                return None;
            };
            let area_code = area_code
                .chunks_exact(2)
                .map(<[u8]>::read_be_16)
                .map(|w| (w & 0b1111_1111_1111_0000) >> 4)
                .collect();
            data = rem;

            emergencies.push(Emergency {
                service_id,
                start_end_flag,
                signal_level,
                area_code,
            });
        }

        Some(EmergencyInformationDescriptor { emergencies })
    }
}

/// データコンポーネント記述子。
#[derive(Debug)]
pub struct DataComponentDescriptor<'a> {
    /// data_component_id
    pub data_component_id: u16,
    /// additional_data_component_info
    pub additional_data_component_info: &'a [u8],
}

impl<'a> Descriptor<'a> for DataComponentDescriptor<'a> {
    const TAG: u8 = 0xFD;

    fn read(data: &'a [u8]) -> Option<DataComponentDescriptor<'a>> {
        if data.len() < 2 {
            log::debug!("invalid DataComponentDescriptor");
            return None;
        }

        let data_component_id = data[0..=1].read_be_16();
        let additional_data_component_info = &data[2..];

        Some(DataComponentDescriptor {
            data_component_id,
            additional_data_component_info,
        })
    }
}

/// Broadcasting/non-broadcasting
#[derive(Debug)]
pub enum BroadcastingType {
    /// Broadcasting
    Broadcasting,
    /// Non-broadcasting
    Nonbroadcasting,
    /// Undefined
    Undefined,
}

/// Standard broadcasting system
#[derive(Debug)]
#[allow(non_camel_case_types)]
pub enum BroadcastingSystem {
    /// Standard system specified as digital satellite broadcasting using 27 MHz bandwidth in
    /// 12.2 to 12.75 GHz frequency band
    Satellite27MHz,
    /// Standard system specified as digital satellite broadcasting using 34.5 MHz bandwidth in
    /// 11.7 to 12.2 GHz frequency band
    Satellite34_5MHz_Low,
    /// Standard system specified as digital terrestrial television broadcasting.
    TerrestrialTelevision,
    /// Standard system specified as digital satellite broadcasting using 34.5 MHz bandwidth in
    /// 12.2 to 12.75 GHz frequency band
    Satellite34_5MHz_High,
    /// Standard system specified as digital terrestrial sound broadcasting
    TerrestrialSound,
    /// Standard system specified as broadcasting operated by broadcasting satellites or
    /// broadcasting stations in 2630 to 2655 MHz frequency band.
    Satellites,
    /// Standard system specified as digital satellite broadcasting based on advanced narrow-band
    /// transmission system using 27 MHz bandwidth in 12.2 to 12.75 GHz frequency band
    Narrowband,

    /// Undefined (6bit, 0b000000 or 0b001000-0b111111)
    Undefined(u8),
}

/// システム管理記述子。
#[derive(Debug)]
pub struct SystemManagementDescriptor<'a> {
    /// broadcasting_flag
    pub broadcasting_flag: BroadcastingType,
    /// broadcasting_identifier (6bit)
    pub broadcasting_identifier: BroadcastingSystem,
    /// additional_broadcasting_identification
    pub additional_broadcasting_identification: u8,
    /// additional_identification_info
    pub additional_identification_info: &'a [u8],
}

impl<'a> Descriptor<'a> for SystemManagementDescriptor<'a> {
    const TAG: u8 = 0xFE;

    fn read(data: &'a [u8]) -> Option<SystemManagementDescriptor<'a>> {
        if data.len() < 2 {
            log::debug!("invalid SystemManagementDescriptor");
            return None;
        }

        let broadcasting_flag = match (data[0] & 0b11000000) >> 6 {
            0b00 => BroadcastingType::Broadcasting,
            0b01 | 0b10 => BroadcastingType::Nonbroadcasting,
            0b11 => BroadcastingType::Undefined,
            _ => unreachable!(),
        };
        let broadcasting_identifier = match data[0] & 0b00111111 {
            0b000001 => BroadcastingSystem::Satellite27MHz,
            0b000010 => BroadcastingSystem::Satellite34_5MHz_Low,
            0b000011 => BroadcastingSystem::TerrestrialTelevision,
            0b000100 => BroadcastingSystem::Satellite34_5MHz_High,
            0b000101 => BroadcastingSystem::TerrestrialSound,
            0b000110 => BroadcastingSystem::Satellites,
            0b000111 => BroadcastingSystem::Narrowband,
            id @ (0b000000 | 0b000111..=0b111111) => BroadcastingSystem::Undefined(id),
            _ => unreachable!(),
        };
        let additional_broadcasting_identification = data[1];
        let additional_identification_info = &data[2..];

        Some(SystemManagementDescriptor {
            broadcasting_flag,
            broadcasting_identifier,
            additional_broadcasting_identification,
            additional_identification_info,
        })
    }
}
