use std::time::Duration;

/// 修正ユリウス日におけるUNIXエポック。
const UNIX_EPOCH_MJD_DAY: i64 = 40587;
/// 日本標準時とUTCの時差。
const JTC_OFFSET: Duration = Duration::from_secs(9 * 60 * 60);
/// NTPのエポックとUNIXエポックの時差。
const NTP_TO_UNIX: Duration = Duration::from_secs(2_208_988_800);

/// UTCのUNIX時間をシリアライズするためのオブジェクト。
///
/// 値表現は符号付き64ビット整数であるため、紀元前後3000億年程度まで格納できる。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
pub struct UnixTime(pub i64);

impl From<isdb::time::DateTime> for UnixTime {
    fn from(dt: isdb::time::DateTime) -> UnixTime {
        let days = dt.date.0 as i64 - UNIX_EPOCH_MJD_DAY;
        let hours = days * 24 + dt.hour as i64;
        let minutes = hours * 60 + dt.minute as i64;
        let seconds = minutes * 60 + dt.second as i64;
        UnixTime(seconds - JTC_OFFSET.as_secs() as i64)
    }
}

/// 1900年1月1日からの経過時刻を格納し、ミリ秒単位のUNIX時間としてシリアライズされるタイムスタンプ。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Timestamp(pub Duration);

impl serde::Serialize for Timestamp {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_f64((self.0 - NTP_TO_UNIX - JTC_OFFSET).as_secs_f64() * 1000.)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unix_time() {
        assert_eq!(
            UnixTime::from(isdb::time::DateTime {
                // 1900/03/01
                date: isdb::time::MjdDate(15079),
                hour: 9,
                minute: 0,
                second: 0,
            }),
            UnixTime(-2203891200)
        );

        assert_eq!(
            UnixTime::from(isdb::time::DateTime {
                // 1970/01/01
                date: isdb::time::MjdDate(40587),
                hour: 9,
                minute: 0,
                second: 0,
            }),
            UnixTime(0)
        );

        assert_eq!(
            UnixTime::from(isdb::time::DateTime {
                // 1982/09/06
                date: isdb::time::MjdDate(0xB0A2),
                hour: 12,
                minute: 34,
                second: 56,
            }),
            UnixTime(400131296)
        );

        assert_eq!(
            serde_json::to_value(&UnixTime(1234567890)).unwrap(),
            serde_json::json!(1234567890),
        );
    }
}
