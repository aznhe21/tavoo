//! ARIB STD-B21で規定されるデータモジュール。

use crate::eight::str::AribStr;
use crate::psi::table::{NetworkId, ServiceId, TransportStreamId};
use crate::utils::{BytesExt, SliceExt};

/// コード情報。
#[derive(Debug, PartialEq, Eq)]
pub struct CodeInfo<'a> {
    /// コード。
    pub table_code: u8,
    /// 大項目名記述。
    pub level_1_name: &'a AribStr,
    /// 中項目名記述。
    pub level_2_name: &'a AribStr,
}

/// ジャンルコード表、番組特性コード表。
#[derive(Debug, PartialEq, Eq)]
pub struct CommonTable<'a> {
    /// コード情報を格納する配列。
    pub codes: Vec<CodeInfo<'a>>,
}

impl<'a> CommonTable<'a> {
    /// `data`から`CommonTable`を読み取る。
    pub fn read(data: &'a [u8]) -> Option<CommonTable<'a>> {
        let [number_of_loop, ref data @ ..] = *data else {
            log::debug!("invalid CommonTable");
            return None;
        };

        let mut data = data;
        let mut codes = Vec::with_capacity(number_of_loop as usize);
        for _ in 0..number_of_loop {
            let [table_code, level_1_name_length, ref rem @ ..] = *data else {
                log::debug!("invalid CommonTable::table_code");
                return None;
            };
            let Some((level_1_name, rem)) = rem.split_at_checked(level_1_name_length as usize)
            else {
                log::debug!("invalid CommonTable::level_1_name");
                return None;
            };
            let level_1_name = AribStr::from_bytes(level_1_name);

            let [level_2_name_length, ref rem @ ..] = *rem else {
                log::debug!("invalid CommonTable::level_2_name_length");
                return None;
            };
            let Some((level_2_name, rem)) = rem.split_at_checked(level_2_name_length as usize)
            else {
                log::debug!("invalid CommonTable::level_2_name");
                return None;
            };
            let level_2_name = AribStr::from_bytes(level_2_name);
            data = rem;

            codes.push(CodeInfo {
                table_code,
                level_1_name,
                level_2_name,
            });
        }

        Some(CommonTable { codes })
    }
}

/// 予約語表。
#[derive(Debug, PartialEq, Eq)]
pub struct KeywordTable<'a> {
    /// 予約語名記述。
    pub names: Vec<&'a AribStr>,
}

impl<'a> KeywordTable<'a> {
    /// `data`から`KeywordTable`を読み取る。
    pub fn read(data: &'a [u8]) -> Option<KeywordTable<'a>> {
        let [number_of_loop, ref data @ ..] = *data else {
            log::debug!("invalid KeywordTable");
            return None;
        };

        let mut data = data;
        let mut names = Vec::with_capacity(number_of_loop as usize);
        for _ in 0..number_of_loop {
            let [name_length, ref rem @ ..] = *data else {
                log::debug!("invalid KeywordTable::name_length");
                return None;
            };
            let Some((name, rem)) = rem.split_at_checked(name_length as usize) else {
                log::debug!("invalid KeywordTable::name");
                return None;
            };
            let name = AribStr::from_bytes(name);
            data = rem;

            names.push(name);
        }

        Some(KeywordTable { names })
    }
}

/// ロゴを使用するサービス。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogoService {
    /// オリジナルネットワーク識別。
    pub original_network_id: NetworkId,
    /// トランスポートストリーム識別。
    pub transport_stream_id: TransportStreamId,
    /// サービス識別。
    pub service_id: ServiceId,
}

/// ロゴ情報。
#[derive(Debug, PartialEq, Eq)]
pub struct LogoInfo<'a> {
    /// logo_id
    pub logo_id: u16,
    /// ロゴを使用するサービスを格納する配列。
    pub services: Vec<LogoService>,
    /// data_byte
    pub data: &'a [u8],
}

/// ロゴタイプ。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LogoType(pub u8);

impl LogoType {
    /// HDラージ、64x36。
    pub const HD_LARGE: LogoType = LogoType(0x05);
    /// HDスモール、 48x27。
    pub const HD_SMALL: LogoType = LogoType(0x02);
    /// SD4:3ラージ、72x36。
    pub const SD_4_3_LARGE: LogoType = LogoType(0x03);
    /// SD4:3スモール48x24。
    pub const SD_4_3_SMALL: LogoType = LogoType(0x00);
    /// SD16:9ラージ、54x36。
    pub const SD_16_9_LARGE: LogoType = LogoType(0x04);
    /// SD16:9スモール36x24。
    pub const SD_16_9_SMALL: LogoType = LogoType(0x01);

