//! MPEG2-TSのパケット。

use std::fmt;
use std::io::{self, Read};

use crate::pid::Pid;
use crate::time::Timestamp;

const SYNC_BYTE: u8 = 0x47;
const PACKET_SIZE: usize = 188;

/// MPEG2-TSのパケット。
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Packet(pub [u8; PACKET_SIZE]);

impl Packet {
    /// `r`からTSパケットを順次読み込むイテレーターを生成する。
    ///
    /// # サンプル
    ///
    /// ```
    /// # fn main() -> std::io::Result<()> {
    /// # let file = &mut (&[] as &[u8]);
    /// for packet in isdb::Packet::iter(file) {
    ///     let packet = packet?;
    ///
    ///     // 同期バイトは常に正しい
    ///     assert_eq!(packet.sync_byte(), 0x47);
    ///     // ただしパケットとして正しいかは不明
    ///     println!("パケットが正常か：{}", packet.is_normal());
    /// }
    /// # Ok(())
    /// # }
    /// ```
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
    ///
    /// 同期バイトやトランスポートエラーインジケーターによるエラー検知に加え、
    /// 予約されたPIDなどパケットとしてあり得ない状態であることも判断材料である。
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

    /// 同期バイトを返す。
    #[inline]
    pub fn sync_byte(&self) -> u8 {
        self.0[0]
    }

    /// トランスポートエラーインジケーターを返す。
    #[inline]
    pub fn error_indicator(&self) -> bool {
        self.0[1] & 0b10000000 != 0
    }

    /// ペイロードユニット開始インジケーターを返す。
    #[inline]
    pub fn unit_start_indicator(&self) -> bool {
        self.0[1] & 0b01000000 != 0
    }

    /// トランスポート優先度を返す。
    #[inline]
    pub fn priority(&self) -> bool {
        self.0[1] & 0b00100000 != 0
    }

    /// PIDを返す。
    #[inline]
    pub fn pid(&self) -> Pid {
        Pid::read(&self.0[1..])
    }

    /// トランスポートスクランブル制御（2ビット）を返す。
    #[inline]
    pub fn scrambling_control(&self) -> u8 {
        (self.0[3] & 0b11000000) >> 6
    }

    /// パケットがスクランブル処理されているかを返す。
    #[inline]
    pub fn is_scrambled(&self) -> bool {
        self.scrambling_control() & 0b10 != 0
    }

    /// アダプテーションフィールド制御（2ビット）を返す。
    #[inline]
    pub fn adaptation_field_control(&self) -> u8 {
        (self.0[3] & 0b00110000) >> 4
    }

    /// 連続性指標（4ビット）を返す。
    #[inline]
    pub fn continuity_counter(&self) -> u8 {
        self.0[3] & 0b00001111
    }

    /// パケットがアダプテーションフィールドを含むかどうかを返す。
    #[inline]
    pub fn has_adaptation_field(&self) -> bool {
        self.adaptation_field_control() & 0b10 != 0
    }

    #[inline]
    fn adaptation_field_length_raw(&self) -> u8 {
        self.0[4]
    }

    /// アダプテーションフィールドがある場合、adaptation_field_lengthを返す。
    #[inline]
    pub fn adaptation_field_length(&self) -> Option<u8> {
        self.has_adaptation_field()
            .then(|| self.adaptation_field_length_raw())
    }

