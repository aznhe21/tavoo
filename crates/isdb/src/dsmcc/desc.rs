//! DIIメッセージのモジュール情報領域ならびにプライベート領域に格納される記述子。

use std::fmt;

use crate::eight::str::AribStr;
use crate::lang::LangCode;
use crate::time::DateTime;
use crate::utils::{BytesExt, SliceExt};

/// DIIメッセージのモジュール情報領域やプライベート領域で使われる記述子を表すトレイト。
pub trait DiiDescriptor<'a>: Sized {
    /// この記述子のタグ。
    const TAG: u8;

    /// `data`から記述子を読み取る。
    ///
    /// `data`には`descriptor_tag`と`descriptor_length`は含まない。
    fn read(data: &'a [u8]) -> Option<Self>;
}

/// パース前の記述子。
pub struct DiiRawDescriptor<'a> {
    /// 記述子のタグ。
    pub tag: u8,

    /// 記述子の内容。
    pub data: &'a [u8],
}

impl<'a> fmt::Debug for DiiRawDescriptor<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("RawDescriptor")
            .field("tag", &crate::utils::UpperHex(self.tag))
            .field("data", &self.data)
            .finish()
    }
}

/// 複数の記述子からなる記述子群。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiiDescriptorBlock<'a>(&'a [u8]);

impl<'a> DiiDescriptorBlock<'a> {
    /// 複数の記述子を含む`data`から`DiiDescriptorBlock`を生成する。
    ///
    /// `block`の中身は`get`で初めてパースされる。
    #[inline]
    pub fn new(block: &'a [u8]) -> DiiDescriptorBlock<'a> {
        DiiDescriptorBlock(block)
    }

    /// 内包する記述子群のイテレーターを返す。
    #[inline]
    pub fn iter(&self) -> DiiDescriptorIter<'a> {
        DiiDescriptorIter(self.0)
    }

    /// 内包する記述子群から`T`のタグと一致する記述子を読み取って返す。
    ///
    /// `T`のタグと一致する記述子がない場合は`None`を返す。
    pub fn get<T: DiiDescriptor<'a>>(&self) -> Option<T> {
        self.iter()
            .find(|d| d.tag == T::TAG)
            .and_then(|d| T::read(d.data))
    }
}

impl<'a> IntoIterator for &DiiDescriptorBlock<'a> {
    type Item = DiiRawDescriptor<'a>;
    type IntoIter = DiiDescriptorIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// [`DiiDescriptorBlock`]のイテレーター。
#[derive(Debug, Clone)]
pub struct DiiDescriptorIter<'a>(&'a [u8]);

impl<'a> Iterator for DiiDescriptorIter<'a> {
    type Item = DiiRawDescriptor<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let [tag, length, ref rem @ ..] = *self.0 else {
            return None;
        };
        let Some((data, tail)) = rem.split_at_checked(length as usize) else {
            return None;
        };

        self.0 = tail;
        Some(DiiRawDescriptor { tag, data })
    }
}

impl<'a> std::iter::FusedIterator for DiiDescriptorIter<'a> {}

/// Type記述子。
#[derive(Debug, PartialEq, Eq)]
pub struct TypeDescriptor<'a> {
    /// メディア型。
    pub text: &'a AribStr,
}

impl<'a> DiiDescriptor<'a> for TypeDescriptor<'a> {
    const TAG: u8 = 0x01;

    fn read(data: &'a [u8]) -> Option<TypeDescriptor<'a>> {
        Some(TypeDescriptor {
            text: AribStr::from_bytes(data),
        })
    }
}

/// Name記述子。
#[derive(Debug, PartialEq, Eq)]
pub struct NameDescriptor<'a> {
    /// モジュールとして伝送するファイルのファイル名。
    pub text: &'a AribStr,
}

impl<'a> DiiDescriptor<'a> for NameDescriptor<'a> {
    const TAG: u8 = 0x02;

    fn read(data: &'a [u8]) -> Option<NameDescriptor<'a>> {
        Some(NameDescriptor {
            text: AribStr::from_bytes(data),
        })
    }
}

/// Info記述子。
#[derive(Debug, PartialEq, Eq)]
pub struct InfoDescriptor<'a> {
    /// 言語コード。
    pub lang_code: LangCode,
    /// モジュールとして伝送するファイルに関する文字列情報。
    pub text: &'a AribStr,
}

