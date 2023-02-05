//! ARIB STD-B24で規定されるセクションの定義。

use crate::psi::PsiSection;

use super::message::{DownloadDataBlock, DownloadInfoIndication};

/// DSM-CCセクション。
#[derive(Debug)]
pub enum DsmccSection<'a> {
    /// DIIメッセージ。
    Dii(DownloadInfoIndication<'a>),
    /// DDBメッセージ。
    Ddb(DownloadDataBlock<'a>),
    /// プライベートデータ。
    PrivateData(&'a [u8]),
}

impl<'a> DsmccSection<'a> {
    /// DIIメッセージのテーブルID。
    pub const TABLE_ID_DII: u8 = 0x3B;
    /// DDBメッセージのテーブルID。
    pub const TABLE_ID_DDB: u8 = 0x3C;
    /// プライベートデータのテーブルID。
    pub const TABLE_ID_PRIVATE_DATA: u8 = 0x3E;

    /// `psi`から`DsmccSection`を読み取る。
    pub fn read(psi: &PsiSection<'a>) -> Option<DsmccSection<'a>> {
        match psi.table_id {
            Self::TABLE_ID_DII => Some(DsmccSection::Dii(DownloadInfoIndication::read(psi.data)?)),
            Self::TABLE_ID_DDB => Some(DsmccSection::Ddb(DownloadDataBlock::read(psi.data)?)),
            Self::TABLE_ID_PRIVATE_DATA => Some(DsmccSection::PrivateData(psi.data)),
            _ => {
                log::debug!("invalid DsmccSection");
                None
            }
        }
    }
}
