//! AACで使われるADTSをパースする。

use std::fmt;

use arrayvec::ArrayVec;

use crate::bit::BitReader;

const ID_PCE: u8 = 0x5;

/// サンプリング周波数。
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SamplingFrequency {
    /// 96kHZ
    SF96000,
    /// 48kHZ
    SF48000,
    /// 44.1kHZ
    SF44100,
    /// 32kHZ
    SF32000,
    /// 24kHZ
    SF24000,
    /// 22.05kHZ
    SF22050,
    /// 16kHZ
    SF16000,
}

impl SamplingFrequency {
    /// `sampling_frequency_index`から`SamplingFrequency`を生成する。
    #[inline]
    pub fn new(v: u8) -> Option<SamplingFrequency> {
        match v {
            0 => Some(SamplingFrequency::SF96000),
            3 => Some(SamplingFrequency::SF48000),
            4 => Some(SamplingFrequency::SF44100),
            5 => Some(SamplingFrequency::SF32000),
            6 => Some(SamplingFrequency::SF24000),
            7 => Some(SamplingFrequency::SF22050),
            8 => Some(SamplingFrequency::SF16000),
            _ => None,
        }
    }

    /// ADTSにおける`sampling_frequency_index`の値を得る。
    #[inline]
    pub fn index(self) -> u8 {
        match self {
            SamplingFrequency::SF96000 => 0,
            SamplingFrequency::SF48000 => 3,
            SamplingFrequency::SF44100 => 4,
            SamplingFrequency::SF32000 => 5,
            SamplingFrequency::SF24000 => 6,
            SamplingFrequency::SF22050 => 7,
            SamplingFrequency::SF16000 => 8,
        }
    }

    /// サンプリング周波数を`u32`に変換。
    #[inline]
    pub fn to_u32(self) -> u32 {
        match self {
            SamplingFrequency::SF96000 => 96000,
            SamplingFrequency::SF48000 => 48000,
            SamplingFrequency::SF44100 => 44100,
            SamplingFrequency::SF32000 => 32000,
            SamplingFrequency::SF24000 => 24000,
            SamplingFrequency::SF22050 => 22050,
            SamplingFrequency::SF16000 => 16000,
        }
    }
}

impl fmt::Debug for SamplingFrequency {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            SamplingFrequency::SF96000 => f.write_str("96kHZ"),
            SamplingFrequency::SF48000 => f.write_str("48kHZ"),
            SamplingFrequency::SF44100 => f.write_str("44.1kHZ"),
            SamplingFrequency::SF32000 => f.write_str("32kHZ"),
            SamplingFrequency::SF24000 => f.write_str("24kHZ"),
            SamplingFrequency::SF22050 => f.write_str("22.05kHZ"),
            SamplingFrequency::SF16000 => f.write_str("16KHz"),
        }
    }
}

/// スピーカーに割り当てられる要素。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Address {
    /// 割り当てられるSCEのタグ。
    Sce(u8),
    /// 割り当てられるCPEのタグ。
    Cpe(u8),
}

/// CCEに割り当てられる要素。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CceAddress {
    /// cc_element_is_ind_sw
    pub is_ind_sw: bool,
    /// valid_cc_element_tag_select
    pub tag: u8,
}

/// Program Config Element
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramConfig {
    /// 前方スピーカーに割り当てられる要素。
    pub front_elements: ArrayVec<Address, 15>,
    /// 側方スピーカーに割り当てられる要素。
    pub side_elements: ArrayVec<Address, 15>,
    /// 後方スピーカーに割り当てられる要素。
    pub back_elements: ArrayVec<Address, 15>,
    /// 低域効果チャンネルに割り当てられる要素。
    pub lfe_elements: ArrayVec<u8, 3>,
    /// DSEに割り当てられる要素。
    pub assoc_data_elements: ArrayVec<u8, 7>,
    /// CCEに割り当てられる要素。
    pub cc_elements: ArrayVec<CceAddress, 15>,
}

/// MPEG-2 AACにおけるADTSのフレーム。
///
/// 各パラメータはARIB STD-B32で規定されているものを前提とする。
#[derive(Clone, PartialEq, Eq)]
pub struct Frame {
    /// サンプリング周波数。
    pub sampling_frequency: SamplingFrequency,
    /// MPEG-4 Channel Configuration (`0..=6`)
    pub channel_configuration: u8,
    /// Frame length (`7..=8191`)
    pub aac_frame_length: u16,
    /// CRC check (16 bits)
    pub crc: Option<u16>,
    /// program_config_element
    pub program_config: Option<ProgramConfig>,
}

