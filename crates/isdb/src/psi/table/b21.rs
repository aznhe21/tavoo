//! ARIB STD-B21で規定されるテーブルの定義。

use crate::psi::desc::DescriptorBlock;
use crate::psi::PsiSection;
use crate::time::DateTime;
use crate::utils::{BytesExt, SliceExt};

use super::b10::{RunningStatus, VersionIndicator};

/// Dit（Discontinuity Information Table）。
#[derive(Debug)]
pub struct Dit {
    /// 変化フラグ。
    pub transition_flag: bool,
}

impl Dit {
    /// DITのテーブルID。
    pub const TABLE_ID: u8 = 0x7E;

    /// `psi`から`Dit`を読み取る。
    pub fn read(psi: &PsiSection) -> Option<Dit> {
        if psi.table_id != Self::TABLE_ID {
            log::debug!("invalid Dit::table_id");
            return None;
        }

        let data = psi.data;
        if data.len() != 1 {
            log::debug!("invalid Dit");
            return None;
        }

        let transition_flag = data[0] & 0b10000000 != 0;

        Some(Dit { transition_flag })
    }
}

/// パーシャルトランスポートストリームで伝送されるサービスとイベント。
#[derive(Debug)]
pub struct SitService<'a> {
    /// サービス識別。
    pub service_id: u16,
    /// 進行状態。
    pub running_status: RunningStatus,
    /// 記述子の塊。
    pub descriptors: DescriptorBlock<'a>,
}

/// SIT（Selection Information Table）。
#[derive(Debug)]
pub struct Sit<'a> {
    /// 記述子の塊。
    pub descriptors: DescriptorBlock<'a>,
    /// サービスとイベントを格納する配列。
    pub services: Vec<SitService<'a>>,
}

impl<'a> Sit<'a> {
    /// SITのテーブルID。
    pub const TABLE_ID: u8 = 0x7F;

    /// `psi`から`Sit`を読み取る。
    pub fn read(psi: &PsiSection<'a>) -> Option<Sit<'a>> {
        if psi.table_id != Self::TABLE_ID {
            log::debug!("invalid Sit::table_id");
            return None;
        }
        if psi.syntax.is_none() {
            log::debug!("invalid Sit::syntax");
            return None;
        };

        let data = psi.data;
        if data.len() < 2 {
            log::debug!("invalid Sit");
            return None;
        }

        let Some((descriptors, mut data)) = DescriptorBlock::read(&data[0..]) else {
            log::debug!("invalid Sit::descriptors");
            return None;
        };

        let mut services = Vec::new();
        while !data.is_empty() {
            if data.len() < 4 {
                log::debug!("invalid SitService");
                return None;
            }

            let service_id = data[0..=1].read_be_16();
            let running_status = ((data[2] & 0b01110000) >> 4).into();
            let Some((descriptors, rem)) = DescriptorBlock::read(&data[2..]) else {
                log::debug!("invalid SitService::descriptors");
                return None;
            };
            data = rem;

            services.push(SitService {
                service_id,
                running_status,
                descriptors,
            });
        }

        Some(Sit {
            descriptors,
            services,
        })
    }
}

/// ダウンロード配信の開始時間と継続時間。
#[derive(Debug)]
pub struct SdttSchedule {
    /// 開始時間。
    pub start_time: DateTime,
    /// 継続時間（単位は秒）。
    pub duration: u32,
}

/// ダウンロードコンテンツ。
#[derive(Debug)]
pub struct SdttContent<'a> {
    /// group（4ビット）。
    pub group: u8,
    /// target_version（12ビット）。
    pub target_version: u16,
    /// new_version（12ビット）。
    pub new_version: u16,
    /// download_level（2ビット）。
    pub download_level: u8,
    /// version_indicator。
    pub version_indicator: VersionIndicator,
    /// marker_id_flag
    pub marker_id_flag: bool,
    /// スケジュール時間シフト情報（4ビット）。
    pub schedule_timeshift_information: u8,
    /// 配信の開始時間等を格納する配列。
    pub schedules: Vec<SdttSchedule>,
    /// 記述子の塊。
    pub descriptors: DescriptorBlock<'a>,
}

/// SDTT（Software Download Trigger Table）。
#[derive(Debug)]
pub struct Sdtt<'a> {
    /// 製造者識別。
    pub maker_id: u8,
    /// モデル識別。
    pub model_id: u8,
    /// トランスポートストリーム識別。
    pub transport_stream_id: u16,
    /// オリジナルネットワーク識別。
    pub original_network_id: u16,
    /// サービス識別。
    pub service_id: u16,
    /// ダウンロードコンテンツを格納する配列。
    pub contents: Vec<SdttContent<'a>>,
}

impl<'a> Sdtt<'a> {
    /// SDTTのテーブルID。
    pub const TABLE_ID: u8 = 0xC3;

