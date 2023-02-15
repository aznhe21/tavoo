//! MPEG2-TSのパケット。

use std::fmt;
use std::io::{self, Read};
use std::time::Duration;

use crate::pid::Pid;
use crate::utils::BytesExt;

const SYNC_BYTE: u8 = 0x47;
const PACKET_SIZE: usize = 188;

/// MPEG2-TSのパケット。
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Packet(pub [u8; PACKET_SIZE]);

impl Packet {
    /// `r`からTSパケットを順次読み込むイテレーターを生成する。
    #[inline]
    #[must_use]
    pub fn iter<R: Read>(r: R) -> PacketIter<R> {
        PacketIter { r }
    }

    /// `r`からTSパケットを読み込む。
    ///
    /// 原則として188バイトずつ読み込むが、パケットとして正しくなさそうな部分は読み飛ばす。
    // LibISDBのTSPacketParserFilter::SyncPacketと同じ動作をする。
    pub fn read<R: Read>(r: R) -> io::Result<Option<Packet>> {
        fn read_inner<R: Read>(mut r: R) -> io::Result<Packet> {
            let mut packet = Packet([0; PACKET_SIZE]);
            r.read_exact(&mut packet.0)?;
            if packet.0[0] == SYNC_BYTE {
                return Ok(packet);
            }

            let mut may_resync = false;
            // 同期バイト待ち
            let pos = loop {
                if let Some(pos) = memchr::memchr(SYNC_BYTE, &packet.0) {
                    // Safety: memchrの戻り値が入力の長さを超えることはない
                    unsafe { crate::utils::assume!(pos < PACKET_SIZE) }

                    // 同期バイト発見
                    break pos;
                }

                r.read_exact(&mut packet.0)?;
                may_resync = true;
            };

            packet.0.copy_within(pos.., 0);
            r.read_exact(&mut packet.0[PACKET_SIZE - pos..])?;

            if may_resync || pos > 16 {
                while !packet.is_normal() {
                    let Some(pos) = memchr::memchr(SYNC_BYTE, &packet.0[1..]) else {
                        break;
                    };
                    // 同期バイトが他にもある場合、そこから再同期する

                    let pos = pos + 1;
                    // Safety: memchrの戻り値が入力の長さを超えることはない
                    unsafe { crate::utils::assume!((1..PACKET_SIZE).contains(&pos)) }

                    packet.0.copy_within(pos.., 0);
                    r.read_exact(&mut packet.0[PACKET_SIZE - pos..])?;
                }
            }
            Ok(packet)
        }

        match read_inner(r) {
            Ok(packet) => Ok(Some(packet)),
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// パケットが正常かどうかを返す。
    pub fn is_normal(&self) -> bool {
        if self.sync_byte() != SYNC_BYTE {
            // 同期バイト不正
            return false;
        }
        if self.error_indicator() {
            // ビット誤りあり
            return false;
        }
        if (0x0002..=0x000F).contains(&self.pid().get()) {
            // 未定義PID範囲
            return false;
        }
        if self.scrambling_control() == 0x01 {
            // 未定義スクランブル制御値
            return false;
        }
        if self.adaptation_field_control() == 0b00 {
            // 未定義アダプテーションフィールド制御値
            return false;
        }
        if self.adaptation_field_control() == 0b10 && self.adaptation_field_length_raw() > 183 {
            // アダプテーションフィールド長異常
            return false;
        }
        if self.adaptation_field_control() == 0b11 && self.adaptation_field_length_raw() > 182 {
            // アダプテーションフィールド長異常
            return false;
        }

        true
    }

    /// sync byteを返す。
    #[inline]
    pub fn sync_byte(&self) -> u8 {
        self.0[0]
    }

    /// transport error indicatorを返す。
    #[inline]
    pub fn error_indicator(&self) -> bool {
        self.0[1] & 0b10000000 != 0
    }

    /// payload unit start indicatorを返す。
    #[inline]
    pub fn unit_start_indicator(&self) -> bool {
        self.0[1] & 0b01000000 != 0
    }

    /// transport priorityを返す。
    #[inline]
    pub fn priority(&self) -> bool {
        self.0[1] & 0b00100000 != 0
    }

    /// PID（13ビット）を返す。
    #[inline]
    pub fn pid(&self) -> Pid {
        Pid::read(&self.0[1..])
    }

    /// transport scrambling control（2ビット）を返す。
    #[inline]
    pub fn scrambling_control(&self) -> u8 {
        (self.0[3] & 0b11000000) >> 6
    }

    /// パケットがスクランブル処理されているかを返す。
    #[inline]
    pub fn is_scrambled(&self) -> bool {
        self.scrambling_control() & 0b10 != 0
    }

    /// adaptation field control（2ビット）を返す。
    #[inline]
    pub fn adaptation_field_control(&self) -> u8 {
        (self.0[3] & 0b00110000) >> 4
    }

    /// continuity counter（4ビット）を返す。
    #[inline]
    pub fn continuity_counter(&self) -> u8 {
        self.0[3] & 0b00001111
    }

    /// パケットがAdaptation Fieldを含むかどうかを返す。
    #[inline]
    pub fn has_adaptation_field(&self) -> bool {
        self.adaptation_field_control() & 0b10 != 0
    }

    #[inline]
    fn adaptation_field_length_raw(&self) -> u8 {
        self.0[4]
    }

    /// adaptation fieldがある場合、adaptation_field_lengthを返す。
    pub fn adaptation_field_length(&self) -> Option<u8> {
        self.has_adaptation_field()
            .then(|| self.adaptation_field_length_raw())
    }

    /// adaptation fieldを返す。
    pub fn adaptation_field(&self) -> Option<AdaptationField> {
        self.adaptation_field_length()
            .and_then(|length| self.0.get(4..4 + 1 + length as usize))
            .and_then(AdaptationField::parse)
    }

    /// パケットがペイロードを含むかどうかを返す。
    #[inline]
    pub fn has_payload(&self) -> bool {
        self.adaptation_field_control() & 0b01 != 0
    }

    /// ペイロードを返す。
    pub fn payload(&self) -> Option<&[u8]> {
        if !self.has_payload() {
            None
        } else if let Some(afl) = self.adaptation_field_length() {
            let offset = 4 + 1 + afl as usize;
            self.0.get(offset..)
        } else {
            self.0.get(4..)
        }
    }

    /// `payload`がPESかどうかを返す。
    #[inline]
    pub fn payload_is_pes(payload: &[u8]) -> bool {
        payload.starts_with(&[0x00, 0x00, 0x01])
    }
}

impl fmt::Debug for Packet {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Packet")
            .field("sync_byte", &self.sync_byte())
            .field("error_indicator", &self.error_indicator())
            .field("unit_start_indicator", &self.unit_start_indicator())
            .field("priority", &self.priority())
            .field("pid", &self.pid())
            .field("scrambling_control", &self.scrambling_control())
            .field("adaptation_field_control", &self.adaptation_field_control())
            .field("continuity_counter", &self.continuity_counter())
            .finish_non_exhaustive()
    }
}

/// Program Clock Reference
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pcr {
    /// PCRにおける90kHzの部分。
    pub base: u64,

    /// PCRにおける27MHzの部分。
    pub extension: u16,
}

impl Pcr {
    /// 90kHzの`base`と27MHzの`extension`から`Pcr`を生成する。
    #[inline]
    pub fn new(base: u64, extension: u16) -> Pcr {
        Pcr { base, extension }
    }