impl<'a> DiiDescriptor<'a> for InfoDescriptor<'a> {
    const TAG: u8 = 0x03;

    fn read(data: &'a [u8]) -> Option<InfoDescriptor<'a>> {
        let Some((lang_code, text)) = data.split_at_checked(3) else {
            log::debug!("invalid InfoDescriptor");
            return None;
        };
        let lang_code = LangCode(lang_code.try_into().unwrap());
        let text = AribStr::from_bytes(text);

        Some(InfoDescriptor { lang_code, text })
    }
}

/// Module_Link記述子。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleLinkDescriptor {
    /// リンクされたモジュールの位置関係。
    pub position: u8,
    /// リンク先のモジュールのモジュール識別。
    pub module_id: u16,
}

impl DiiDescriptor<'_> for ModuleLinkDescriptor {
    const TAG: u8 = 0x04;

    fn read(data: &[u8]) -> Option<ModuleLinkDescriptor> {
        if data.len() != 3 {
            log::debug!("invalid ModuleLinkDescriptor");
            return None;
        }

        let position = data[0];
        let module_id = data[1..=2].read_be_16();

        Some(ModuleLinkDescriptor {
            position,
            module_id,
        })
    }
}

/// CRC記述子。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Crc32Descriptor {
    /// CRC値。
    pub crc32: u32,
}

impl DiiDescriptor<'_> for Crc32Descriptor {
    const TAG: u8 = 0x05;

    fn read(data: &[u8]) -> Option<Crc32Descriptor> {
        if data.len() != 4 {
            log::debug!("invalid Crc32Descriptor");
            return None;
        }

        let crc32 = data[0..=3].read_be_32();

        Some(Crc32Descriptor { crc32 })
    }
}

/// ダウンロード推定時間記述子。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EstimatedDownloadTimeDescriptor {
    /// モジュールのダウンロード推定時間（単位は秒）。
    pub est_download_time: u32,
}

impl DiiDescriptor<'_> for EstimatedDownloadTimeDescriptor {
    const TAG: u8 = 0x07;

    fn read(data: &[u8]) -> Option<EstimatedDownloadTimeDescriptor> {
        if data.len() != 4 {
            log::debug!("invalid EstimatedDownloadTimeDescriptor");
            return None;
        }

        let est_download_time = data[0..=3].read_be_32();

        Some(EstimatedDownloadTimeDescriptor { est_download_time })
    }
}

/// ARIB STD-B23で規定されるキャッシュ優先度記述子。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachingPriorityDescriptor {
    /// 優先度。
    pub priority_value: u8,
    /// 透過レベル。
    pub transparency_level: u8,
}

impl DiiDescriptor<'_> for CachingPriorityDescriptor {
    const TAG: u8 = 0x71;

    fn read(data: &[u8]) -> Option<CachingPriorityDescriptor> {
        let [priority_value, transparency_level] = *data else {
            log::debug!("invalid CachingPriorityDescriptor");
            return None;
        };

        Some(CachingPriorityDescriptor {
            priority_value,
            transparency_level,
        })
    }
}

/// Expire記述子。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExpireDescriptor {
    /// 修正ユリウス暦と日本標準時で表される絶対時刻。
    AbsoluteTime(DateTime),
    /// ダウンロード後の経過時間（単位は秒）。
    PassedTime(u32),
    /// 予約。
    Unknown,
}

impl DiiDescriptor<'_> for ExpireDescriptor {
    const TAG: u8 = 0xC0;

    fn read(data: &[u8]) -> Option<ExpireDescriptor> {
        let [time_mode, ref data @ ..] = *data else {
            log::debug!("invalid ExpireDescriptor");
            return None;
        };

        let descriptor = match time_mode {
            0x01 => {
                let Some(mjd_jst_time) = data.get(..5) else {
                    log::debug!("invalid ExpireDescriptor::AbsoluteTime");
                    return None;
                };
                let mjd_jst_time = DateTime::read(mjd_jst_time.try_into().unwrap());

                ExpireDescriptor::AbsoluteTime(mjd_jst_time)
            }
            0x04 => {
                if data.len() < 5 {
                    log::debug!("invalid ExpireDescriptor::PassedTime");
                    return None;
                }

                let passed_seconds = data[1..=4].read_be_32();

                ExpireDescriptor::PassedTime(passed_seconds)
            }
            _ => ExpireDescriptor::Unknown,
        };
        Some(descriptor)
    }
}

/// ActivationTime記述子。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActivationTimeDescriptor {
    /// 修正ユリウス暦と日本標準時で表される絶対時刻。
    PlaybackTime(DateTime),
    /// NPT（33ビット）。
    NPT(u64),
    /// 番組開始時刻からの相対時間指定（単位はミリ秒）。
    EventRelativeTime(u64),
    /// 修正ユリウス暦と日本標準時で表される時刻。
    RecordedTime(DateTime),
    /// 予約。
    Unknown,
}