    /// `psi`から`Sdtt`を読み取る。
    pub fn read(psi: &PsiSection<'a>) -> Option<Sdtt<'a>> {
        if psi.table_id != Self::TABLE_ID {
            log::debug!("invalid Sdtt::table_id");
            return None;
        }
        let Some(syntax) = psi.syntax.as_ref() else {
            log::debug!("invalid Sdtt::syntax");
            return None;
        };

        let data = psi.data;
        if data.len() < 7 {
            log::debug!("invalid Sdtt");
            return None;
        }

        let maker_id = ((syntax.table_id_extension & 0b1111_1111_0000_0000) >> 8) as u8;
        let model_id = (syntax.table_id_extension & 0b0000_0000_1111_1111) as u8;
        let transport_stream_id = data[0..=1].read_be_16();
        let original_network_id = data[2..=3].read_be_16();
        let service_id = data[4..=5].read_be_16();
        let num_of_contents = data[6];

        let mut data = &data[7..];
        let mut contents = Vec::with_capacity(num_of_contents as usize);
        for _ in 0..num_of_contents {
            if data.len() < 8 {
                log::debug!("invalid Sdtt::contents");
                return None;
            }

            let group = (data[0] & 0b11110000) >> 4;
            let target_version = data[0..=1].read_be_16() & 0b0000_1111_1111_1111;
            let new_version = (data[2..=3].read_be_16() & 0b1111_1111_1111_0000) >> 4;
            let download_level = (data[3] & 0b00001100) >> 2;
            let version_indicator = VersionIndicator::new(data[3] & 0b00000011);
            let content_description_length =
                (data[4..=5].read_be_16() & 0b1111_1111_1111_0000) >> 4;
            let marker_id_flag = data[5] & 0b00001000 != 0;
            let schedule_description_length =
                (data[6..=7].read_be_16() & 0b1111_1111_1111_0000) >> 4;
            let schedule_timeshift_information = data[7] & 0b00001111;
            let Some(descriptors_length) = content_description_length
                .checked_sub(schedule_description_length)
            else {
                log::debug!("invalid Sdtt::content_description_length");
                return None;
            };

            let Some((schedule_description, rem)) = data[8..]
                .split_at_checked(schedule_description_length as usize)
            else {
                log::debug!("invalid Sdtt::schedule_description");
                return None;
            };
            let Some((descriptors, rem)) = DescriptorBlock::read_with_len(rem, descriptors_length)
            else {
                log::debug!("invalid Sdtt::descriptors");
                return None;
            };
            let schedules = schedule_description
                .chunks_exact(8)
                .map(|chunk| {
                    let start_time = DateTime::read(chunk[0..=4].try_into().unwrap());
                    let duration = chunk[5..=7].read_bcd_second();

                    SdttSchedule {
                        start_time,
                        duration,
                    }
                })
                .collect();
            data = rem;

            contents.push(SdttContent {
                group,
                target_version,
                new_version,
                download_level,
                version_indicator,
                marker_id_flag,
                schedule_timeshift_information,
                schedules,
                descriptors,
            });
        }

        Some(Sdtt {
            maker_id,
            model_id,
            transport_stream_id,
            original_network_id,
            service_id,
            contents,
        })
    }

    /// BS共通データかどうかを返す。
    #[inline]
    pub fn is_bs_common(&self) -> bool {
        self.maker_id == 0xFF && self.model_id == 0xFE
    }
}

/// [`Cdt`]におけるデータ属性。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CdtDataType(pub u8);

impl CdtDataType {
    /// ロゴデータを表すデータ属性。
    pub const LOGO: CdtDataType = CdtDataType(0x01);
}

/// CDT（Common Data Table）。
#[derive(Debug)]
pub struct Cdt<'a> {
    /// ダウンロードデータ識別。
    pub download_data_id: u16,
    /// オリジナルネットワーク識別。
    pub original_network_id: u16,
    /// データ属性。
    pub data_type: CdtDataType,
    /// 記述子の塊。
    pub descriptors: DescriptorBlock<'a>,
    /// データモジュールバイト。
    pub data_module: &'a [u8],
}

impl<'a> Cdt<'a> {
    /// CDTのテーブルID。
    pub const TABLE_ID: u8 = 0xC8;

    /// `psi`から`Cdt`を読み取る。
    pub fn read(psi: &PsiSection<'a>) -> Option<Cdt<'a>> {
        if psi.table_id != Self::TABLE_ID {
            log::debug!("invalid Cdt::table_id");
            return None;
        }
        let Some(syntax) = psi.syntax.as_ref() else {
            log::debug!("invalid Cdt::syntax");
            return None;
        };

        let data = psi.data;
        if data.len() < 5 {
            log::debug!("invalid Cdt");
            return None;
        }

        let download_data_id = syntax.table_id_extension;
        let original_network_id = data[0..=1].read_be_16();
        let data_type = CdtDataType(data[2]);
        let Some((descriptors, data)) = DescriptorBlock::read(&data[3..]) else {
            log::debug!("invalid Cdt::descriptors");
            return None;
        };
        let data_module = data;

        Some(Cdt {
            download_data_id,
            original_network_id,
            data_type,
            descriptors,
            data_module,
        })
    }
}