    /// `base`と`extension`から27MHzのPCRを取得する。
    #[inline]
    pub fn full(&self) -> u64 {
        self.base * 300 + self.extension as u64
    }

    /// PCRを秒に変換する。
    #[inline]
    pub fn to_secs(&self) -> u64 {
        self.full() / 27_000_000
    }

    /// PCRを秒成分を含むナノ秒に変換する。
    #[inline]
    pub fn to_nanos(&self) -> u64 {
        self.full() * 1_000 / 27
    }

    /// PCRを[`Duration`]に変換する。
    pub fn to_duration(&self) -> Duration {
        let pcr = self.full();
        let secs = pcr / 27_000_000;
        let nanos = (pcr % 27_000_000 * 1000 / 27) as u32;
        Duration::new(secs, nanos)
    }
}

/// TSパケットのadaptation field。
#[derive(Debug, Clone)]
pub struct AdaptationField<'a> {
    /// discontinuity indicator
    pub discontinuity_indicator: bool,

    /// random access indicator
    pub random_access_indicator: bool,

    /// elementary stream priority indicator
    pub es_priority_indicator: bool,

    /// program clock reference
    pub pcr: Option<Pcr>,

    /// original program clock reference
    pub pcr_original: Option<Pcr>,

    /// splice countdown
    pub splice_countdown: Option<u8>,

