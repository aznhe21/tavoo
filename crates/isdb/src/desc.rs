//! ARIB STD-B10やARIB STD-B21で規定される記述子とその関連。

use std::fmt;

use crate::pid::Pid;
use crate::time::{DateTime, MjdDate};
use crate::types::{Polarization, ServiceType, StreamType};
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
    /// `data`から`length`バイト分の記述子群を読み取り後続データと共に返す。
    ///
    /// 記述子の内容はパースせず、`get`メソッドで初めてパースする。
    ///
    /// データ長が不足している場合は`None`を返す。
    // `length`が`u16`なのは規格上`u16`以上の長さになることがなく、
    // 呼び出し側でのキャストが無意味であるため。
    pub fn read_with_len(data: &'a [u8], length: u16) -> Option<(DescriptorBlock<'a>, &'a [u8])> {
        let (block, rem) = data.split_at_checked(length as usize)?;
        Some((DescriptorBlock(block), rem))
    }

    /// `data`から記述子群を読み取り後続データと共に返す。
    ///
    /// 記述子の内容はパースせず、`get`メソッドで初めてパースする。
    ///
    /// データ長が不足している場合は`None`を返す。
    #[inline]
    pub fn read(data: &'a [u8]) -> Option<(DescriptorBlock<'a>, &'a [u8])> {
        if data.len() < 2 {
            return None;
        }

        let length = data[0..=1].read_be_16() & 0b0000_1111_1111_1111;
        DescriptorBlock::read_with_len(&data[2..], length)
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
    /// 限定受信方式識別。
    pub ca_system_id: u16,
    /// 限定受信PID。
    pub ca_pid: Pid,
    /// プライベートデータ。
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
        let ca_pid = Pid::read(&data[2..=3]);
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
    /// ネットワーク名
    // TODO: 文字符号
    pub network_name: &'a [u8],
}

impl<'a> Descriptor<'a> for NetworkNameDescriptor<'a> {
    const TAG: u8 = 0x40;

    fn read(data: &'a [u8]) -> Option<NetworkNameDescriptor<'a>> {
        Some(NetworkNameDescriptor { network_name: data })
    }
}

/// サービスリスト記述子におけるサービス。
#[derive(Debug)]
pub struct ServiceEntry {
    /// サービス識別。
    pub service_id: u16,
    /// サービス形式種別。
    pub service_type: ServiceType,
}

/// サービスリスト記述子。
#[derive(Debug)]
pub struct ServiceListDescriptor {
    /// サービスを格納する配列。
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
    /// 周波数（単位は10kHz）。
    pub frequency: u32,
    /// 軌道。
    pub orbital_position: u16,
    /// 東経西経フラグ。
    pub west_east_flag: bool,
    /// 偏波。
    pub polarization: Polarization,
    /// 変調（5ビット）。
    pub modulation: u8,
    /// シンボルレート。
    pub symbol_rate: u32,
    /// FEC（内符号、4ビット）。
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
        let polarization = match (data[6] & 0b01100000) >> 5 {
            0b00 => Polarization::LinearHorizontal,
            0b01 => Polarization::LinearVertical,
            0b10 => Polarization::CircularLeft,
            0b11 => Polarization::CircularRight,
            _ => unreachable!(),
        };
        let modulation = data[6] & 0b00011111;
        let symbol_rate = data[7..=10].read_bcd(7);
        let fec_inner = data[10] & 0b00001111;

        Some(SatelliteDeliverySystemDescriptor {
            frequency,
            orbital_position,
            west_east_flag,
            polarization,
            modulation,
            symbol_rate,
            fec_inner,
        })
    }
}

/// 有線分配システム記述子。
#[derive(Debug)]
pub struct CableDeliverySystemDescriptor {
    /// 周波数（単位は100Hz）。
    pub frequency: u32,
    /// 多重フレーム形式番号（4ビット）。
    pub frame_type: u8,
    /// FEC（外側、4ビット）。
    pub fec_outer: u8,
    /// 変調。
    pub modulation: u8,
    /// シンボルレート（28ビット）。
    pub symbol_rate: u32,
    /// FEC（内側、4ビット）。
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
    /// サービス形式種別。
    pub service_type: ServiceType,
    /// 事業者名。
    // TODO: 文字符号
    pub service_provider_name: &'a [u8],
    /// サービス名。
    // TODO: 文字符号
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
    /// トランスポートストリーム識別。
    pub transport_stream_id: u16,
    /// オリジナルネットワーク識別。
    pub original_network_id: u16,
    /// サービス識別。
    pub service_id: u16,
    /// リンク種別（4ビット）。
    pub linkage_type: u8,
    /// プライベートデータ。
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
    /// ISO 639-2で規定される3文字の言語コード。
    pub lang_code: [u8; 3],
    /// 番組名。
    // TODO: 文字符号
    pub event_name: &'a [u8],
    /// 番組記述。
    // TODO: 文字符号
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

/// 拡張形式イベント記述子における項目。
#[derive(Debug)]
pub struct ExtendedEventItem<'a> {
    /// 項目名。
    // TODO: 文字符号
    pub item_description: &'a [u8],
    /// 項目記述。
    // TODO: 文字符号
    pub item: &'a [u8],
}