    /// 定義されているロゴ種別かどうかを返す。
    #[inline]
    pub fn is_known(&self) -> bool {
        self.0 <= 0x05
    }
}

/// ロゴデータ。
#[derive(Debug, PartialEq, Eq)]
pub struct Logo<'a> {
    /// ロゴタイプ。
    pub logo_type: LogoType,
    /// ロゴ情報を格納する配列。
    pub logos: Vec<LogoInfo<'a>>,
}

impl<'a> Logo<'a> {
    /// `data`から`Logo`を読み取る。
    pub fn read(data: &'a [u8]) -> Option<Logo<'a>> {
        if data.len() < 3 {
            log::debug!("invalid Logo");
            return None;
        };

        let logo_type = LogoType(data[0]);
        let number_of_loop = data[1..=2].read_be_16();
        let mut data = &data[3..];

        let mut logos = Vec::with_capacity(number_of_loop as usize);
        for _ in 0..number_of_loop {
            if data.len() < 3 {
                log::debug!("invalid Logo::logo_id");
                return None;
            }

            let logo_id = data[0..=1].read_be_16();
            let number_of_services = data[2];

            let Some((services, rem)) = data[3..].split_at_checked(6 * number_of_services as usize)
            else {
                log::debug!("invalid LogoInfo::services");
                return None;
            };
            let services = services
                .chunks_exact(6)
                .map(|chunk| {
                    let Some(original_network_id) = NetworkId::new(chunk[0..=1].read_be_16())
                    else {
                        log::debug!("invalid LogoInfo::original_network_id");
                        return None;
                    };
                    let Some(transport_stream_id) =
                        TransportStreamId::new(chunk[2..=3].read_be_16())
                    else {
                        log::debug!("invalid LogoInfo::transport_stream_id");
                        return None;
                    };
                    let Some(service_id) = ServiceId::new(chunk[4..=5].read_be_16()) else {
                        log::debug!("invalid LogoInfo::service_id");
                        return None;
                    };

                    Some(LogoService {
                        original_network_id,
                        transport_stream_id,
                        service_id,
                    })
                })
                .collect::<Option<_>>()?;

            if rem.len() < 2 {
                log::debug!("invalid LogoInfo::data_size");
                return None;
            }
            let data_size = rem[0..=1].read_be_16();
            let Some((logo_data, rem)) = rem[2..].split_at_checked(data_size as usize) else {
                log::debug!("invalid LogoInfo::data");
                return None;
            };
            data = rem;

            logos.push(LogoInfo {
                logo_id,
                services,
                data: logo_data,
            });
        }

        Some(Logo { logo_type, logos })
    }
}

/// CDTで配信されるロゴのデータモジュール。
///
/// [`Cdt::data_module`][`crate::psi::table::Cdt::data_module`]を読み取ることが出来る。
#[derive(Debug, PartialEq, Eq)]
pub struct CdtLogo<'a> {
    /// ロゴタイプ。
    pub logo_type: LogoType,
    /// ロゴID（9ビット）。
    pub logo_id: u16,
    /// ロゴバージョン番号（12ビット）。
    pub logo_version: u16,
    /// ロゴデータサイズ。
    pub data: &'a [u8],
}

impl<'a> CdtLogo<'a> {
    /// `data`から`CdtLogo`を読み取る。
    pub fn read(data: &'a [u8]) -> Option<CdtLogo<'a>> {
        if data.len() < 7 {
            log::debug!("invalid CdtLogo");
            return None;
        };

        let logo_type = LogoType(data[0]);
        let logo_id = data[1..=2].read_be_16() & 0b0000_0001_1111_1111;
        let logo_version = data[3..=4].read_be_16() & 0b0000_1111_1111_1111;
        let data_size = data[5..=6].read_be_16();
        let Some(data) = data[7..].get(..data_size as usize) else {
            log::debug!("invalid CdtLogo::data");
            return None;
        };

        Some(CdtLogo {
            logo_type,
            logo_id,
            logo_version,
            data,
        })
    }
}