    /// transport private data
    pub private_data: Option<&'a [u8]>,

    /// adaptation extension
    pub extension: Option<AdaptationExtension>,
}

impl<'a> AdaptationField<'a> {
    fn read_pcr(data: &[u8; 6]) -> Pcr {
        let base =
            ((data[0..=3].read_be_32() as u64) << 1) | (((data[4] & 0b10000000) >> 7) as u64);
        let extension = data[4..=5].read_be_16() & 0b0000_0001_1111_1111;
        Pcr { base, extension }
    }

    /// TSパケットのadaptation fieldをパースして[`AdaptationField`]として返す。
    ///
    /// `af`の長さが不足している場合は`None`を返す。
    pub fn parse(mut af: &[u8]) -> Option<AdaptationField> {
        if af.len() < 2 {
            return None;
        }

        // af[0]はadaptation_field_length
        let discontinuity_indicator = (af[1] & 0b10000000) != 0;
        let random_access_indicator = (af[1] & 0b01000000) != 0;
        let es_priority_indicator = (af[1] & 0b00100000) != 0;
        let pcr_flag = (af[1] & 0b00010000) != 0;
        let original_pcr_flag = (af[1] & 0b00001000) != 0;
        let splicing_point_flag = (af[1] & 0b00000100) != 0;
        let private_data_flag = (af[1] & 0b00000010) != 0;
        let extension_flag = (af[1] & 0b00000001) != 0;

        af = &af[2..];
        let pcr = if pcr_flag && af.len() >= 6 {
            let v = AdaptationField::read_pcr(&af[..6].try_into().unwrap());
            af = &af[6..];
            Some(v)
        } else {
            None
        };
        let pcr_original = if original_pcr_flag && af.len() >= 6 {
            let v = AdaptationField::read_pcr(&af[..6].try_into().unwrap());
            af = &af[6..];
            Some(v)
        } else {
            None
        };
        let splice_countdown = if splicing_point_flag && af.len() >= 1 {
            let v = af[0];
            af = &af[1..];
            Some(v)
        } else {
            None
        };
        let private_data = if private_data_flag && af.len() >= 1 && af.len() >= 1 + af[0] as usize {
            let len = af[0] as usize;
            let v = &af[1..1 + len];
            af = &af[1 + len..];
            Some(v)
        } else {
            None
        };
        let valid_extension =
            extension_flag && af.len() >= 2 && af[0] >= 1 && af.len() >= 1 + af[0] as usize;
        let extension = if valid_extension {
            let len = af[0] as usize;
            let ltw_flag = af[1] & 0b10000000 != 0;
            let piecewise_rate_flag = af[1] & 0b01000000 != 0;
            let seamless_splice_flag = af[1] & 0b00100000 != 0;

            let mut v = &af[2..1 + len];
            let ltw = if ltw_flag && v.len() >= 2 {
                let ltw_valid_flag = v[0] & 0b10000000 != 0;
                let ltw_offset = v[0..=1].read_be_16() & 0b0111_1111_1111_1111;
                v = &v[2..];

                Some(LegalTimeWindow {
                    ltw_valid_flag,
                    ltw_offset,
                })
            } else {
                None
            };
            let piecewise = if piecewise_rate_flag && v.len() >= 3 {
                let piecewise_rate =
                    ((v[0] & 0b00111111) as u32) << 16 | (v[1..=2].read_be_16() as u32);
                v = &v[3..];

                Some(Piecewise { piecewise_rate })
            } else {
                None
            };
            let seamless_splice = if seamless_splice_flag && v.len() >= 5 {
                let splice_type = (v[0] & 0b11110000) >> 4;
                let dts_next_access_unit = (((v[0] & 0b00001110) as u64) << 29)
                    | (((v[1] & 0b11111111) as u64) << 22)
                    | (((v[2] & 0b11111110) as u64) << 14)
                    | (((v[3] & 0b11111111) as u64) << 7)
                    | (((v[4] & 0b11111110) as u64) >> 1);

                Some(SeamlessSplice {
                    splice_type,
                    dts_next_access_unit,
                })
            } else {
                None
            };

            // af = &af[1 + len..];
            Some(AdaptationExtension {
                ltw,
                piecewise,
                seamless_splice,
            })
        } else {
            None
        };

        Some(AdaptationField {
            discontinuity_indicator,
            random_access_indicator,
            es_priority_indicator,
            pcr,
            pcr_original,
            splice_countdown,
            private_data,
            extension,
        })
    }
}