/// 拡張形式イベント記述子。
#[derive(Debug)]
pub struct ExtendedEventDescriptor<'a> {
    /// 記述子番号（4ビット）。
    pub descriptor_number: u8,
    /// 最終記述子番号（4ビット）。
    pub last_descriptor_number: u8,
    /// ISO 639-2で規定される3文字の言語コード。
    pub lang_code: [u8; 3],
    /// 項目を格納する配列。
    pub items: Vec<ExtendedEventItem<'a>>,
    /// 拡張記述。
    // TODO: 文字符号
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
    /// コンポーネント内容（4ビット）。
    pub stream_content: u8,
    /// コンポーネント種別。
    pub component_type: u8,
    /// コンポーネントタグ。
    pub component_tag: u8,
    /// ISO 639-2で規定される3文字の言語コード。
    pub lang_code: [u8; 3],
    /// コンポーネント記述。
    // TODO: 文字符号
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
    /// コンポーネントタグ。
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
    /// ジャンル1（4ビット）。
    pub large_genre_classification: u8,
    /// ジャンル2（4ビット）。
    pub middle_genre_classification: u8,
    /// ユーザジャンル（4ビット）。
    pub user_genre_1: u8,
    /// ユーザジャンル（4ビット）。
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
    /// ISO 3166-1で規定される3文字の言語コード。
    pub country_code: [u8; 3],
    /// 国地域識別（6ビット）。
    pub country_region_id: u8,
    /// ローカル時間オフセット極性。
    pub local_time_offset_polarity: bool,
    /// ローカル時間オフセット。
    pub local_time_offset: u16,
    /// 変更時刻。
    pub time_of_change: DateTime,
    /// 変更後時間オフセット。
    pub next_time_offset: u16,
}

/// ローカル時間オフセット記述子。
#[derive(Debug)]
pub struct LocalTimeOffsetDescriptor {
    /// ローカル時間オフセットを格納する配列。
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
    /// 階層レベル。
    pub high_quality: bool,
    /// 参照先PID。
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
        let reference_pid = Pid::read(&data[1..=2]);

        Some(HierarchicalTransmissionDescriptor {
            high_quality,
            reference_pid,
        })
    }
}

/// デジタルコピー制御記述子におけるコンポーネント制御情報。
#[derive(Debug)]
pub struct ComponentControlEntry {
    /// コンポーネントタグ。
    pub component_tag: u8,
    /// デジタルコピー制御情報（2ビット）。
    pub digital_recording_control_data: u8,
    /// コピー制御形式情報（2ビット）。
    pub copy_control_type: u8,
    /// アナログ出力コピー制御情報（2ビット）。
    pub aps_control_data: Option<u8>,
    /// 最大伝送レート。
    pub maximum_bitrate: Option<u8>,
}