impl DiiDescriptor<'_> for ActivationTimeDescriptor {
    const TAG: u8 = 0xC1;

    fn read(data: &[u8]) -> Option<ActivationTimeDescriptor> {
        let [time_mode, ref data @ ..] = *data else {
            log::debug!("invalid ActivationTimeDescriptor");
            return None;
        };

        let descriptor = match time_mode {
            0x01 => {
                let Ok(mjd_jst_time) = data.try_into() else {
                    log::debug!("invalid ActivationTimeDescriptor::PlaybackTime");
                    return None;
                };
                let mjd_jst_time = DateTime::read(mjd_jst_time);

                ActivationTimeDescriptor::PlaybackTime(mjd_jst_time)
            }
            0x02 => {
                if data.len() != 5 {
                    log::debug!("invalid ActivationTimeDescriptor::NPT");
                    return None;
                }

                let npt_time =
                    (((data[0] & 0b00000001) as u64) << 32) | (data[1..=4].read_be_32() as u64);

                ActivationTimeDescriptor::NPT(npt_time)
            }
            0x03 => {
                if data.len() != 5 {
                    log::debug!("invalid ActivationTimeDescriptor::EventRelativeTime");
                    return None;
                }

                let event_relative_time =
                    (((data[0] & 0b00001111) as u64) << 32) | (data[1..=4].read_be_32() as u64);

                ActivationTimeDescriptor::EventRelativeTime(event_relative_time)
            }
            0x05 => {
                let Ok(mjd_jst_time) = data.try_into() else {
                    log::debug!("invalid ActivationTimeDescriptor::RecordedTime");
                    return None;
                };
                let mjd_jst_time = DateTime::read(mjd_jst_time);

                ActivationTimeDescriptor::RecordedTime(mjd_jst_time)
            }
            _ => ActivationTimeDescriptor::Unknown,
        };
        Some(descriptor)
    }
}

/// CompressionType記述子。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompressionTypeDescriptor {
    /// 圧縮形式。
    pub compression_type: u8,
    /// 圧縮前のモジュールのサイズ（単位はバイト）。
    pub original_size: u32,
}

impl DiiDescriptor<'_> for CompressionTypeDescriptor {
    const TAG: u8 = 0xC2;

    fn read(data: &[u8]) -> Option<CompressionTypeDescriptor> {
        if data.len() != 5 {
            log::debug!("invalid CompressionTypeDescriptor");
            return None;
        }

        let compression_type = data[0];
        let original_size = data[1..=4].read_be_32();

        Some(CompressionTypeDescriptor {
            compression_type,
            original_size,
        })
    }
}

/// Control記述子。
#[derive(Debug, PartialEq, Eq)]
pub struct ControlDescriptor<'a> {
    /// モジュールの解釈・制御に必要な情報。
    pub control_data: &'a [u8],
}

impl<'a> DiiDescriptor<'a> for ControlDescriptor<'a> {
    const TAG: u8 = 0xC3;

    fn read(data: &'a [u8]) -> Option<ControlDescriptor<'a>> {
        Some(ControlDescriptor { control_data: data })
    }
}

/// ProviderPrivate記述子。
#[derive(Debug, PartialEq, Eq)]
pub struct ProviderPrivateDescriptor<'a> {
    /// この記述子のスコープを示す識別子の種類。
    pub private_scope_type: u8,
    /// スコープ種別ごとのスコープ識別値。
    pub scope_identifier: u32,
    /// 補助情報。
    pub private: &'a [u8],
}

impl<'a> DiiDescriptor<'a> for ProviderPrivateDescriptor<'a> {
    const TAG: u8 = 0xC4;

    fn read(data: &'a [u8]) -> Option<ProviderPrivateDescriptor<'a>> {
        if data.len() < 5 {
            log::debug!("invalid ProviderPrivateDescriptor");
            return None;
        }

        let private_scope_type = data[0];
        let scope_identifier = data[1..=4].read_be_32();
        let private = &data[5..];

        Some(ProviderPrivateDescriptor {
            private_scope_type,
            scope_identifier,
            private,
        })
    }
}

/// StoreRoot記述子。
#[derive(Debug, PartialEq, Eq)]
pub struct StoreRootDescriptor<'a> {
    /// `store_root_path`で指定されるディレクトリの内容を消去するか否か。
    pub update_type: bool,
    /// 当該カルーセルに含まれるモジュール群が蓄積される最上位のディレクトリ。
    pub store_root_path: &'a AribStr,
}