/// adaptation extension
#[derive(Debug, Clone)]
pub struct AdaptationExtension {
    /// legal time window
    pub ltw: Option<LegalTimeWindow>,

    /// piecewise rate
    pub piecewise: Option<Piecewise>,

    /// seamless splice
    pub seamless_splice: Option<SeamlessSplice>,
}

/// legal time window
#[derive(Debug, Clone)]
pub struct LegalTimeWindow {
    /// LTW valid flag
    pub ltw_valid_flag: bool,

    /// LTW offset
    pub ltw_offset: u16,
}

/// piecewise
#[derive(Debug, Clone)]
pub struct Piecewise {
    /// piecewise rate
    pub piecewise_rate: u32,
}

/// seamless splice
#[derive(Debug, Clone)]
pub struct SeamlessSplice {
    /// splice type
    pub splice_type: u8,

    /// DTS next access unit
    pub dts_next_access_unit: u64,
}

/// TSパケットを順次読み込むイテレーター。
#[derive(Debug)]
pub struct PacketIter<R> {
    r: R,
}

impl<R: Read> Iterator for PacketIter<R> {
    type Item = io::Result<Packet>;

    fn next(&mut self) -> Option<Self::Item> {
        Packet::read(&mut self.r).transpose()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_matches::assert_matches;

    // https://youzaka.hatenablog.com/entry/2011/11/09/001615
    const PACKET_1: Packet = Packet(hex_literal::hex!(
        "
47 40 12 18 00 4F F0 CC 01 26 FF 01 01 43 11 00
04 01 4F 44 4D DA 15 17 25 00 00 05 00 10 B1 4D
78 6A 70 6E 10 AA A6 C1 CE 3F 40 4D 4D 1B 24 2A
3B 1B 7D FA D6 63 30 61 42 58 A8 CE 35 28 40 61
21 22 32 46 49 7E F2 3C 7D 47 3C B9 EB 41 30 CB
E4 EB B3 C8 C8 CF 1B 7E BF 1B 7D E4 E9 BA CB 3C
7D 47 3C B9 EB C8 33 32 43 6E AC 49 7E F2 39 53
E9 B7 C6 B7 DE A6 B3 C8 E2 21 26 21 26 21 26 40
35 B7 A4 32 46 49 7E 3C 7D 47 3C 4A 7D 4B 21 F2
3E 52 32 70 B7 DE B9 21 23 50 06 F1 03 00 6A 70
6E 54 06 22 FF 2F FF 84 FF C1 02 A4 01 C4 11 F2
03 10 0F FF 6F 6A 70 6E 25 39 25 46
"
    ));
    const PACKET_2: Packet = Packet(hex_literal::hex!(
        "
47 01 40 37 3F 00 FF FF FF FF FF FF FF FF FF FF
FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF
FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF
FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF
FF FF FF FF 34 66 D8 08 6A E7 24 A1 28 F9 07 89
57 01 65 9A 48 3B 9E AC 90 24 AB C6 0F 93 94 58
DD 91 F2 4E 4A 1C F7 01 16 B2 CA 26 36 E5 A4 9A
30 24 0C 38 EC 78 55 6C 80 F1 A1 E0 72 14 41 32
D9 82 A9 48 2C A4 16 53 1F 53 03 3A 84 8C 1B FF
91 8D F7 54 C1 D4 C7 CE 72 A6 AA 45 EA 62 6A 61
65 75 20 F2 B9 48 1C A6 46 52 1B 39 C9 A4 F0 C1
A8 19 92 72 D6 38 D8 00 00 00 00 00
"
    ));
    const PACKET_3: Packet = Packet(hex_literal::hex!(
        "
47 01 11 20 B7 10 D2 2D 74 82 80 F9 FF FF FF FF
FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF
FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF
FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF
FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF
FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF
FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF
FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF
FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF
FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF
FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF FF
FF FF FF FF FF FF FF FF FF FF FF FF
    "
    ));

    #[test]
    fn test_packet_read() {
        for packet in [PACKET_1, PACKET_2, PACKET_3] {
            let pkt: &[u8] = &packet.0;

            assert_eq!(Packet::read(&mut &pkt[..0]).unwrap(), None);
            assert_eq!(Packet::read(&mut &pkt[1..]).unwrap(), None);
            assert_eq!(Packet::read(&mut &*pkt).unwrap(), Some(packet.clone()));
            assert_eq!(
                Packet::read(&mut &*[&[0; 1], pkt].concat()).unwrap(),
                Some(packet.clone()),
            );
            assert_eq!(
                Packet::read(&mut &*[&[0; 1], pkt].concat()).unwrap(),
                Some(packet.clone()),
            );
            assert_eq!(
                Packet::read(&mut &*[&[0_u8; 17] as &[u8], &[SYNC_BYTE, 1], pkt].concat()).unwrap(),
                Some(packet.clone()),
            );
        }
    }

    #[test]
    fn test_pcr() {
        let pcr = Pcr::new(7052388613, 249);
        assert_eq!(pcr.full(), 2115716584149);
        assert_eq!(pcr.to_duration(), Duration::new(78359, 873487 * 1000));
    }

    #[test]
    fn test_packet_read_err() {
        struct ReadErr(io::ErrorKind);
        impl Read for ReadErr {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                Err(self.0.into())
            }
        }

        assert_matches!(
            Packet::read(ReadErr(io::ErrorKind::UnexpectedEof)),
            Ok(None)
        );
        assert_matches!(
            Packet::read(ReadErr(io::ErrorKind::BrokenPipe)),
            Err(e) if e.kind() == io::ErrorKind::BrokenPipe
        );
    }