/// デジタルコピー制御記述子。
#[derive(Debug)]
pub struct DigitalCopyControlDescriptor {
    /// デジタルコピー制御情報（2ビット）。
    pub digital_recording_control_data: u8,
    /// コピー制御形式情報（2ビット）。
    pub copy_control_type: u8,
    /// アナログ出力コピー制御情報（2ビット）。
    pub aps_control_data: Option<u8>,
    /// 最大伝送レート。
    pub maximum_bitrate: Option<u8>,
    /// 番組を構成するコンポーネントごとのデジタルコピー制御情報。
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

/// 音声コンポーネント記述子における音質表示。
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

/// 音声コンポーネント記述子におけるサンプリング周波数。
#[derive(Debug)]
pub enum SamplingFrequency {
    /// 予約
    Reserved,
    /// 16kHZ
    SF16k,
    /// 22.05kHZ
    SF22_05k,
    /// 24kHZ
    SF24k,
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
    /// コンポーネント内容（4ビット）。
    pub stream_content: u8,
    /// コンポーネント種別。
    pub component_type: u8,
    /// コンポーネントタグ。
    pub component_tag: u8,
    /// ストリーム形式種別。
    pub stream_type: StreamType,
    /// サイマルキャストグループ識別。
    pub simulcast_group_tag: u8,
    /// 主コンポーネントフラグ。
    pub main_component_flag: bool,
    /// 音質表示。
    pub quality_indicator: QualityIndicator,
    /// サンプリング周波数。
    pub sampling_rate: SamplingFrequency,
    /// ISO 639-2で規定される3文字の言語コード。
    pub lang_code: [u8; 3],
    /// ISO 639-2で規定される3文字の言語コードその2。
    pub lang_code_2: Option<[u8; 3]>,
    /// コンポーネント記述。
    // TODO: 文字符号
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
            0b000 | 0b100 => SamplingFrequency::Reserved,
            0b001 => SamplingFrequency::SF16k,
            0b010 => SamplingFrequency::SF22_05k,
            0b011 => SamplingFrequency::SF24k,
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

/// ハイパーリンク記述子におけるリンク先。
#[derive(Debug)]
pub enum SelectorInfo<'a> {
    /// サービス。
    LinkServiceInfo(LinkServiceInfo),
    /// イベント。
    LinkEventInfo(LinkEventInfo),
    /// イベントの特定モジュール。
    LinkModuleInfo(LinkModuleInfo),
    /// コンテント。
    LinkContentInfo(LinkContentInfo),
    /// コンテントの特定モジュール。
    LinkContentModuleInfo(LinkContentModuleInfo),
    /// イベント関係テーブルのノード。
    LinkErtNodeInfo(LinkErtNodeInfo),
    /// 蓄積コンテント。
    LinkStoredContentInfo(LinkStoredContentInfo<'a>),
    /// 不明。
    Unknown(LinkUnknown<'a>),
}

/// サービス。
#[derive(Debug)]
pub struct LinkServiceInfo {
    /// オリジナルネットワーク識別。
    pub original_network_id: u16,
    /// トランスポートストリーム識別。
    pub transport_stream_id: u16,
    /// サービス識別。
    pub service_id: u16,
}

/// イベント。
#[derive(Debug)]
pub struct LinkEventInfo {
    /// オリジナルネットワーク識別。
    pub original_network_id: u16,
    /// トランスポートストリーム識別。
    pub transport_stream_id: u16,
    /// サービス識別。
    pub service_id: u16,
    /// イベント識別。
    pub event_id: u16,
}

/// イベントの特定モジュール。
#[derive(Debug)]
pub struct LinkModuleInfo {
    /// オリジナルネットワーク識別。
    pub original_network_id: u16,
    /// トランスポートストリーム識別。
    pub transport_stream_id: u16,
    /// サービス識別。
    pub service_id: u16,
    /// イベント識別。
    pub event_id: u16,
    /// コンポーネントタグ。
    pub component_tag: u8,
    /// モジュール識別。
    pub module_id: u16,
}

/// コンテント。
#[derive(Debug)]
pub struct LinkContentInfo {
    /// オリジナルネットワーク識別。
    pub original_network_id: u16,
    /// トランスポートストリーム識別。
    pub transport_stream_id: u16,
    /// サービス識別。
    pub service_id: u16,
    /// コンテンツ識別。
    pub content_id: u32,
}

/// コンテントの特定モジュール。
#[derive(Debug)]
pub struct LinkContentModuleInfo {
    /// オリジナルネットワーク識別。
    pub original_network_id: u16,
    /// トランスポートストリーム識別。
    pub transport_stream_id: u16,
    /// サービス識別。
    pub service_id: u16,
    /// コンテンツ識別。
    pub content_id: u32,
    /// コンポーネントタグ。
    pub component_tag: u8,
    /// モジュール識別。
    pub module_id: u16,
}

/// イベント関係テーブルのノード。
#[derive(Debug)]
pub struct LinkErtNodeInfo {
    /// 情報提供者識別。
    pub information_provider_id: u16,
    /// イベント関係識別。
    pub event_relation_id: u16,
    /// ノード識別。
    pub node_id: u16,
}

/// 蓄積コンテント。
#[derive(Debug)]
pub struct LinkStoredContentInfo<'a> {
    /// URI文字
    // TODO: 文字符号？
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
    /// ハイパーリンク種別。
    pub hyper_linkage_type: u8,
    /// リンク先。
    pub selector: SelectorInfo<'a>,
    /// プライベートデータ。
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

/// 対象地域記述子における地域記述方式。
#[derive(Debug)]
pub enum TargetRegionSpec<'a> {
    /// BSデジタル用県域指定。
    BsPrefectureSpec(BsPrefectureSpec),
    /// 不明な地域記述方式。
    Unknown(&'a [u8]),
}

/// 県域指定ビットマップ。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrefectureBitmap(pub [u8; 7]);

macro_rules! prefactures {
    ($($method:ident($bit:literal) => $jp:literal,)*) => {
        $(
            #[doc = concat!($jp, "が対象であるかどうかを返す。")]
            #[inline]
            pub fn $method(&self) -> bool {
                self.0[($bit - 1) / 8] & (1 << (($bit - 1) % 8)) != 0
            }
        )*
    };
}

impl PrefectureBitmap {
    prefactures! {
        has_east_hokkaido(1) => "東北海道",
        has_west_hokkaido(2) => "西北海道",
        has_aomori(3) => "青森県",
        has_iwate(4) => "岩手県",
        has_miyagi(5) => "宮城県",
        has_akita(6) => "秋田県",
        has_yamagata(7) => "山形県",
        has_fukushima(8) => "福島県",
        has_ibaraki(9) => "茨城県",
        has_tochigi(10) => "栃木県",
        has_gunma(11) => "群馬県",
        has_saitama(12) => "埼玉県",
        has_chiba(13) => "千葉県",
        has_tokyo(14) => "東京都（島部を除く）",
        has_kanagawa(15) => "神奈川県",
        has_niigata(16) => "新潟県",
        has_toyama(17) => "富山県",
        has_ishikawa(18) => "石川県",
        has_fukui(19) => "福井県",
        has_yamanashi(20) => "山梨県",
        has_nagano(21) => "長野県",
        has_gifu(22) => "岐阜県",
        has_shizuoka(23) => "静岡県",
        has_aichi(24) => "愛知県",
        has_mie(25) => "三重県",
        has_shiga(26) => "滋賀県",
        has_kyoto(27) => "京都府",
        has_osaka(28) => "大阪府",
        has_hyogo(29) => "兵庫県",
        has_nara(30) => "奈良県",
        has_wakayama(31) => "和歌山県",
        has_tottori(32) => "鳥取県",
        has_shimane(33) => "島根県",
        has_okayama(34) => "岡山県",
        has_hiroshima(35) => "広島県",
        has_yamaguchi(36) => "山口県",
        has_tokushima(37) => "徳島県",
        has_kagawa(38) => "香川県",
        has_ehime(39) => "愛媛県",
        has_kochi(40) => "高知県",
        has_fukuoka(41) => "福岡県",
        has_saga(42) => "佐賀県",
        has_nagasaki(43) => "長崎県",
        has_kumamoto(44) => "熊本県",
        has_oita(45) => "大分県",
        has_miyazaki(46) => "宮崎県",
        has_kagoshima(47) => "鹿児島県（南西諸島を除く）",
        has_okinawa(48) => "沖縄県",
        has_tokyo_island(49) => "東京都島部（伊豆・小笠原諸島）",
        has_kagoshima_island(50) => "鹿児島県島部（南西諸島の鹿児島県域）",
    }
}

/// BSデジタル用県域指定。
#[derive(Debug)]
pub struct BsPrefectureSpec {
    /// 県域指定ビットマップ。
    pub prefecture_bitmap: PrefectureBitmap,
}

/// 対象地域記述子。
#[derive(Debug)]
pub struct TargetRegionDescriptor<'a> {
    /// 地域記述方式。
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
                let prefecture_bitmap = PrefectureBitmap(prefecture_bitmap);

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
    /// 静止画フラグ。
    pub still_picture_flag: bool,
    /// シーケンスエンドコードフラグ。
    pub sequence_end_code_flag: bool,
    /// ビデオエンコードフォーマット（4ビット）。
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

/// ダウンロードコンテンツ記述子におけるモジュール。
#[derive(Debug)]
pub struct ModuleInfo<'a> {
    /// モジュール識別。
    pub module_id: u16,
    /// 当該モジュールのバイト長。
    pub module_size: u32,
    /// DIIにて記述される記述子。
    pub module_info: &'a [u8],
}

/// ダウンロードコンテンツ記述子におけるサービス記述。
#[derive(Debug)]
pub struct ServiceDescription<'a> {
    /// ISO 639-2で規定される3文字の言語コード。
    pub lang_code: [u8; 3],
    /// サービス記述。
    // TODO: 文字符号？
    pub text: &'a [u8],
}

/// ダウンロードコンテンツ記述子。
#[derive(Debug)]
pub struct DownloadContentDescriptor<'a> {
    /// 再起動要否フラグ。
    pub reboot: bool,
    /// 既存モジュール追加フラグ。
    pub add_on: bool,
    /// コンポーネントサイズ。
    pub component_size: u32,
    /// ダウンロード識別。
    pub download_id: u32,
    /// DIIタイムアウト値（単位はミリ秒）。
    pub time_out_value_dii: u32,
    /// リークレート（単位は50bytes/s）。
    pub leak_rate: u32,
    /// コンポーネントタグ
    pub component_tag: u8,
    /// compatibilityDescriptor,
    pub compatibility_descriptors: Option<Vec<CompatibilityDescriptor<'a>>>,
    /// モジュールごとの情報。
    pub modules: Option<Vec<ModuleInfo<'a>>>,
    /// プライベートデータ。
    pub private_data: &'a [u8],
    /// サービス記述。
    pub service_descs: Option<ServiceDescription<'a>>,
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
            data = &data[2..];

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

