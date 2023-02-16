//! MPEG-2 Systemsや告示（平成26年総務省告示第233号）で規定される記述子および関連する型の定義。

use crate::pid::Pid;
use crate::utils::{BytesExt, SliceExt};

use super::base::Descriptor;

/// サービス形式種別。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ServiceType(pub u8);

impl ServiceType {
    /// デジタルTVサービス
    pub const DIGITAL_TV: ServiceType = ServiceType(0x01);
    /// デジタル音声サービス
    pub const DIGITAL_AUDIO: ServiceType = ServiceType(0x02);
    // 0x03 - 0x7F 未定義
    // 0x80 - 0xA0 事業者定義
    /// 臨時映像サービス
    pub const TEMPORARY_VIDEO: ServiceType = ServiceType(0xA1);
    /// 臨時音声サービス
    pub const TEMPORARY_AUDIO: ServiceType = ServiceType(0xA2);
    /// 臨時データサービス
    pub const TEMPORARY_DATA: ServiceType = ServiceType(0xA3);
    /// エンジニアリングサービス
    pub const ENGINEERING: ServiceType = ServiceType(0xA4);
    /// プロモーション映像サービス
    pub const PROMOTION_VIDEO: ServiceType = ServiceType(0xA5);
    /// プロモーション音声サービス
    pub const PROMOTION_AUDIO: ServiceType = ServiceType(0xA6);
    /// プロモーションデータサービス
    pub const PROMOTION_DATA: ServiceType = ServiceType(0xA7);
    /// 事前蓄積用データサービス
    pub const ACCUMULATION_DATA: ServiceType = ServiceType(0xA8);
    /// 蓄積専用データサービス
    pub const ACCUMULATION_ONLY_DATA: ServiceType = ServiceType(0xA9);
    /// ブックマーク一覧データサービス
    pub const BOOKMARK_LIST_DATA: ServiceType = ServiceType(0xAA);
    /// サーバー型サイマルサービス
    pub const SERVER_TYPE_SIMULTANEOUS: ServiceType = ServiceType(0xAB);
    /// 独立ファイルサービス
    pub const INDEPENDENT_FILE: ServiceType = ServiceType(0xAC);
    /// 超高精細度4K専用TVサービス
    pub const UHD_TV: ServiceType = ServiceType(0xAD);
    // 0xAD - 0xBF 未定義(標準化機関定義領域)
    /// データサービス
    pub const DATA: ServiceType = ServiceType(0xC0);
    /// TLVを用いた蓄積型サービス
    pub const TLV_ACCUMULATION: ServiceType = ServiceType(0xC1);
    /// マルチメディアサービス
    pub const MULTIMEDIA: ServiceType = ServiceType(0xC2);
    // 0xC3 - 0xFF 未定義
    /// 無効
    pub const INVALID: ServiceType = ServiceType(0xFF);

    /// 定義されているサービス種別かどうかを返す。
    #[inline]
    pub fn is_known(&self) -> bool {
        matches!(self.0, 0x01 | 0x02 | 0xA1..=0xAD | 0xC0..=0xC2)
    }
}

/// 偏波。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Polarization {
    /// 水平。
    LinearHorizontal,
    /// 垂直。
    LinearVertical,
    /// 左旋。
    CircularLeft,
    /// 右旋。
    CircularRight,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
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