    #[test]
    fn test_packet_abnormal() {
        macro_rules! packet {
            ($($part:expr),*$(,)?) => {{
                Packet([
                    $(
                        std::convert::identity::<&[u8]>(&$part),
                    )*
                ].concat().try_into().unwrap())
            }};
        }

        let packet = packet!([0x00], [0; 187]);
        assert_eq!(packet.sync_byte(), 0x00);
        assert!(!packet.is_normal());
        let packet = packet!([0xFF], [0; 187]);
        assert_eq!(packet.sync_byte(), 0xFF);
        assert!(!packet.is_normal());

        let packet = packet!([SYNC_BYTE, 0b10000000], [0; 186]);
        assert!(packet.error_indicator());
        assert!(!packet.is_normal());

        for pid in 0x0002..=0x000F {
            let [hi, lo] = u16::to_be_bytes(pid);
            let packet = packet!([SYNC_BYTE, 0b00000000 | hi, lo], [0; 185]);
            assert_eq!(packet.pid().get(), pid);
            assert!(!packet.is_normal());
        }

        let packet = packet!([SYNC_BYTE, 0x00, 0x00, 0b01000000], [0; 184]);
        assert_eq!(packet.scrambling_control(), 0b01);
        assert!(!packet.is_normal());

        let packet = packet!([SYNC_BYTE, 0x00, 0x00, 0b00000000], [0; 184]);
        assert_eq!(packet.adaptation_field_control(), 0b00);
        assert!(!packet.is_normal());
        let packet = packet!([SYNC_BYTE, 0x00, 0x00, 0b00100000, 184], [0; 183]);
        assert_eq!(packet.adaptation_field_control(), 0b10);
        assert!(!packet.is_normal());
        let packet = packet!([SYNC_BYTE, 0x00, 0x00, 0b00110000, 183], [0; 183]);
        assert_eq!(packet.adaptation_field_control(), 0b11);
        assert!(!packet.is_normal());
    }