    /// アダプテーションフィールドを返す。
    #[inline]
    pub fn adaptation_field(&self) -> Option<AdaptationField> {
        AdaptationField::new(self)
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

    /// 前回の連続性指標である`last_cc`を元にパケット順の正当性を確認する。
    ///
    /// `last_cc`の初期値は`0x10`以上とする。
    pub fn validate_cc(&self, last_cc: &mut u8) -> bool {
        let pid = self.pid();
        let cc = if self.has_payload() {
            self.continuity_counter()
        } else {
            0x10
        };
        let is_discontinuity = self
            .adaptation_field()
            .map_or(false, |af| af.discontinuity_indicator());
        let cc_ok = pid == Pid::NULL
            || is_discontinuity
            || cc >= 0x10
            || *last_cc >= 0x10
            || (*last_cc + 1) & 0x0F == cc;
        *last_cc = cc;

        cc_ok
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

/// TSパケット内のアダプテーションフィールド。
#[derive(Debug)]
pub struct AdaptationField<'a>(&'a [u8]);

impl<'a> AdaptationField<'a> {
    #[inline]
    fn new(packet: &'a Packet) -> Option<AdaptationField<'a>> {
        packet
            .adaptation_field_length()
            .filter(|&length| length >= 1)
            .and_then(|length| packet.0.get(5..5 + length as usize))
            .map(AdaptationField)
    }

    /// 不連続性インジケーターを返す。
    #[inline]
    pub fn discontinuity_indicator(&self) -> bool {
        // Safety: 生成時に確認済み
        unsafe { crate::utils::assume!(self.0.len() >= 1) }

        self.0[0] & 0b10000000 != 0
    }

    /// ランダムアクセスインジケーターを返す。
    #[inline]
    pub fn random_access_indicator(&self) -> bool {
        // Safety: 生成時に確認済み
        unsafe { crate::utils::assume!(self.0.len() >= 1) }

        self.0[0] & 0b01000000 != 0
    }

    /// エレメンタリーストリーム優先度インジケーターを返す。
    #[inline]
    pub fn es_priority_indicator(&self) -> bool {
        // Safety: 生成時に確認済み
        unsafe { crate::utils::assume!(self.0.len() >= 1) }

        self.0[0] & 0b00100000 != 0
    }

    /// PCRフラグを返す。
    #[inline]
    pub fn pcr_flag(&self) -> bool {
        // Safety: 生成時に確認済み
        unsafe { crate::utils::assume!(self.0.len() >= 1) }

        self.0[0] & 0b00010000 != 0
    }

    /// オリジナルPCRフラグを返す。
    #[inline]
    pub fn original_pcr_flag(&self) -> bool {
        // Safety: 生成時に確認済み
        unsafe { crate::utils::assume!(self.0.len() >= 1) }

        self.0[0] & 0b00001000 != 0
    }

    /// 編集点フラグを返す。
    #[inline]
    pub fn splicing_point_flag(&self) -> bool {
        // Safety: 生成時に確認済み
        unsafe { crate::utils::assume!(self.0.len() >= 1) }

        self.0[0] & 0b00000100 != 0
    }

    /// プライベートデータフラグを返す。
    #[inline]
    pub fn private_data_flag(&self) -> bool {
        // Safety: 生成時に確認済み
        unsafe { crate::utils::assume!(self.0.len() >= 1) }

        self.0[0] & 0b00000010 != 0
    }

    /// 拡張フラグを返す。
    #[inline]
    pub fn extension_flag(&self) -> bool {
        // Safety: 生成時に確認済み
        unsafe { crate::utils::assume!(self.0.len() >= 1) }

        self.0[0] & 0b00000001 != 0
    }

    fn pcr_offset(&self) -> Option<usize> {
        if !self.pcr_flag() {
            None
        } else {
            Some(1)
        }
    }

    fn opcr_offset(&self) -> Option<usize> {
        if !self.original_pcr_flag() {
            None
        } else if self.pcr_flag() {
            Some(1 + 6)
        } else {
            Some(1)
        }
    }

    fn splice_countdown_offset(&self) -> Option<usize> {
        if !self.splicing_point_flag() {
            None
        } else if self.pcr_flag() && self.original_pcr_flag() {
            Some(1 + 6 + 6)
        } else if self.pcr_flag() || self.original_pcr_flag() {
            Some(1 + 6)
        } else {
            Some(1)
        }
    }

    fn private_data_offset(&self) -> Option<usize> {
        if !self.private_data_flag() {
            None
        } else {
            Some(
                1 + if self.pcr_flag() { 6 } else { 0 }
                    + if self.original_pcr_flag() { 6 } else { 0 }
                    + if self.splicing_point_flag() { 1 } else { 0 },
            )
        }
    }

    /// PCRを返す。
    pub fn pcr(&self) -> Option<Timestamp> {
        self.pcr_offset()
            .and_then(|offset| self.0.get(offset..offset + 6))
            .map(|slice| slice.try_into().unwrap())
            .and_then(Timestamp::read_pcr)
    }

    /// オリジナルPCRを返す。
    pub fn original_pcr(&self) -> Option<Timestamp> {
        self.opcr_offset()
            .and_then(|offset| self.0.get(offset..offset + 6))
            .map(|slice| slice.try_into().unwrap())
            .and_then(Timestamp::read_pcr)
    }

    /// スプライスカウントダウンを返す。
    pub fn splice_countdown(&self) -> Option<u8> {
        self.splice_countdown_offset()
            .and_then(|offset| self.0.get(offset))
            .copied()
    }

    /// プライベートデータを返す。
    pub fn private_data(&self) -> Option<&[u8]> {
        let offset = self.private_data_offset()?;
        let len = *self.0.get(offset)?;
        self.0.get(offset + len as usize..)
    }
}

/// [`Packet::iter`]から返される。TSパケットを順次読み込むイテレーター。
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
        assert!(PACKET_2.adaptation_field().is_some());
        let af = PACKET_2.adaptation_field().unwrap();
        assert!(!af.discontinuity_indicator());
        assert!(!af.random_access_indicator());
        assert!(!af.es_priority_indicator());
        assert!(af.pcr().is_none());
        assert!(af.original_pcr().is_none());
        assert!(af.private_data().is_none());

        assert_eq!(PACKET_2.payload(), Some(&PACKET_2.0[68..]));

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
        assert!(PACKET_3.adaptation_field().is_some());
        let af = PACKET_3.adaptation_field().unwrap();
        assert!(!af.discontinuity_indicator());
        assert!(!af.random_access_indicator());
        assert!(!af.es_priority_indicator());
        assert_eq!(af.pcr(), Some(Timestamp::new(7052388613, 249)));
        assert!(af.original_pcr().is_none());
        assert!(af.private_data().is_none());

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