        let service_descs = if text_info_flag {
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

            Some(ServiceDescription { lang_code, text })
        } else {
            None
        };

        Some(DownloadContentDescriptor {
            reboot,
            add_on,
            component_size,
            download_id,
            time_out_value_dii,
            leak_rate,
            component_tag,
            compatibility_descriptors,
            modules,
            private_data,
            service_descs,
        })
    }
}

/// CA_EMM_TS記述子。
#[derive(Debug)]
pub struct CaEmmTsDescriptor {
    /// 限定受信方式識別。
    pub ca_system_id: u16,
    /// トランスポートストリーム識別。
    pub transport_stream_id: u16,
    /// オリジナルネットワーク識別。
    pub original_network_id: u16,
    /// 電源保持時間（単位は分）。
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
    /// 限定受信方式識別。
    pub ca_system_id: u16,
    /// 課金単位／非課金単位の識別（4ビット）。
    pub ca_unit_id: u8,
    /// コンポーネントタグ。
    pub component_tag: &'a [u8],
    /// 契約確認情報。
    pub contract_verification_info: &'a [u8],
    /// 料金名称。
    // TODO: 文字符号？
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
    /// 限定受信方式識別。
    pub ca_system_id: u16,
    /// 事業体識別。
    pub ca_broadcaster_group_id: u8,
    /// 猶予期間。
    pub message_control: u8,
    /// サービス識別。
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

/// TS情報記述子における伝送種別。
#[derive(Debug)]
pub struct TsInformationTransmissionType {
    /// 伝承種別情報。
    pub transmission_type_info: u8,
    /// サービス識別。
    pub service_ids: Vec<u16>,
}

/// TS情報記述子。
#[derive(Debug)]
pub struct TsInformationDescriptor<'a> {
    /// リモコンキー識別。
    pub remote_control_key_id: u8,
    /// TS名記述。
    // TODO: 文字符号
    pub ts_name: &'a [u8],
    /// 伝送種別を格納する配列。
    pub transmission_types: Vec<TsInformationTransmissionType>,
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

