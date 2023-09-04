//! ARIB STD-B25で規定される記述子の定義。

use crate::eight::str::AribStr;
use crate::utils::{BytesExt, SliceExt};

use super::super::table::{NetworkId, TransportStreamId};
use super::base::Descriptor;

/// CA_EMM_TS記述子。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaEmmTsDescriptor {
    /// 限定受信方式識別。
    pub ca_system_id: u16,
    /// トランスポートストリーム識別。
    pub transport_stream_id: TransportStreamId,
    /// オリジナルネットワーク識別。
    pub original_network_id: NetworkId,
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
        let Some(transport_stream_id) = TransportStreamId::new(data[2..=3].read_be_16()) else {
            log::debug!("invalid CaEmmTsDescriptor::transport_stream_id");
            return None;
        };
        let Some(original_network_id) = NetworkId::new(data[4..=5].read_be_16()) else {
            log::debug!("invalid CaEmmTsDescriptor::original_network_id");
            return None;
        };
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
#[derive(Debug, PartialEq, Eq)]
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
    pub fee_name: &'a AribStr,
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
        let Some((contract_verification_info, data)) =
            data.split_at_checked(contract_verification_info_length as usize)
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
        let fee_name = AribStr::from_bytes(fee_name);

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
#[derive(Debug, Clone, PartialEq, Eq)]
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
