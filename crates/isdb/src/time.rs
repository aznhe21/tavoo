//! MPEG2-TSにおける日付時刻。

use std::fmt::{self, Write};

use crate::utils::BytesExt;

fn write_hundreds<W: Write>(w: &mut W, n: u8) -> fmt::Result {
    let h = b'0' + n / 10;
    let l = b'0' + n % 10;
    w.write_char(h as char)?;
    w.write_char(l as char)
}

/// 修正ユリウス日。
#[derive(Clone, PartialEq, Eq)]
pub struct MjdDate {
    /// 1900年からの年（2003年＝103）。
    pub year: u16,
    /// 月（1月＝1、12月＝12）。
    pub month: u8,
    /// 日（1～31）。
    pub day: u8,
    /// 曜日（月曜日＝1、日曜日＝7）。
    pub day_of_week: u8,
}

impl MjdDate {
    /// `data`から`MjdDate`を読み取る。
    pub fn read(data: &[u8; 2]) -> MjdDate {
        let mjd = data.read_be_16();
        let yd = ((mjd as f32 - 15078.2) / 365.25) as u16;
        let md = ((mjd as f32 - 14956.1 - (yd as f32 * 365.25) as u16 as f32) / 30.6001) as u8;

        let day = (mjd - 14956 - (yd as f32 * 365.25) as u16 - (md as f32 * 30.6001) as u16) as u8;
        let day_of_week = ((mjd + 2) % 7 + 1) as u8;
        let (year, month) = if md == 14 || md == 15 {
            (yd + 1, md - 1 - 12)
        } else {
            (yd, md - 1)
        };

        MjdDate {
            year,
            month,
            day,
            day_of_week,
        }
    }
}

impl fmt::Debug for MjdDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (1900 + self.year).fmt(f)?;

        f.write_char('-')?;
        write_hundreds(f, self.month)?;

        f.write_char('-')?;
        write_hundreds(f, self.day)
    }
}

impl fmt::Display for MjdDate {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

/// 修正ユリウス日と日本標準時からなる日付時刻。
#[derive(Clone, PartialEq, Eq)]
pub struct DateTime {
    /// 修正ユリウス日。
    pub date: MjdDate,
    /// 時（0～23）。
    pub hour: u8,
    /// 分（0～59）。
    pub minute: u8,
    /// 秒（0～60）。
    pub second: u8,
}

impl DateTime {
    /// `data`から`DateTime`を読み取る。
    pub fn read(data: &[u8; 5]) -> DateTime {
        let date = MjdDate::read(&data[0..=1].try_into().unwrap());

        let hour = crate::utils::read_bcd_digit(data[2]);
        let minute = crate::utils::read_bcd_digit(data[3]);
        let second = crate::utils::read_bcd_digit(data[4]);

        DateTime {
            date,
            hour,
            minute,
            second,
        }
    }
}

impl fmt::Debug for DateTime {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.date.fmt(f)?;
        f.write_char(' ')?;

        write_hundreds(f, self.hour)?;
        f.write_char(':')?;
        write_hundreds(f, self.minute)?;
        f.write_char(':')?;
        write_hundreds(f, self.second)
    }
}

impl fmt::Display for DateTime {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_date_time() {
        // MJD = 45218, HMS = 12:34:56
        let date = MjdDate::read(&[0xB0, 0xA2]);
        assert_eq!(date.year, 82);
        assert_eq!(date.month, 9);
        assert_eq!(date.day, 6);
        assert_eq!(date.day_of_week, 1);
        assert_eq!(date.to_string(), "1982-09-06");

        let dt = DateTime::read(&[0xB0, 0xA2, 0x12, 0x34, 0x56]);
        assert_eq!(dt.date.year, 82);
        assert_eq!(dt.date.month, 9);
        assert_eq!(dt.date.day, 6);
        assert_eq!(dt.date.day_of_week, 1);
        assert_eq!(dt.hour, 12);
        assert_eq!(dt.minute, 34);
        assert_eq!(dt.second, 56);
    }
}
