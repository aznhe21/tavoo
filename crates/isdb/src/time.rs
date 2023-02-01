//! MPEG2-TSにおける日付時刻。

use std::fmt::{self, Write};

use crate::utils::BytesExt;

fn write_hundreds<W: Write>(w: &mut W, n: u8) -> fmt::Result {
    let h = b'0' + n / 10;
    let l = b'0' + n % 10;
    w.write_char(h as char)?;
    w.write_char(l as char)
}

/// 曜日。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Weekday {
    /// 月曜日。
    Mon,
    /// 火曜日。
    Tue,
    /// 水曜日。
    Wed,
    /// 木曜日。
    Thu,
    /// 金曜日。
    Fri,
    /// 土曜日。
    Sat,
    /// 日曜日。
    Sun,
}

/// [`MjdDate`]から変換された日本の日付。
#[derive(Clone, PartialEq, Eq)]
pub struct Date {
    /// 年。
    pub year: i32,
    /// 月（1～12）。
    pub month: u8,
    /// 日（1～31）。
    pub day: u8,
    /// 曜日。
    pub weekday: Weekday,
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.year.fmt(f)?;

        f.write_char('-')?;
        write_hundreds(f, self.month)?;

        f.write_char('-')?;
        write_hundreds(f, self.day)
    }
}

impl fmt::Debug for Date {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.year.fmt(f)?;

        f.write_char('-')?;
        write_hundreds(f, self.month)?;

        f.write_char('-')?;
        write_hundreds(f, self.day)?;

        f.write_str(" (")?;
        self.weekday.fmt(f)?;
        f.write_char(')')
    }
}

/// 修正ユリウス日。
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MjdDate(pub u16);

impl MjdDate {
    /// `data`から`MjdDate`を読み取る。
    #[inline]
    pub fn read(data: &[u8; 2]) -> MjdDate {
        MjdDate(data.read_be_16())
    }

    /// `MjdDate`から`Date`に変換する。
    ///
    /// 無効な日付の場合は`None`が返る。
    pub fn to_date(&self) -> Option<Date> {
        if self.0 == u16::MAX || self.0 < 15018 {
            return None;
        }

        let yd = ((self.0 as f32 - 15078.2) / 365.25) as i32;
        let md = ((self.0 as f32 - 14956.1 - (yd as f32 * 365.25) as u16 as f32) / 30.6001) as u8;

        let day =
            (self.0 as i32 - 14956 - (yd as f32 * 365.25) as i32 - (md as f32 * 30.6001) as i32)
                as u8;
        let weekday = match (self.0 + 2) % 7 {
            0 => Weekday::Mon,
            1 => Weekday::Tue,
            2 => Weekday::Wed,
            3 => Weekday::Thu,
            4 => Weekday::Fri,
            5 => Weekday::Sat,
            6 => Weekday::Sun,
            _ => unreachable!(),
        };
        let (year, month) = if md == 14 || md == 15 {
            (yd + 1901, md - 1 - 12)
        } else {
            (yd + 1900, md - 1)
        };

        Some(Date {
            year,
            month,
            day,
            weekday,
        })
    }
}

impl fmt::Display for MjdDate {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.to_date() {
            Some(date) => date.fmt(f),
            None => f
                .debug_tuple("MjdDate")
                .field(&crate::utils::UpperHex(self.0))
                .finish(),
        }
    }
}

impl fmt::Debug for MjdDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.to_date() {
            Some(date) => date.fmt(f),
            None => f
                .debug_tuple("MjdDate")
                .field(&crate::utils::UpperHex(self.0))
                .finish(),
        }
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

impl fmt::Display for DateTime {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_date_time() {
        // MJD = 45218, HMS = 12:34:56
        let mjd_date = MjdDate::read(&[0xB0, 0xA2]);
        assert_eq!(mjd_date, MjdDate(0xB0A2));

        assert_eq!(
            mjd_date.to_date(),
            Some(Date {
                year: 1982,
                month: 9,
                day: 6,
                weekday: Weekday::Mon
            })
        );
        assert_eq!(mjd_date.to_date().unwrap().to_string(), "1982-09-06");
        assert_eq!(mjd_date.to_string(), "1982-09-06");
        assert_eq!(format!("{:?}", mjd_date), "1982-09-06 (Mon)");

        let mjd_date = MjdDate(15018);
        assert_eq!(
            mjd_date.to_date(),
            Some(Date {
                year: 1900,
                month: 1,
                day: 1,
                weekday: Weekday::Sat,
            })
        );

        let mjd_date = MjdDate(0xFFFF);
        assert_eq!(mjd_date.to_date(), None);
        assert_eq!(mjd_date.to_string(), "MjdDate(FFFF)");
        assert_eq!(format!("{:?}", mjd_date), "MjdDate(FFFF)");

        let dt = DateTime::read(&[0xB0, 0xA2, 0x12, 0x34, 0x56]);
        assert_eq!(dt.date, MjdDate(0xB0A2));
        assert_eq!(dt.hour, 12);
        assert_eq!(dt.minute, 34);
        assert_eq!(dt.second, 56);
        assert_eq!(dt.to_string(), "1982-09-06 12:34:56");
        assert_eq!(format!("{:?}", dt), "1982-09-06 (Mon) 12:34:56");
    }
}
