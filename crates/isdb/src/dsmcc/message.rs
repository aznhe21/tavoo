//! DIIメッセージおよびDDBメッセージ。

use crate::psi::desc::CompatibilityDescriptor;
use crate::utils::{BytesExt, SliceExt};

use super::desc::DiiDescriptorBlock;

/// DIIメッセージ、DDBメッセージのヘッダ部分。
#[derive(Debug)]
pub struct DsmccAdaptationHeader<'a> {
    /// アダプテーション型。
    pub adaptation_type: u8,
    /// アダプテーションデータ。
    pub adaptation_data: &'a [u8],
}

/// [`DownloadInfoIndication`]における`modules`の要素。
#[derive(Debug)]
pub struct DiiModule<'a> {
    /// moduleId
    pub module_id: u16,
    /// moduleSize
    pub module_size: u32,
    /// moduleVersion
    pub module_version: u8,
    /// moduleInfoByte
    pub module_info: DiiDescriptorBlock<'a>,
}

/// `DownloadInfoIndication`における`dsmccMessageHeader`。
#[derive(Debug)]
pub struct DsmccMessageHeader<'a> {
    /// 0x11の場合このメッセージがMPEG-2 DSM-CCメッセージであることを示す。
    pub protocol_discriminator: u8,
    /// DSM-CC型。
    pub dsmcc_type: u8,
    /// メッセージ型識別。
    pub message_id: u16,
    /// トランザクション識別。
    pub transaction_id: u32,
    /// アダプテーションヘッダ。
    pub dsmcc_adaptation_header: Option<DsmccAdaptationHeader<'a>>,
}

/// DIIメッセージ。
#[derive(Debug)]
pub struct DownloadInfoIndication<'a> {
    /// DSM-CCメッセージヘッダ。
    pub header: DsmccMessageHeader<'a>,
    /// downloadId
    pub download_id: u32,
    /// blockSize
    pub block_size: u16,
    /// windowSize
    pub window_size: u8,
    /// ackPeriod
    pub ack_period: u8,
    /// tCDownloadWindow
    pub t_c_download_window: u32,
    /// tCDownloadScenario
    pub t_c_download_scenario: u32,
    /// compatibilityDescriptor
    pub compatibility_descriptor: Vec<CompatibilityDescriptor<'a>>,
    /// [`DiiModule`]の配列。
    pub modules: Vec<DiiModule<'a>>,
    /// privateDataByte
    pub private_data: &'a [u8],
}

