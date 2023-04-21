/// 修正ユリウス日におけるUNIXエポック。
const UNIX_EPOCH_MJD_DAY: i64 = 40587;
/// 日本標準時とUTCの時差。
const JTC_OFFSET: i64 = 9 * 60 * 60;

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
        UnixTime(seconds - JTC_OFFSET)
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