impl Frame {
    /// ADTSのヘッダを探して返す。
    // https://wiki.multimedia.cx/index.php/ADTS
    pub fn find(mut buf: &[u8]) -> Option<Frame> {
        // Syncwordを探す
        loop {
            let pos = memchr::memchr(0xFF, buf)?;
            buf = &buf[pos..];
            if buf.len() < 2 {
                return None;
            }

            // Syncwordの下位4ビットとLayer
            if buf[1] & 0xF6 == 0xF0 {
                break;
            }
            buf = &buf[1..];
        }

        let mut br = BitReader::new(buf);
        if br.bits() < 56 {
            log::trace!("ADTSの長さが不足");
            return None;
        }
        br.skip(15); // Syncword + MPEG version + Layer

        let protection_absent = br.read1().unwrap();
        let profile = br.read::<2>().unwrap() as u8;
        if profile != 1 {
            log::trace!("profileがLCでない：{}", profile);
            return None;
        }
        let sampling_frequency_index = br.read::<4>().unwrap() as u8;
        let Some(sampling_frequency) = SamplingFrequency::new(sampling_frequency_index) else {
            log::trace!(
                "sampling_frequency_indexが不正：{}",
                sampling_frequency_index
            );
            return None;
        };
        br.skip(1); // Private bit
        let channel_configuration = br.read::<3>().unwrap() as u8;
        if channel_configuration > 6 {
            log::trace!("channel_configurationが不正：{}", channel_configuration);
            return None;
        }
        br.skip(4); // original_copy + home + copyright_id_bit + copyright_id_start
        let aac_frame_length = br.read::<13>().unwrap() as u16;
        if aac_frame_length < 7 {
            log::trace!("aac_frame_lengthが不正：{}", aac_frame_length);
            return None;
        }
        br.skip(11); // adts_buffer_fulness
        let num_blocks = br.read::<2>().unwrap() as u8;
        if num_blocks != 0 {
            log::trace!(
                "number_of_raw_data_blocks_in_frameが'0'でない：{}",
                num_blocks
            );
            return None;
        }

        let crc = if protection_absent {
            None
        } else {
            // 残りちょうど2バイトかもしれないのでread_insideを使う
            Some(br.read_inside::<16>()?)
        };

        let program_config = if br.read::<3>() == Some(ID_PCE as u16) {
            if br.bits() < 34 {
                log::trace!("PCEの長さが不足");
                return None;
            }

            br.skip(6); // element_instance_tag + profile

            let pce_sampling_index = br.read::<4>().unwrap() as u8;
            if sampling_frequency.index() != pce_sampling_index {
                log::trace!(
                    "ADTSのsampling_index（{}）とPCEのsampling_index（{}）が一致しない",
                    sampling_frequency.index(),
                    pce_sampling_index
                );
                return None;
            }
            let num_front = br.read::<4>().unwrap() as u8;
            let num_side = br.read::<4>().unwrap() as u8;
            let num_back = br.read::<4>().unwrap() as u8;
            let num_lfe = br.read::<2>().unwrap() as u8;
            let num_assoc_data = br.read::<3>().unwrap() as u8;
            let num_cc = br.read::<4>().unwrap() as u8;
            if br.read1().unwrap() {
                log::trace!("mono_mixdown_presentが'1'");
                return None;
            }
            if br.read1().unwrap() {
                log::trace!("stereo_mixdown_presentが'1'");
                return None;
            }
            if br.read1().unwrap() {
                br.skip(3); // mixdown_coeff_index + pseudo_surround
            }

            if br.bits()
                < 5 * (num_front + num_side + num_back + num_cc) as usize
                    + 4 * (num_lfe + num_assoc_data + num_cc) as usize
            {
                log::trace!("PCEの長さが不足");
                return None;
            }

            let mut read_addr = |_| {
                let is_cpe = br.read1().unwrap();
                let tag = br.read::<4>().unwrap() as u8;
                if is_cpe {
                    Address::Cpe(tag)
                } else {
                    Address::Sce(tag)
                }
            };
            let front_elements = (0..num_front).map(&mut read_addr).collect();
            let side_elements = (0..num_side).map(&mut read_addr).collect();
            let back_elements = (0..num_back).map(&mut read_addr).collect();

            let mut read_tag_select = |_| br.read::<4>().unwrap() as u8;
            let lfe_elements = (0..num_lfe).map(&mut read_tag_select).collect();
            let assoc_data_elements = (0..num_assoc_data).map(&mut read_tag_select).collect();

            let cc_elements = (0..num_cc)
                .map(|_| {
                    let is_ind_sw = br.read1().unwrap();
                    let tag = br.read::<4>().unwrap() as u8;
                    CceAddress { is_ind_sw, tag }
                })
                .collect();

            Some(ProgramConfig {
                front_elements,
                side_elements,
                back_elements,
                lfe_elements,
                assoc_data_elements,
                cc_elements,
            })
        } else {
            if channel_configuration == 0 {
                log::trace!("channel_configurationが'0'なのにPCEがない");
                return None;
            }

            None
        };

        Some(Frame {
            sampling_frequency,
            channel_configuration,
            aac_frame_length,
            crc,
            program_config,
        })
    }

    /// チャンネル数。
    pub fn num_channels(&self) -> u8 {
        match (self.channel_configuration, &self.program_config) {
            (0, Some(pc)) => {
                // channel_configuration=0ではPCEは必須
                pc.front_elements
                    .iter()
                    .chain(&*pc.side_elements)
                    .chain(&*pc.back_elements)
                    .map(|a| if matches!(a, Address::Cpe(_)) { 2 } else { 1 })
                    .sum::<u8>()
                    + pc.lfe_elements.len() as u8
            }
            (ch @ 1..=6, _) => ch,
            _ => unreachable!(),
        }
    }
}

impl fmt::Debug for Frame {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Header")
            .field("sampling_frequency", &self.sampling_frequency)
            .field("channel_configuration", &self.channel_configuration)
            .field("aac_frame_length", &self.aac_frame_length)
            .field("crc", &self.crc)
            .field("program_config", &self.program_config)
            .finish()
    }
}