        let mut transmission_types = Vec::with_capacity(transmission_type_count as usize);
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

            transmission_types.push(TsInformationTransmissionType {
                transmission_type_info,
                service_ids,
            });
        }

        Some(TsInformationDescriptor {
            remote_control_key_id,
            ts_name,
            transmission_types,
        })
    }
}

/// 拡張ブロードキャスタ識別におけるブロードキャスタ識別。
#[derive(Debug)]
pub struct BroadcasterId {
    /// オリジナルネットワーク識別。
    pub original_network_id: u16,
    /// ブロードキャスタ識別。
    pub broadcaster_id: u8,
}

/// 地上デジタルテレビジョン放送。
#[derive(Debug)]
pub struct DigitalTerrestrialTelevisionBroadcast<'a> {
    /// 地上ブロードキャスタ識別。
    pub terrestrial_broadcaster_id: u16,
    /// 系列識別。
    pub affiliation_id: &'a [u8],
    /// ブロードキャスタ識別を格納する配列。
    pub broadcaster_ids: Vec<BroadcasterId>,
    /// プライベートデータ。
    pub private_data: &'a [u8],
}

/// 地上デジタル音声放送。
#[derive(Debug)]
pub struct DigitalTerrestrialSoundBroadcast<'a> {
    /// 地上音声ブロードキャスタ識別。
    pub terrestrial_sound_broadcaster_id: u16,
    /// 音声放送系列識別。
    pub sound_broadcast_affiliation_id: &'a [u8],
    /// ブロードキャスタ識別を格納する配列。
    pub broadcaster_ids: Vec<BroadcasterId>,
    /// プライベートデータ。
    pub private_data: &'a [u8],
}

