//! ARIB STD-B10で規定される日付時刻。

use std::cmp::Ordering;
use std::fmt::{self, Write};
use std::ops;
use std::time::Duration;

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
#[derive(Clone, Copy, PartialEq, Eq)]
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
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MjdDate(pub u16);

impl MjdDate {
    /// `data`から`MjdDate`を読み取る。
    #[inline]
    pub fn read(data: &[u8; 2]) -> MjdDate {
        MjdDate(data.read_be_16())
    }

    /// `MjdDate`から`Date`に変換する。
    ///
    /// 無効な日付（1970年3月1日より前など）の場合は`None`が返る。
    pub fn to_date(&self) -> Option<Date> {
        if self.0 == u16::MAX || self.0 < 15079 {
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
#[derive(Clone, Copy, PartialEq, Eq)]
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

const FULL_PER_SECS: u64 = 27_000_000;

/// PCRやPTS等を表すタイムスタンプ。時間は27MHz単位で表現される。
///
/// 演算・比較はラップアラウンドを考慮して行われ、自動でオーバーフローする。
///
/// # サンプル
///
/// ```
/// use std::time::Duration;
/// use isdb::time::Timestamp;
///
/// assert_eq!(Timestamp::new(100, 10) + Timestamp::new(200, 20), Timestamp::new(300, 30));
/// assert_eq!(Timestamp::new(10, 100) + Timestamp::new(20, 200), Timestamp::new(31, 0));
/// assert_eq!(Timestamp::new(300, 30) - Timestamp::new(200, 20), Timestamp::new(100, 10));
/// assert_eq!(Timestamp::new(31, 0) - Timestamp::new(20, 200), Timestamp::new(10, 100));
/// assert!(Timestamp::new(100, 10) < Timestamp::new(100, 11));
/// assert!(Timestamp::new(100, 11) > Timestamp::new(100, 10));
///
/// // ラップアラウンドを考慮した演算
/// assert_eq!(Timestamp::MAX + Timestamp::new(0, 1), Timestamp::ZERO);
/// assert_eq!(Timestamp::MAX + Timestamp::MAX, Timestamp::new(2u64.pow(33) - 1, 298));
/// assert_eq!(Timestamp::ZERO - Timestamp::new(0, 1), Timestamp::MAX);
/// assert_eq!(Timestamp::new(1, 0) - Timestamp::new(2, 0), Timestamp::new(2u64.pow(33) - 1, 0));
///
/// // ラップアラウンドを考慮したDurationとの演算
/// assert_eq!(Timestamp::ZERO + Duration::from_secs(1), Timestamp::from_full(27_000_000));
/// assert_eq!(Timestamp::MAX + Duration::from_secs(1), Timestamp::from_full(27_000_000 - 1));
/// assert_eq!(Timestamp::ZERO + Duration::new(95443, 717688887), Timestamp::MAX);
/// assert_eq!(Timestamp::ZERO + Duration::new(95443, 717688888), Timestamp::ZERO);
/// assert_eq!(Timestamp::ZERO + Duration::new(190887, 435377775), Timestamp::MAX);
/// assert_eq!(Timestamp::ZERO + Duration::new(190887, 435377776), Timestamp::ZERO);
/// assert_eq!(
///     Timestamp::from_duration(Duration::from_secs(1)) - Duration::from_secs(1),
///     Timestamp::ZERO,
/// );
/// assert_eq!(
///     Timestamp::ZERO - Duration::from_secs(1),
///     Timestamp::from_full(Timestamp::MAX.full() - (27_000_000 - 1)),
/// );
/// assert_eq!(Timestamp::ZERO - Duration::new(95443, 717688887), Timestamp::from_full(1));
/// assert_eq!(Timestamp::ZERO - Duration::new(95443, 717688888), Timestamp::ZERO);
/// assert_eq!(Timestamp::ZERO - Duration::new(190887, 435377775), Timestamp::from_full(1));
/// assert_eq!(Timestamp::ZERO - Duration::new(190887, 435377776), Timestamp::ZERO);
///
/// // ラップアラウンドを考慮した比較
/// assert!(Timestamp::new(2u64.pow(33) - 100, 299) < Timestamp::ZERO);
/// assert!(Timestamp::ZERO > Timestamp::new(2u64.pow(33) - 100, 0));
/// ```
#[derive(Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Timestamp(u64);

impl Timestamp {
    /// ゼロを表すタイムスタンプ。
    pub const ZERO: Timestamp = Timestamp::new(0, 0);

    /// 最大のタイムスタンプ。
    pub const MAX: Timestamp = Timestamp::new(2u64.pow(33) - 1, 299);

    // ラップアラウンドがあると見做す差分値。
    const WRAP_THRESH: u64 = Self::MAX.0 / 2;

    /// 90kHZの`base`と27MHzの`extension`から`Timestamp`を生成する。
    ///
    /// # パニック
    ///
    /// `base`が8589934591を、または`extension`が299を超える時、このメソッドはパニックを起こす。
    #[inline]
    pub const fn new(base: u64, extension: u16) -> Timestamp {
        assert!(base < 2u64.pow(33), "baseがオーバーフロー");
        assert!(extension < 300, "extensionがオーバーフロー");
        Timestamp(base * 300 + extension as u64)
    }

    /// 90kHZの`base`と27MHzの`extension`から`Timestamp`を生成する。
    ///
    /// `base`が8589934591を、または`extension`が299を超える場合は`None`を返す。
    #[inline]
    pub const fn try_new(base: u64, extension: u16) -> Option<Timestamp> {
        if base < 2u64.pow(33) && extension < 300 {
            Some(Timestamp(base * 300 + extension as u64))
        } else {
            None
        }
    }

    /// 27MHz単位で表現されるタイムスタンプから`Timestamp`を生成する。
    ///
    /// # パニック
    ///
    /// このメソッドは[`Timestamp::full`]の戻り値から`Timestamp`を再構築するためのものであり、
    /// 最大のタイムスタンプを超える値が渡された場合はパニックする。
    ///
    /// # サンプル
    ///
    /// ```
    /// use isdb::time::Timestamp;
    ///
    /// assert_eq!(Timestamp::from_full(123456789), Timestamp::new(411522, 189));
    /// assert_eq!(Timestamp::from_full(Timestamp::ZERO.full()), Timestamp::ZERO);
    /// assert_eq!(Timestamp::from_full(Timestamp::MAX.full()), Timestamp::MAX);
    /// ```
    #[inline]
    pub const fn from_full(full: u64) -> Timestamp {
        assert!(full <= Self::MAX.0, "Timestamp::from_fullでオーバーフロー");
        Timestamp(full)
    }

    /// [`Duration`]から`Timestamp`を生成する。この変換により誤差が生じる場合がある。
    ///
    /// `dur`の値が`Timestamp`の最大値を超える場合は`Timestamp::MAX`が返る。
    ///
    /// # サンプル
    ///
    /// ```
    /// use std::time::Duration;
    /// use isdb::time::Timestamp;
    ///
    /// assert_eq!(Timestamp::from_duration(Duration::ZERO), Timestamp::ZERO);
    /// assert_eq!(
    ///     Timestamp::from_duration(Duration::new(123, 456789000)),
    ///     Timestamp::from_full((123.456789 * 27_000_000.) as u64),
    /// );
    /// assert_eq!(
    ///     Timestamp::from_duration(Duration::new(u64::MAX, 999_999_999)),
    ///     Timestamp::MAX,
    /// );
    /// ```
    #[inline]
    pub const fn from_duration(dur: Duration) -> Timestamp {
        let secs = dur.as_secs();
        let nanos = dur.subsec_nanos();

        // Option::and_thenはconst fnではない
        let x = match secs.checked_mul(FULL_PER_SECS) {
            Some(x) => x.checked_add(nanos as u64 * 27 / 1_000),
            None => None,
        };
        match x {
            Some(x) if x <= Self::MAX.0 => Timestamp(x),
            _ => Self::MAX,
        }
    }

    /// PCRを格納する`data`から`Timestamp`を読み取る。
    #[inline]
    pub fn read_pcr(data: &[u8; 6]) -> Timestamp {
        let base =
            ((data[0..=3].read_be_32() as u64) << 1) | (((data[4] & 0b10000000) >> 7) as u64);
        let extension = data[4..=5].read_be_16() & 0b0000_0001_1111_1111;
        Timestamp::new(base, extension)
    }

    /// PTS・DTSを格納する`data`から`Timestamp`を読み取る。
    #[inline]
    pub fn read_pts(data: &[u8; 5]) -> Timestamp {
        let timestamp = ((data[0] & 0b00001110) as u64) << 29
            | (((data[1..=2].read_be_16() & 0b11111111_11111110) as u64) << 14)
            | ((data[3..=4].read_be_16() >> 1) as u64);
        Timestamp::new(timestamp, 0)
    }

    /// 27MHz単位で表現されるタイムスタンプを取得する。
    ///
    /// この値は42ビットに収まる。
    ///
    /// # サンプル
    ///
    /// ```
    /// use isdb::time::Timestamp;
    ///
    /// assert_eq!(Timestamp::new(411522, 189).full(), 123456789);
    /// assert_eq!(Timestamp::ZERO.full(), 0);
    /// assert_eq!(Timestamp::MAX.full(), 2576980377599);
    /// ```
    #[inline]
    pub const fn full(&self) -> u64 {
        self.0
    }

    /// タイムスタンプの90kHz部分を取得する。
    ///
    /// この値は33ビットに収まる。
    ///
    /// # サンプル
    ///
    /// ```
    /// use isdb::time::Timestamp;
    ///
    /// assert_eq!(Timestamp::new(100, 10).base(), 100);
    /// assert_eq!(Timestamp::ZERO.base(), 0);
    /// assert_eq!(Timestamp::MAX.base(), 2u64.pow(33) - 1);
    /// ```
    #[inline]
    pub const fn base(&self) -> u64 {
        self.0 / 300
    }

    /// タイムスタンプの27MHz部分を取得する。
    ///
    /// この値は`0..300`の範囲であり9ビットに収まる。
    ///
    /// # サンプル
    ///
    /// ```
    /// use isdb::time::Timestamp;
    ///
    /// assert_eq!(Timestamp::new(100, 10).extension(), 10);
    /// assert_eq!(Timestamp::ZERO.extension(), 0);
    /// assert_eq!(Timestamp::MAX.extension(), 299);
    /// ```
    #[inline]
    pub const fn extension(&self) -> u16 {
        (self.0 % 300) as u16
    }

    /// タイムスタンプを秒に変換する。
    ///
    /// # サンプル
    ///
    /// ```
    /// use std::time::Duration;
    /// use isdb::time::Timestamp;
    ///
    /// assert_eq!(Timestamp::new(500_000, 10).as_secs(), 5);
    /// assert_eq!(Timestamp::ZERO.as_secs(), 0);
    /// assert_eq!(Timestamp::MAX.as_secs(), 95443);
    /// assert_eq!(Timestamp::from(Duration::from_secs_f32(123.456789)).as_secs(), 123);
    /// ```
    #[inline]
    pub const fn as_secs(&self) -> u64 {
        self.0 / FULL_PER_SECS
    }

    /// タイムスタンプを秒成分を含むナノ秒に変換する。
    ///
    /// # サンプル
    ///
    /// ```
    /// use std::time::Duration;
    /// use isdb::time::Timestamp;
    ///
    /// assert_eq!(Timestamp::new(500_000, 10).as_nanos(), 5555555925);
    /// assert_eq!(Timestamp::ZERO.as_nanos(), 0);
    /// assert_eq!(Timestamp::MAX.as_nanos(), 95443717688851);
    ///
    /// // `Duration::new(100, 50).as_nanos()`は100000000050だが、
    /// // `Timestamp`への変換により誤差が生じる
    /// assert_eq!(Timestamp::from(Duration::new(100, 50)).as_nanos(), 100000000037);
    /// ```
    #[inline]
    pub const fn as_nanos(&self) -> u64 {
        self.0 * 1_000 / 27
    }

    /// タイムスタンプを[`Duration`]に変換する。
    ///
    /// # サンプル
    ///
    /// ```
    /// use std::time::Duration;
    /// use isdb::time::Timestamp;
    ///
    /// assert_eq!(Timestamp::new(100, 10).to_duration(), Duration::from_nanos(1111481));
    /// assert_eq!(Timestamp::MAX.to_duration(), Duration::new(95443, 717688851));
    /// ```
    #[inline]
    pub const fn to_duration(&self) -> Duration {
        let secs = self.0 / FULL_PER_SECS;
        let nanos = (self.0 % FULL_PER_SECS * 1_000 / 27) as u32;
        Duration::new(secs, nanos)
    }
}

impl From<Duration> for Timestamp {
    #[inline]
    fn from(value: Duration) -> Timestamp {
        Timestamp::from_duration(value)
    }
}

impl From<Timestamp> for Duration {
    #[inline]
    fn from(value: Timestamp) -> Duration {
        value.to_duration()
    }
}

impl ops::Add for Timestamp {
    type Output = Timestamp;

    #[inline]
    fn add(self, rhs: Timestamp) -> Timestamp {
        match self.0 + rhs.0 {
            x if x > Self::MAX.0 => Timestamp(x - Self::MAX.0 - 1),
            x => Timestamp(x),
        }
    }
}

impl ops::AddAssign for Timestamp {
    #[inline]
    fn add_assign(&mut self, rhs: Timestamp) {
        *self = *self + rhs;
    }
}

impl ops::Add<Duration> for Timestamp {
    type Output = Timestamp;

    #[inline]
    fn add(self, mut rhs: Duration) -> Timestamp {
        const OVER_FULL: u64 = Timestamp::MAX.0 + 1;
        // `+`がconstでないため`saturating_add`を使っているが飽和するわけではない
        const OVER_DUR: Duration = Timestamp::MAX
            .to_duration()
            .saturating_add(Timestamp(1).to_duration());

        while rhs >= OVER_DUR {
            rhs -= OVER_DUR;
        }
        let rhs = rhs.as_secs() * FULL_PER_SECS + rhs.subsec_nanos() as u64 * 27 / 1_000;

        Timestamp((self.0 + rhs) % OVER_FULL)
    }
}

impl ops::Sub for Timestamp {
    type Output = Timestamp;

    #[inline]
    fn sub(self, rhs: Timestamp) -> Timestamp {
        match self.0.checked_sub(rhs.0) {
            Some(x) => Timestamp(x),
            None => Timestamp((self.0 + Self::MAX.0 + 1) - rhs.0),
        }
    }
}

impl ops::SubAssign for Timestamp {
    #[inline]
    fn sub_assign(&mut self, rhs: Timestamp) {
        *self = *self - rhs;
    }
}

impl ops::Sub<Duration> for Timestamp {
    type Output = Timestamp;

    fn sub(self, mut rhs: Duration) -> Self::Output {
        // `+`がconstでないため`saturating_add`を使っているが飽和するわけではない
        const OVER_DUR: Duration = Timestamp::MAX
            .to_duration()
            .saturating_add(Timestamp(1).to_duration());

        while rhs >= OVER_DUR {
            rhs -= OVER_DUR;
        }
        let rhs = rhs.as_secs() * FULL_PER_SECS + rhs.subsec_nanos() as u64 * 27 / 1_000;

        match self.0.checked_sub(rhs) {
            Some(x) => Timestamp(x),
            None => Timestamp((self.0 + Self::MAX.0 + 1) - rhs),
        }
    }
}

impl fmt::Debug for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.to_duration().fmt(f)
    }
}

impl PartialOrd for Timestamp {
    #[inline]
    fn partial_cmp(&self, rhs: &Timestamp) -> Option<Ordering> {
        Some(self.cmp(rhs))
    }
}

impl Ord for Timestamp {
    fn cmp(&self, rhs: &Timestamp) -> Ordering {
        // 二値の差がWRAP_THRESH以上であればラップアラウンドしたものとし結果を逆転させる
        if self.0 > rhs.0 {
            let diff = self.0 - rhs.0;
            if diff < Self::WRAP_THRESH {
                Ordering::Greater
            } else {
                Ordering::Less
            }
        } else if self.0 < rhs.0 {
            let diff = rhs.0 - self.0;
            if diff < Self::WRAP_THRESH {
                Ordering::Less
            } else {
                Ordering::Greater
            }
        } else {
            Ordering::Equal
        }
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

        let mjd_date = MjdDate(15079);
        assert_eq!(
            mjd_date.to_date(),
            Some(Date {
                year: 1900,
                month: 3,
                day: 1,
                weekday: Weekday::Thu,
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