    #[test]
    fn test_packet_accessor() {
        assert!(PACKET_1.is_normal());
        assert!(!PACKET_1.error_indicator());
        assert!(PACKET_1.unit_start_indicator());
        assert!(!PACKET_1.priority());
        assert_eq!(PACKET_1.pid(), Pid::new(0x0012));
        assert_eq!(PACKET_1.scrambling_control(), 0b00);
        assert!(!PACKET_1.is_scrambled());
        assert_eq!(PACKET_1.adaptation_field_control(), 0b01);
        assert_eq!(PACKET_1.continuity_counter(), 8);

        assert_eq!(PACKET_1.adaptation_field_length(), None);
        assert_matches!(PACKET_1.adaptation_field(), None);

        assert_eq!(PACKET_1.payload(), Some(&PACKET_1.0[4..]));
        assert!(!Packet::payload_is_pes(PACKET_1.payload().unwrap()));

        //

        assert!(PACKET_2.is_normal());
        assert!(!PACKET_2.error_indicator());
        assert!(!PACKET_2.unit_start_indicator());
        assert!(!PACKET_2.priority());
        assert_eq!(PACKET_2.pid(), Pid::new(0x0140));
        assert_eq!(PACKET_2.scrambling_control(), 0b00);
        assert!(!PACKET_2.is_scrambled());
        assert_eq!(PACKET_2.adaptation_field_control(), 0b11);
        assert_eq!(PACKET_2.continuity_counter(), 7);

        assert_eq!(PACKET_2.adaptation_field_length(), Some(63));
        assert_matches!(
            PACKET_2.adaptation_field(),
            Some(AdaptationField {
                discontinuity_indicator: false,
                random_access_indicator: false,
                es_priority_indicator: false,
                pcr: None,
                pcr_original: None,
                splice_countdown: None,
                private_data: None,
                extension: None
            })
        );

        assert_eq!(PACKET_2.payload(), Some(&PACKET_2.0[68..]));
        assert!(!Packet::payload_is_pes(PACKET_2.payload().unwrap()));

        //

        assert!(PACKET_3.is_normal());
        assert!(!PACKET_3.error_indicator());
        assert!(!PACKET_3.unit_start_indicator());
        assert!(!PACKET_3.priority());
        assert_eq!(PACKET_3.pid(), Pid::new(0x0111));
        assert_eq!(PACKET_3.scrambling_control(), 0b00);
        assert!(!PACKET_3.is_scrambled());
        assert_eq!(PACKET_3.adaptation_field_control(), 0b10);
        assert_eq!(PACKET_3.continuity_counter(), 0);

        assert_eq!(PACKET_3.adaptation_field_length(), Some(183));
        assert_matches!(
            PACKET_3.adaptation_field(),
            Some(AdaptationField {
                discontinuity_indicator: false,
                random_access_indicator: false,
                es_priority_indicator: false,
                pcr: Some(Pcr {
                    base: 7052388613,
                    extension: 249
                }),
                pcr_original: None,
                splice_countdown: None,
                private_data: None,
                extension: None
            })
        );

        assert_eq!(PACKET_3.payload(), None);
    }

    #[test]
    fn test_packet_iter() {
        let data = [PACKET_1.0, PACKET_2.0, PACKET_3.0].concat();
        let mut iter = Packet::iter(&*data);
        assert_eq!(iter.next().unwrap().unwrap(), PACKET_1);
        assert_eq!(iter.next().unwrap().unwrap(), PACKET_2);
        assert_eq!(iter.next().unwrap().unwrap(), PACKET_3);
        assert_matches!(iter.next(), None);
    }
}