/// 拡張ブロードキャスタ記述子。
#[derive(Debug)]
pub enum ExtendedBroadcasterDescriptor<'a> {
    /// 地上デジタルテレビジョン放送。
    DigitalTerrestrialTelevisionBroadcast(DigitalTerrestrialTelevisionBroadcast<'a>),
    /// 地上デジタル音声放送。
    DigitalTerrestrialSoundBroadcast(DigitalTerrestrialSoundBroadcast<'a>),
    /// 未定義。
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

/// CDT伝送方式1。
///
/// CDTをダウンロードデータ識別で直接参照する場合。
#[derive(Debug)]
pub struct LogoTransmissionCdt1 {
    /// ロゴ識別（9ビット）。
    pub logo_id: u16,
    /// ロゴバージョン番号（12ビット）。
    pub logo_version: u16,
    /// ダウンロードデータ識別。
    pub download_data_id: u16,
}

/// CDT伝送方式2．
///
/// CDTをロゴ識別を用いてダウンロードデータ識別を間接的に参照する場合。
#[derive(Debug)]
pub struct LogoTransmissionCdt2 {
    /// ロゴ識別（9ビット）。
    pub logo_id: u16,
}

/// ロゴ伝送記述子。
#[derive(Debug)]
pub enum LogoTransmissionDescriptor<'a> {
    /// CDT伝送方式1。
    Cdt1(LogoTransmissionCdt1),
    /// CDT伝送方式2．
    Cdt2(LogoTransmissionCdt2),
    /// 簡易ロゴ方式。
    Simple(&'a [u8]),
    /// 予約。
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

/// シリーズ記述子における編成パターン。
#[derive(Debug)]
pub enum ProgramPattern {
    /// 不定期。
    Nonscheduled,
    /// 帯番組（毎日、平日のみ毎日、土・日のみなど）、週に複数回の編成。
    Regular,
    /// 週に1回程度の編成。
    OnceAWeek,
    /// 月に1回程度の編成。
    OnceAMonth,
    /// 同日内に複数話数の編成。
    SeveralEventsInADay,
    /// 長時間番組の分割。
    Division,
    /// 定期または不定期の蓄積用の編成。
    Accumulation,
    /// 未定義。
    Undefined,
}

/// シリーズ記述子。
#[derive(Debug)]
pub struct SeriesDescriptor<'a> {
    /// シリーズ識別。
    pub series_id: u16,
    /// 再放送ラベル（4ビット）。
    pub repeat_label: u8,
    /// 編成パターン。
    pub program_pattern: ProgramPattern,
    /// 有効期限。
    pub expire_date: Option<MjdDate>,
    /// 話数（12ビット）。
    pub episode_number: u16,
    /// 番組総数（12ビット）。
    pub last_episode_number: u16,
    /// シリーズ名。
    // TODO: 文字符号
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

/// イベントグループ記述子におけるイベント。
#[derive(Debug)]
pub struct ActualEvent {
    /// サービス識別。
    pub service_id: u16,
    /// イベント識別。
    pub event_id: u16,
}

/// イベントグループ記述子における`RelayToOtherNetworks`か`MovementFromOtherNetworks`に入る値。
#[derive(Debug)]
pub struct OtherNetwork {
    /// オリジナルネットワーク識別。
    pub original_network_id: u16,
    /// トランスポートストリーム識別。
    pub transport_stream_id: u16,
    /// サービス識別。
    pub service_id: u16,
    /// イベント識別。
    pub event_id: u16,
}

/// イベントグループ記述子におけるグループ。
#[derive(Debug)]
pub enum EventGroup<'a> {
    /// イベント共有。
    Common(&'a [u8]),
    /// イベントリレー。
    Relay(&'a [u8]),
    /// イベント移動。
    Movement(&'a [u8]),
    /// 他ネットワークへのイベントリレー。
    RelayToOtherNetworks(Vec<OtherNetwork>),
    /// 他ネットワークからのイベント移動。
    MovementFromOtherNetworks(Vec<OtherNetwork>),
    /// 未定義。
    Undefined(&'a [u8]),
}

/// イベントグループ記述子。
#[derive(Debug)]
pub struct EventGroupDescriptor<'a> {
    /// イベントを格納する配列。
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

/// SI伝送パラメータ記述子におけるテーブル。
#[derive(Debug)]
pub struct SiParameterTable<'a> {
    /// テーブル識別。
    pub table_id: u8,
    /// テーブル記述。
    pub table_description: &'a [u8],
}

/// SI伝送パラメータ記述子。
#[derive(Debug)]
pub struct SiParameterDescriptor<'a> {
    /// パラメータバージョン。
    pub parameter_version: u8,
    /// 更新日。
    pub update_time: MjdDate,
    /// テーブルを格納する配列。
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
    /// ブロードキャスタ名。
    // TODO: 文字符号
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

/// コンポーネントグループ記述子における課金単位。
#[derive(Debug)]
pub struct CaUnit<'a> {
    /// 課金単位識別。
    pub ca_unit_id: u8,
    /// コンポーネントタグ。
    pub component_tag: &'a [u8],
}

/// コンポーネントグループ記述子におけるグループ。
#[derive(Debug)]
pub struct ComponentGroup<'a> {
    /// コンポーネントグループ識別（4ビット）。
    pub component_group_id: u8,
    /// 課金単位を格納する配列。
    pub ca_units: Vec<CaUnit<'a>>,
    /// トータルビットレート。
    pub total_bit_rate: Option<u8>,
    /// コンポーネントグループ記述。
    // TODO: 文字符号
    pub text: &'a [u8],
}

/// コンポーネントグループ記述子。
#[derive(Debug)]
pub struct ComponentGroupDescriptor<'a> {
    /// コンポーネントグループ種別（3ビット）。
    pub component_group_type: u8,
    /// コンポーネントグループを格納する配列。
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
            data = &data[1..];

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

/// LDTリンク記述子におけるリンク先の記述に関する情報。
#[derive(Debug)]
pub struct LdtLinkedDescriptor {
    /// 記述識別。
    pub description_id: u16,
    /// 記述形式識別（4ビット）。
    pub description_type: u8,
    /// 事業者定義ビット。
    pub user_defined: u8,
}

/// LDTリンク記述子。
#[derive(Debug)]
pub struct LdtLinkageDescriptor {
    /// オリジナルサービス識別。
    pub original_service_id: u16,
    /// トランスポートストリーム識別。
    pub transport_stream_id: u16,
    /// オリジナルネットワーク識別。
    pub original_network_id: u16,
    /// リンク先の記述を格納する配列。
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
    /// 限定受信方式識別。
    pub ca_system_id: u16,
    /// 伝送情報。
    pub transmission_type: u8,
    /// PID。
    pub pid: Pid,
    /// プライベートデータ。
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
        let pid = Pid::read(&data[2..=3]);
        let private_data = &data[4..];

        Some(AccessControlDescriptor {
            ca_system_id,
            transmission_type,
            pid,
            private_data,
        })
    }
}

/// 地上分配システム記述子におけるガードインターバル。
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

/// 地上分配システム記述子におけるモード情報。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransmissionMode {
    /// Mode 1。
    Mode1,
    /// Mode 2。
    Mode2,
    /// Mode 3。
    Mode3,
    /// 未定義。
    Undefined,
}

/// 地上分配システム記述子。
#[derive(Debug)]
pub struct TerrestrialDeliverySystemDescriptor {
    /// エリアコード。
    pub area_code: u16,
    /// ガードインターバル。
    pub guard_interval: GuardInterval,
    /// モード情報。
    pub transmission_mode: TransmissionMode,
    /// 周波数を格納する配列。単位は1/7MHz。
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
    /// サービス識別。
    pub service_ids: Vec<u16>,
}

impl Descriptor<'_> for PartialReceptionDescriptor {
    const TAG: u8 = 0xFB;

    fn read(data: &[u8]) -> Option<PartialReceptionDescriptor> {
        let service_ids = data.chunks_exact(2).map(<[u8]>::read_be_16).collect();

        Some(PartialReceptionDescriptor { service_ids })
    }
}