impl<'a> DiiDescriptor<'a> for StoreRootDescriptor<'a> {
    const TAG: u8 = 0xC5;

    fn read(data: &'a [u8]) -> Option<StoreRootDescriptor<'a>> {
        if data.len() < 1 {
            log::debug!("invalid StoreRootDescriptor");
            return None;
        }

        let update_type = data[0] & 0b10000000 != 0;
        let store_root_path = AribStr::from_bytes(&data[1..]);

        Some(StoreRootDescriptor {
            update_type,
            store_root_path,
        })
    }
}

/// Subdirectory記述子。
#[derive(Debug, PartialEq, Eq)]
pub struct SubdirectoryDescriptor<'a> {
    /// 当該カルーセルに含まれるモジュールが蓄積されるサブディレクトリ。
    pub subdirectory_path: &'a AribStr,
}

impl<'a> DiiDescriptor<'a> for SubdirectoryDescriptor<'a> {
    const TAG: u8 = 0xC6;

    fn read(data: &'a [u8]) -> Option<SubdirectoryDescriptor<'a>> {
        Some(SubdirectoryDescriptor {
            subdirectory_path: AribStr::from_bytes(data),
        })
    }
}

/// Title記述子。
#[derive(Debug, PartialEq, Eq)]
pub struct TitleDescriptor<'a> {
    /// 言語コード。
    pub lang_code: LangCode,
    /// コンテンツ全体またはモジュールに関して視聴者に提示する名前。
    pub text: &'a AribStr,
}

impl<'a> DiiDescriptor<'a> for TitleDescriptor<'a> {
    const TAG: u8 = 0xC7;

    fn read(data: &'a [u8]) -> Option<TitleDescriptor<'a>> {
        let Some((lang_code, text)) = data.split_at_checked(3) else {
            log::debug!("invalid TitleDescriptor");
            return None;
        };
        let lang_code = LangCode(lang_code.try_into().unwrap());
        let text = AribStr::from_bytes(text);

        Some(TitleDescriptor { lang_code, text })
    }
}

/// DataEncoding記述子。
#[derive(Debug, PartialEq, Eq)]
pub struct DataEncodingDescriptor<'a> {
    /// データ符号化方式識別。
    pub data_component_id: u16,
    /// 付加識別情報。
    pub additional_data_encoding_info: &'a [u8],
}

impl<'a> DiiDescriptor<'a> for DataEncodingDescriptor<'a> {
    const TAG: u8 = 0xC8;

    fn read(data: &'a [u8]) -> Option<DataEncodingDescriptor<'a>> {
        if data.len() < 2 {
            log::debug!("invalid DataEncodingDescriptor");
            return None;
        }

        let data_component_id = data[0..=1].read_be_16();
        let additional_data_encoding_info = &data[2..];

        Some(DataEncodingDescriptor {
            data_component_id,
            additional_data_encoding_info,
        })
    }
}

/// ルート証明書記述子における汎用証明書。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenericRootCertificate {
    /// ルート証明書の有無とその識別情報。
    pub root_certificate_id: u32,
    /// `root_certificate_id`で識別される単位でのルート証明書のバージョン。
    pub root_certificate_version: u32,
}

/// ルート証明書記述子。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RootCertificateDescriptor {
    /// 汎用証明書。
    Generic(Vec<GenericRootCertificate>),
    /// 事業者専用証明書。
    BroadcasterSpecificCertificate(Vec<u64>),
}

impl DiiDescriptor<'_> for RootCertificateDescriptor {
    const TAG: u8 = 0xCA;

    fn read(data: &[u8]) -> Option<RootCertificateDescriptor> {
        if data.len() < 1 {
            log::debug!("invalid RootCertificateDescriptor");
            return None;
        }

        let root_certificate_type = (data[0] & 0b10000000) >> 7;
        let data = &data[1..];

        let descriptor = match root_certificate_type {
            0 => {
                let certificates = data
                    .chunks_exact(8)
                    .map(|chunk| {
                        let root_certificate_id = chunk[0..=3].read_be_32();
                        let root_certificate_version = chunk[4..=7].read_be_32();

                        GenericRootCertificate {
                            root_certificate_id,
                            root_certificate_version,
                        }
                    })
                    .collect();

                RootCertificateDescriptor::Generic(certificates)
            }
            1 => {
                let certificates = data.chunks_exact(8).map(<[u8]>::read_be_64).collect();

                RootCertificateDescriptor::BroadcasterSpecificCertificate(certificates)
            }
            _ => unreachable!(),
        };

        Some(descriptor)
    }
}
