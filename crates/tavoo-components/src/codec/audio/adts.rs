//! AACで使われるADTSをパースする。

use std::fmt;

/// ADTSのヘッダ。
///
/// フィールドの名前はffmpegの`AACADTSHeaderInfo`由来。
#[derive(Clone, PartialEq, Eq)]
pub struct Header {
    /// MPEG-4 Audio Object Type (`1..=4`)
    pub object_type: u8,
    /// MPEG-4 Sampling Frequency Index (`0..=12`)
    pub sampling_index: u8,
    /// MPEG-4 Channel Configuration (`0..=7`)
    pub chan_config: u8,
    /// Frame length (`7..=8191`)
    pub frame_length: u16,
    /// Number of AAC frames (`1..=4`)
    pub num_aac_frames: u8,
    /// CRC check (16 bits)
    pub crc: Option<u16>,
}

impl Header {
    /// ADTSのヘッダを探して返す。
    // https://wiki.multimedia.cx/index.php/ADTS
    pub fn find(mut buf: &[u8]) -> Option<Header> {
        // Syncwordを探す
        loop {
            let pos = memchr::memchr(0xFF, buf)?;
            buf = &buf[pos..];
            if buf.len() < 7 {
                return None;
            }

            // Syncwordの下位ビットとLayer
            if buf[1] & 0xF6 == 0xF0 {
                break;
            }
            buf = &buf[1..];
        }

        let crc_absent = buf[1] & 0b1 != 0;
        let object_type = (buf[2] >> 6) + 1;
        let sampling_index = (buf[2] >> 2) & 0b1111;
        if sampling_index > 12 {
            return None;
        }
        let chan_config = ((buf[2] & 0b1) << 2) | (buf[3] >> 6);
        let frame_length =
            (((buf[3] & 0b11) as u16) << 11) | ((buf[4] as u16) << 3) | ((buf[5] >> 5) as u16);
        if frame_length < 7 {
            return None;
        }

        let num_aac_frames = (buf[6] & 0b11) + 1;
        let crc = (!crc_absent && buf.len() >= 9).then(|| ((buf[7] as u16) << 8) | (buf[8] as u16));

        Some(Header {
            object_type,
            sampling_index,
            chan_config,
            frame_length,
            num_aac_frames,
            crc,
        })
    }

    /// サンプリングレート。
    #[inline]
    pub fn sample_rate(&self) -> u32 {
        match self.sampling_index {
            0 => 96000,
            1 => 88200,
            2 => 64000,
            3 => 48000,
            4 => 44100,
            5 => 32000,
            6 => 24000,
            7 => 22050,
            8 => 16000,
            9 => 12000,
            10 => 11025,
            11 => 8000,
            12 => 7350,
            _ => unreachable!(),
        }
    }

    /// サンプル数。
    #[inline]
    pub fn samples(&self) -> u32 {
        (self.num_aac_frames as u32 + 1) * 1024
    }

    /// ビットレート。
    #[inline]
    pub fn bit_rate(&self) -> u32 {
        self.frame_length as u32 * 8 * self.sample_rate() / self.samples()
    }

    /// チャンネル数。
    #[inline]
    pub fn num_channels(&self) -> u8 {
        if self.chan_config != 7 {
            self.chan_config
        } else {
            8
        }
    }
}

impl fmt::Debug for Header {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Header")
            .field("object_type", &self.object_type)
            .field("sampling_rate", &self.sample_rate())
            .field("chan_config", &self.chan_config)
            .field("frame_length", &self.frame_length)
            .field("num_aac_frames", &self.num_aac_frames)
            .field("samples", &self.samples())
            .field("bit_rate", &self.bit_rate())
            .field("crc", &self.crc)
            .finish()
    }
}