/// 緊急情報記述子における信号種別。
#[derive(Debug)]
pub enum SignalType {
    /// 第1種開始信号。
    First,
    /// 第2種開始信号。
    Second,
}

/// 緊急情報記述子における緊急情報。
#[derive(Debug)]
pub struct Emergency {
    /// サービス識別。
    pub service_id: u16,
    /// 開始／終了フラグ。
    pub start_end_flag: bool,
    /// 信号種別。
    pub signal_level: SignalType,
    /// 地域符号を格納する配列（各12ビット）。
    pub area_code: Vec<u16>,
}

/// 緊急情報記述子。
#[derive(Debug)]
pub struct EmergencyInformationDescriptor {
    /// 緊急情報を格納する配列。
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

/// データ符号化記述子。
#[derive(Debug)]
pub struct DataComponentDescriptor<'a> {
    /// データ符号化方式識別。
    pub data_component_id: u16,
    /// 付加識別情報。
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

/// 放送／非放送種別。
#[derive(Debug)]
pub enum BroadcastingType {
    /// 放送。
    Broadcasting,
    /// 非放送。
    Nonbroadcasting,
    /// 未定義。
    Undefined,
}

/// 放送の標準方式種別（6ビット）。
// 未定義の値も扱うため列挙型ではなく構造体にする。
#[derive(Debug)]
pub struct BroadcastingSystem(pub u8);

impl BroadcastingSystem {
    /// 12.2～12.75GHzの周波数帯において27MHz帯域幅を使用する狭帯域伝送方式による
    /// 衛星デジタル放送として規定する標準方式。
    pub const NARROWBAND_27MHZ: BroadcastingSystem = BroadcastingSystem(0b000001);
    /// 11.7～12.2GHzの周波数帯において34.5MHz帯域幅を使用する狭帯域伝送方式による
    /// 衛星デジタル放送として規定する標準方式。
    pub const NARROWBAND_34_5MHZ_LOW: BroadcastingSystem = BroadcastingSystem(0b000010);
    /// 地上デジタルテレビジョン放送として規定する標準方式。
    pub const TERRESTRIAL_TELEVISION: BroadcastingSystem = BroadcastingSystem(0b000011);
    /// 12.2～12.75GHzの周波数帯において34.5MHz帯域幅を使用する狭帯域伝送方式による
    /// 衛星デジタル放送として規定する標準方式。
    pub const NARROWBAND_34_5MHZ_HIGH: BroadcastingSystem = BroadcastingSystem(0b000100);
    /// 地上デジタル音声放送として規定する標準方式。
    pub const TERRESTRIAL_SOUND: BroadcastingSystem = BroadcastingSystem(0b000101);
    /// 12.2～12.75GHzの周波数帯において27MHz帯域幅を使用する高度狭帯域伝送方式による
    /// 衛星デジタル放送として規定する標準方式。
    pub const ADVANCED_NARROWBAND_27MHZ: BroadcastingSystem = BroadcastingSystem(0b000111);
    /// 11.7～12.2GHzの周波数帯において34.5MHz帯域幅を使用する高度広帯域伝送による
    /// 衛星デジタル放送として規定する標準方式。
    pub const ADVANCED_BROADBAND_LOW: BroadcastingSystem = BroadcastingSystem(0b001000);
    /// 12.2～12.75GHzの周波数帯において34.5MHz帯域幅を使用する高度広帯域伝送による
    /// 衛星デジタル放送として規定する標準方式。
    pub const ADVANCED_BROADBAND_HIGH: BroadcastingSystem = BroadcastingSystem(0b001001);
    /// 207.5MHz～222MHzの周波数帯の電波を使用するセグメント連結伝送方式による
    /// テレビジョン放送及びマルチメディア放送として規定する標準方式。
    pub const VHF: BroadcastingSystem = BroadcastingSystem(0b001010);
    /// 99MHz～108MHzの周波数の電波を使用するセグメント連結伝送方式による
    /// マルチメディア放送として規定する標準方式。
    pub const V_LOW: BroadcastingSystem = BroadcastingSystem(0b001011);
}

/// システム管理記述子。
#[derive(Debug)]
pub struct SystemManagementDescriptor<'a> {
    /// 放送／非放送種別。
    pub broadcasting_flag: BroadcastingType,
    /// 放送の標準方式種別。
    pub broadcasting_identifier: BroadcastingSystem,
    /// 詳細の識別。
    pub additional_broadcasting_identification: u8,
    /// 付加識別情報。
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
        let broadcasting_identifier = BroadcastingSystem(data[0] & 0b00111111);
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
