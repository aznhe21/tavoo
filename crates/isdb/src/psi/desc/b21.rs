//! ARIB STD-B21で規定される記述子の定義。

use crate::eight::str::AribStr;
use crate::lang::LangCode;
use crate::utils::{BytesExt, SliceExt};

use super::base::Descriptor;

/// SubDescriptor
#[derive(Debug, PartialEq, Eq)]
pub struct SubDescriptor<'a> {
    /// SubDescriptorType
    pub sub_descriptor_type: u8,
    /// additionalInformation
    pub additional_information: &'a [u8],
}

/// [`DownloadContentDescriptor`]における`compatibility_descriptors`。
#[derive(Debug, PartialEq, Eq)]
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

impl<'a> CompatibilityDescriptor<'a> {
    /// `CompatibilityDescriptor`を読み取る。
    ///
    /// 戻り値は`Vec<CompatibilityDescriptor>`と、それを読み取ったあとの残りのバイト列である。
    pub fn read(data: &'a [u8]) -> Option<(Vec<CompatibilityDescriptor<'a>>, &'a [u8])> {
        if data.len() < 4 {
            log::debug!("invalid CompatibilityDescriptor");
            return None;
        }

        let compatibility_descriptor_length = data[0..=1].read_be_16();
        let descriptor_count = data[2..=3].read_be_16();
        let Some((mut data, tail)) = data[4..]
            .split_at_checked(compatibility_descriptor_length as usize - 2)
        else {
            log::debug!("invalid CompatibilityDescriptor::compatibility_descriptor_length");
            return None;
        };

        let mut descriptors = Vec::with_capacity(descriptor_count as usize);
        for _ in 0..descriptor_count {
            if data.len() < 11 {
                log::debug!("invalid CompatibilityDescriptor::descriptor_type");
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

        Some((descriptors, tail))
    }
}

/// ダウンロードコンテンツ記述子におけるモジュール。
#[derive(Debug, PartialEq, Eq)]
pub struct ModuleInfo<'a> {
    /// モジュール識別。
    pub module_id: u16,
    /// 当該モジュールのバイト長。
    pub module_size: u32,
    /// DIIにて記述される記述子。
    pub module_info: &'a [u8],
}

/// ダウンロードコンテンツ記述子におけるサービス記述。
#[derive(Debug, PartialEq, Eq)]
pub struct ServiceDescription<'a> {
    /// 言語コード。
    pub lang_code: LangCode,
    /// サービス記述。
    pub text: &'a AribStr,
}

/// ダウンロードコンテンツ記述子。
#[derive(Debug, PartialEq, Eq)]
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
            let (descriptors, rem) = CompatibilityDescriptor::read(data)?;
            data = rem;

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
            let lang_code = LangCode(lang_code.try_into().unwrap());

            let [text_length, ref rem @ ..] = *rem else {
                log::debug!("invalid DownloadContentDescriptor::text_length");
                return None;
            };
            let Some((text, _rem)) = rem.split_at_checked(text_length as usize) else {
                log::debug!("invalid DownloadContentDescriptor::text");
                return None;
            };
            let text = AribStr::from_bytes(text);
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