impl<'a> DownloadInfoIndication<'a> {
    /// `data`から`DownloadInfoIndication`を読み取る。
    pub fn read(data: &'a [u8]) -> Option<DownloadInfoIndication<'a>> {
        if data.len() < 12 {
            log::debug!("invalid DownloadInfoIndication");
            return None;
        }

        let protocol_discriminator = data[0];
        let dsmcc_type = data[1];
        let message_id = data[2..=3].read_be_16();
        let transaction_id = data[4..=7].read_be_32();
        let adaptation_length = data[9];
        // let message_length = data[10..=11].read_be_16();
        let dsmcc_adaptation_header = if adaptation_length > 0 {
            let adaptation_type = data[12];
            let Some(adaptation_data) = data[13..].get(..adaptation_length as usize - 1) else {
                log::debug!("invalid DsmccAdaptationHeader::adaptation_data");
                return None;
            };

            Some(DsmccAdaptationHeader {
                adaptation_type,
                adaptation_data,
            })
        } else {
            None
        };

        let data = &data[12 + adaptation_length as usize..];
        if data.len() < 16 {
            log::debug!("invalid DownloadInfoIndication::download_id");
            return None;
        }

        let download_id = data[0..=3].read_be_32();
        let block_size = data[4..=5].read_be_16();
        let window_size = data[6];
        let ack_period = data[7];
        let t_c_download_window = data[8..=11].read_be_32();
        let t_c_download_scenario = data[12..=15].read_be_32();
        let (compatibility_descriptor, data) = CompatibilityDescriptor::read(&data[16..])?;
        let number_of_modules = data[0..=1].read_be_16();
        let mut data = &data[2..];

        let mut modules = Vec::with_capacity(number_of_modules as usize);
        for _ in 0..number_of_modules {
            if data.len() < 8 {
                log::debug!("invalid DownloadInfoIndication::modules");
                return None;
            }

            let module_id = data[0..=1].read_be_16();
            let module_size = data[2..=5].read_be_32();
            let module_version = data[6];
            let module_info_length = data[7];
            let Some((module_info, rem)) = data[8..].split_at_checked(module_info_length as usize)
            else {
                log::debug!("invalid DownloadInfoIndication::module_info");
                return None;
            };
            let module_info = DiiDescriptorBlock::new(module_info);
            data = rem;

            modules.push(DiiModule {
                module_id,
                module_size,
                module_version,
                module_info,
            });
        }
        let private_data = data;

        Some(DownloadInfoIndication {
            header: DsmccMessageHeader {
                protocol_discriminator,
                dsmcc_type,
                message_id,
                transaction_id,
                dsmcc_adaptation_header,
            },
            download_id,
            block_size,
            window_size,
            ack_period,
            t_c_download_window,
            t_c_download_scenario,
            compatibility_descriptor,
            modules,
            private_data,
        })
    }
}

/// `DownloadDataBlock`における`dsmcc_download_data_header`。
#[derive(Debug)]
pub struct DsmccDownloadDataHeader<'a> {
    /// 0x11の場合このメッセージがMPEG-2 DSM-CCメッセージであることを示す。
    pub protocol_discriminator: u8,
    /// DSM-CC型。
    pub dsmcc_type: u8,
    /// メッセージ型識別。
    pub message_id: u16,
    /// ダウンロード識別。
    pub download_id: u32,
    /// アダプテーションヘッダ。
    pub dsmcc_adaptation_header: Option<DsmccAdaptationHeader<'a>>,
}

/// DDBメッセージ。
#[derive(Debug)]
pub struct DownloadDataBlock<'a> {
    /// DSM-CCダウンロードデータヘッダ。
    pub header: DsmccDownloadDataHeader<'a>,
    /// モジュール識別。
    pub module_id: u16,
    /// モジュールバージョン。
    pub module_version: u8,
    /// ブロック番号。
    pub block_number: u16,
    /// ブロックデータ。
    pub block_data: &'a [u8],
}

impl<'a> DownloadDataBlock<'a> {
    /// `data`から`DownloadDataBlock`を読み取る。
    pub fn read(data: &'a [u8]) -> Option<DownloadDataBlock<'a>> {
        if data.len() < 12 {
            log::debug!("invalid DownloadDataBlock");
            return None;
        }

        let protocol_discriminator = data[0];
        let dsmcc_type = data[1];
        let message_id = data[2..=3].read_be_16();
        let download_id = data[4..=7].read_be_32();
        let adaptation_length = data[9];
        // let message_length = data[10..=11].read_be_16();
        let dsmcc_adaptation_header = if adaptation_length > 0 {
            let adaptation_type = data[12];
            let Some(adaptation_data) = data[13..].get(..adaptation_length as usize - 1) else {
                log::debug!("invalid DsmccAdaptationHeader::adaptation_data");
                return None;
            };

            Some(DsmccAdaptationHeader {
                adaptation_type,
                adaptation_data,
            })
        } else {
            None
        };

        let data = &data[12 + adaptation_length as usize..];
        if data.len() < 6 {
            log::debug!("invalid DownloadDataBlock::module_id");
            return None;
        }

        let module_id = data[0..=1].read_be_16();
        let module_version = data[2];
        let block_number = data[4..=5].read_be_16();
        let block_data = &data[6..];

        Some(DownloadDataBlock {
            header: DsmccDownloadDataHeader {
                protocol_discriminator,
                dsmcc_type,
                message_id,
                download_id,
                dsmcc_adaptation_header,
            },
            module_id,
            module_version,
            block_number,
            block_data,
        })
    }
}
