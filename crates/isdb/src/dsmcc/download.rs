//! DSM-CCによるダウンロードの支援。

use super::message::{DiiModule, DownloadDataBlock, DownloadInfoIndication};

/// [`DownloadInfoIndication`]と[`DownloadDataBlock`]を組み合わせてデータをダウンロードする。
///
/// `DownloadData`は`DiiModule::module_id`ごとに管理する。
pub struct DownloadData {
    download_id: u32,
    module_version: u8,
    block_size: u16,
    data: Vec<u8>,
    downloaded: u16,
}

impl DownloadData {
    /// `module`のダウンロードを管理するための`DownloadData`を生成する。
    ///
    /// `info`には`module`の親である`DownloadInfoIndication`を指定する。
    pub fn new(info: &DownloadInfoIndication, module: &DiiModule) -> DownloadData {
        DownloadData {
            download_id: info.download_id,
            module_version: module.module_version,
            block_size: info.block_size,
            data: vec![0; module.module_size as usize],
            downloaded: 0,
        }
    }

    /// この`DownloadData`のダウンロード識別を返す。
    #[inline]
    pub fn download_id(&self) -> u32 {
        self.download_id
    }

    /// 新しくダウンロードを開始する必要があるかどうかを返す。
    ///
    /// このメソッドが`true`を返した場合、`DownloadData::new`を呼び出して
    /// 新しくデータのダウンロードを開始すべきである。
    pub fn needs_restart(&self, info: &DownloadInfoIndication, module: &DiiModule) -> bool {
        self.download_id != info.download_id
            || self.block_size != info.block_size
            || self.data.len() as u32 != module.module_size
            || self.module_version != module.module_version
    }

    #[inline]
    fn n_blocks(&self) -> u16 {
        ((self.data.len() as u32 - 1) / (self.block_size as u32) + 1) as u16
    }

    /// 全ブロックのダウンロードが完了していればそのデータを返す。
    #[inline]
    pub fn completed(&self) -> Option<&[u8]> {
        (self.downloaded == self.n_blocks()).then_some(&*self.data)
    }

    /// `block`をダウンロード完了データに加える。
    ///
    /// 全ブロックのダウンロードが完了した場合、そのデータを返す。
    pub fn store(&mut self, block: &DownloadDataBlock) -> Option<&[u8]> {
        if block.block_number >= self.n_blocks()
            || self.download_id != block.header.download_id
            || self.module_version != block.module_version
        {
            return None;
        }

        let offset = block.block_number as usize * self.block_size as usize;
        let size = if block.block_number < self.n_blocks() - 1 {
            self.block_size as usize
        } else {
            self.data.len() - offset
        };
        if block.block_data.len() < size {
            log::debug!("DownloadData: download block is too small");
            return None;
        }

        self.data[offset..offset + size].copy_from_slice(&block.block_data[..size]);
        self.downloaded += 1;

        self.completed()
    }
}
