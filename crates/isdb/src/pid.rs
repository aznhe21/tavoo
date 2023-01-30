//! PID関連。

use std::fmt;

/// MPEG2-TSのPID（13ビット）。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Pid(u16);

impl Pid {
    /// PIDの最大値。
    pub const MAX: u16 = 0x1FFF;

    /// Program Association Table
    pub const PAT: Pid = Pid::new(0x0000);
    /// Conditional Access Table
    pub const CAT: Pid = Pid::new(0x0001);
    /// Transport Stream Description Table
    pub const TSDT: Pid = Pid::new(0x0002);

    /// Network Information Table
    pub const NIT: Pid = Pid::new(0x0010);
    /// Service Description Table
    pub const SDT: Pid = Pid::new(0x0011);
    /// Bouquet Association Table
    pub const BAT: Pid = Pid::new(0x0011);
    /// Event Information Table
    pub const EIT: Pid = Pid::new(0x0012);
    /// Running Status Table
    pub const RST: Pid = Pid::new(0x0013);
    /// Time and Date Table
    pub const TDT: Pid = Pid::new(0x0014);
    /// Time Offset Table
    pub const TOT: Pid = Pid::new(0x0014);
    /// RAR Notification Table
    pub const RNT: Pid = Pid::new(0x0016);

    /// Discontinuity Information Table
    pub const DIT: Pid = Pid::new(0x001E);
    /// Selection Information Table
    pub const SIT: Pid = Pid::new(0x001F);
    /// Null packet
    pub const NULL: Pid = Pid::new(0x1FFF);

    /// `Pid`を生成する。
    ///
    /// # Panics
    ///
    /// `pid`の値が範囲外の際はパニックする。
    #[inline]
    pub const fn new(pid: u16) -> Pid {
        assert!(pid <= Pid::MAX);
        Pid(pid)
    }

    /// `pid`がPIDとして範囲内であれば`Pid`を生成する。
    #[inline]
    pub const fn new_checked(pid: u16) -> Option<Pid> {
        if pid > Pid::MAX {
            None
        } else {
            Some(Pid(pid))
        }
    }

    /// `data`からPIDを読み出す。
    ///
    /// # パニック
    ///
    /// `data`の長さが2未満の場合、このメソッドはパニックする。
    #[inline]
    pub fn read(data: &[u8]) -> Pid {
        Pid(crate::utils::read_be_16(data) & 0x1FFF)
    }

    /// PIDを`u16`で返す。
    #[inline]
    pub const fn get(&self) -> u16 {
        // Safety: `Pid`を生成できている時点で値は範囲内
        unsafe { crate::utils::assume!(self.0 <= Pid::MAX) }
        self.0
    }
}

impl Default for Pid {
    fn default() -> Self {
        Pid::NULL
    }
}

impl From<Pid> for u16 {
    fn from(value: Pid) -> Self {
        value.get()
    }
}

impl fmt::Debug for Pid {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Pid(0x{:04X})", self.0)
    }
}

macro_rules! pid_delegate_fmt {
    ($($trait:path,)*) => {
        $(
            impl $trait for Pid {
                fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                    self.0.fmt(f)
                }
            }
        )*
    };
}

pid_delegate_fmt!(
    fmt::Display,
    fmt::Binary,
    fmt::Octal,
    fmt::LowerHex,
    fmt::UpperHex,
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pid() {
        assert_eq!(Pid::new(0x1FFF), Pid::NULL);
        std::panic::catch_unwind(|| Pid::new(0x2000)).unwrap_err();
        assert_eq!(Pid::new_checked(0x1FFF), Some(Pid::NULL));
        assert_eq!(Pid::new_checked(0x2000), None);

        std::panic::catch_unwind(|| Pid::read(&[])).unwrap_err();
        std::panic::catch_unwind(|| Pid::read(&[0x00])).unwrap_err();
        assert_eq!(Pid::read(&u16::to_be_bytes(0x0000)), Pid::new(0x0000));
        assert_eq!(Pid::read(&u16::to_be_bytes(0x2000)), Pid::new(0x0000));

        assert_eq!(Pid::default(), Pid::NULL);

        assert_eq!(Pid::PAT.clone(), Pid::PAT);
        assert!(Pid::new(0x0000) < Pid::new(0x0001));
        assert!(Pid::new(0x0001) > Pid::new(0x0000));
        assert_eq!(
            [Pid::PAT, Pid::CAT, Pid::TSDT].into_iter().max(),
            Some(Pid::TSDT),
        );

        assert_eq!(Pid::PAT.get(), 0x0000);
        assert_eq!(u16::from(Pid::PAT), 0x0000);
        assert_eq!(Pid::NULL.get(), 0x1FFF);
        assert_eq!(u16::from(Pid::NULL), 0x1FFF);

        assert_eq!(format!("{}", Pid::PAT), "0");
        assert_eq!(format!("{:4}", Pid::PAT), "   0");
        assert_eq!(format!("{}", Pid::NULL), "8191");
        assert_eq!(format!("{:4}", Pid::NULL), "8191");

        assert_eq!(format!("{:b}", Pid::PAT), "0");
        assert_eq!(format!("{:13b}", Pid::PAT), "            0");
        assert_eq!(format!("{:b}", Pid::NULL), "1111111111111");
        assert_eq!(format!("{:13b}", Pid::NULL), "1111111111111");

        assert_eq!(format!("{:o}", Pid::PAT), "0");
        assert_eq!(format!("{:5o}", Pid::PAT), "    0");
        assert_eq!(format!("{:o}", Pid::NULL), "17777");
        assert_eq!(format!("{:5o}", Pid::NULL), "17777");

        assert_eq!(format!("{:x}", Pid::PAT), "0");
        assert_eq!(format!("{:4x}", Pid::PAT), "   0");
        assert_eq!(format!("{:x}", Pid::NULL), "1fff");
        assert_eq!(format!("{:4x}", Pid::NULL), "1fff");

        assert_eq!(format!("{:X}", Pid::PAT), "0");
        assert_eq!(format!("{:4X}", Pid::PAT), "   0");
        assert_eq!(format!("{:X}", Pid::NULL), "1FFF");
        assert_eq!(format!("{:4X}", Pid::NULL), "1FFF");

        assert_eq!(format!("{:?}", Pid::PAT), "Pid(0x0000)");
        assert_eq!(format!("{:?}", Pid::NULL), "Pid(0x1FFF)");
    }
}
