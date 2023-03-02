//! PID関連。

use std::fmt;
use std::ops;

use crate::utils::BytesExt;

/// MPEG2-TSのPID（13ビット）。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Pid(u16);

// 定数のほとんどはARIB STD-B10による。
impl Pid {
    /// PIDの最大値。
    pub const MAX: u16 = 0x1FFF;

    /// プログラムアソシエーションテーブル（Program Association Table）。
    pub const PAT: Pid = Pid::new(0x0000);
    /// 限定受信テーブル（Conditional Access Table）。
    pub const CAT: Pid = Pid::new(0x0001);

    /// ネットワーク情報テーブル（Network Information Table）。
    pub const NIT: Pid = Pid::new(0x0010);
    /// サービス記述テーブル（Service Description Table）。
    pub const SDT: Pid = Pid::new(0x0011);
    /// ブーケアソシエーションテーブル（Bouquet Association Table）。
    pub const BAT: Pid = Pid::new(0x0011);
    /// イベント情報テーブル（Event Information Table）。
    pub const EIT: Pid = Pid::new(0x0012);
    /// 固定受信機での表示を目的としてEITの総称。
    // ARIB TR-B14より。
    pub const H_EIT: Pid = Self::EIT;
    /// 進行状態テーブル（Running Status Table）。
    pub const RST: Pid = Pid::new(0x0013);
    /// 時刻日付テーブル（Time and Date Table）。
    pub const TDT: Pid = Pid::new(0x0014);
    /// 時刻日付オフセットテーブル（Time Offset Table）。
    pub const TOT: Pid = Pid::new(0x0014);

    /// 不連続情報テーブル（Discontinuity Information Table）。
    pub const DIT: Pid = Pid::new(0x001E);
    /// 選択情報テーブル（Selection Information Table）。
    pub const SIT: Pid = Pid::new(0x001F);
    /// 差分配信告知テーブル（Partial Content Announcement Table）。
    pub const PCAT: Pid = Pid::new(0x0022);
    /// ソフトウェアダウンロードトリガーテーブル（Software Download Trigger Table）。
    pub const SDTT: Pid = Pid::new(0x0023);
    /// ブロードキャスタ情報テーブル（Broadcaster Information Table）。
    pub const BIT: Pid = Pid::new(0x0024);
    /// ネットワーク掲示板情報テーブル（Network Board Information Table）。
    pub const NBIT: Pid = Pid::new(0x0025);
    /// リンク記述テーブル（Linked Description Table）。
    pub const LDT: Pid = Pid::new(0x0025);
    /// 3セグメント受信機での表示を目的としたEITの総称。
    // ARIB TR-B14より。
    pub const M_EIT: Pid = Pid::new(0x0026);
    /// 1セグメント受信機での表示を目的としたEITの総称。
    // ARIB TR-B14より。
    pub const L_EIT: Pid = Pid::new(0x0027);
    /// 全受信機共通データテーブル（Common Data Table）。
    pub const CDT: Pid = Pid::new(0x0029);
    /// ヌルパケット（Null packet）。
    pub const NULL: Pid = Pid::new(0x1FFF);

    /// `Pid`を生成する。
    ///
    /// # パニック
    ///
    /// `pid`の値が範囲外の際はパニックする。
    #[inline]
    pub const fn new(pid: u16) -> Pid {
        assert!(pid <= Pid::MAX);
        Pid(pid)
    }

    /// `pid`がPIDとして範囲内であれば`Pid`を生成する。
    #[inline]
    pub const fn try_new(pid: u16) -> Option<Pid> {
        if pid > Pid::MAX {
            None
        } else {
            Some(Pid(pid))
        }
    }

    /// `Pid`を生成する。
    ///
    /// # 安全性
    ///
    /// `pid`がPIDとして範囲外の場合、動作は未定義である。
    #[inline]
    pub const unsafe fn new_unchecked(pid: u16) -> Pid {
        Pid(pid)
    }

    /// `data`からPIDを読み出す。
    ///
    /// # パニック
    ///
    /// `data`の長さが2未満の場合、このメソッドはパニックする。
    #[inline]
    pub fn read(data: &[u8]) -> Pid {
        Pid(data[0..=1].read_be_16() & 0x1FFF)
    }

    /// PIDを`u16`で返す。
    #[inline]
    pub const fn get(&self) -> u16 {
        // Safety: `Pid`を生成できている時点で値は範囲内
        unsafe { crate::utils::assume!(self.0 <= Pid::MAX) }
        self.0
    }

    /// このPIDがワンセグのPMTかどうかを返す。
    #[inline]
    pub const fn is_oneseg_pmt(&self) -> bool {
        matches!(self.0, 0x1FC8..=0x1FCF)
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

crate::utils::delegate_fmt!(Pid);

/// [`Pid`]をキーにして値`V`にアクセスができるテーブル。
///
/// データはヒープに確保される。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PidTable<V>(Box<[V; Pid::MAX as usize + 1]>);

impl<V> PidTable<V> {
    /// `f`を呼び出した戻り値から`PidTable`を生成する。
    #[inline]
    pub fn from_fn<F: FnMut(Pid) -> V>(mut f: F) -> PidTable<V> {
        // Safety: iはPIDの範囲である
        let table = unsafe { crate::utils::boxed_array(|i| f(Pid::new_unchecked(i as u16))) };
        PidTable(table)
    }

    /// 内部の配列を返す。
    #[inline]
    pub fn into_inner(self) -> Box<[V; Pid::MAX as usize + 1]> {
        self.0
    }

    /// テーブルを回すイテレーターを返す。
    #[inline]
    pub fn iter(&self) -> std::slice::Iter<V> {
        self.0.iter()
    }

    /// テーブルを可変で回すイテレーターを返す。
    #[inline]
    pub fn iter_mut(&mut self) -> std::slice::IterMut<V> {
        self.0.iter_mut()
    }
}

impl<V> From<Box<[V; Pid::MAX as usize + 1]>> for PidTable<V> {
    #[inline]
    fn from(table: Box<[V; Pid::MAX as usize + 1]>) -> Self {
        PidTable(table)
    }
}

impl<V> From<PidTable<V>> for Box<[V; Pid::MAX as usize + 1]> {
    #[inline]
    fn from(table: PidTable<V>) -> Self {
        table.0
    }
}

impl<V> IntoIterator for PidTable<V> {
    type Item = V;
    type IntoIter = std::array::IntoIter<V, { Pid::MAX as usize + 1 }>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a, V> IntoIterator for &'a PidTable<V> {
    type Item = &'a V;
    type IntoIter = std::slice::Iter<'a, V>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, V> IntoIterator for &'a mut PidTable<V> {
    type Item = &'a mut V;
    type IntoIter = std::slice::IterMut<'a, V>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl<V> ops::Index<Pid> for PidTable<V> {
    type Output = V;

    #[inline]
    fn index(&self, pid: Pid) -> &Self::Output {
        &self.0[pid.get() as usize]
    }
}

impl<V> ops::IndexMut<Pid> for PidTable<V> {
    #[inline]
    fn index_mut(&mut self, pid: Pid) -> &mut Self::Output {
        &mut self.0[pid.get() as usize]
    }
}

impl<V> AsRef<[V]> for PidTable<V> {
    #[inline]
    fn as_ref(&self) -> &[V] {
        &*self.0
    }
}

impl<V> AsMut<[V]> for PidTable<V> {
    #[inline]
    fn as_mut(&mut self) -> &mut [V] {
        &mut *self.0
    }
}

impl<V> ops::Deref for PidTable<V> {
    type Target = [V; Pid::MAX as usize + 1];

    #[inline]
    fn deref(&self) -> &Self::Target {
        &*self.0
    }
}

impl<V> ops::DerefMut for PidTable<V> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut *self.0
    }
}

impl<V> std::borrow::Borrow<[V]> for PidTable<V> {
    #[inline]
    fn borrow(&self) -> &[V] {
        &*self.0
    }
}

impl<V> std::borrow::BorrowMut<[V]> for PidTable<V> {
    #[inline]
    fn borrow_mut(&mut self) -> &mut [V] {
        &mut *self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pid() {
        assert_eq!(Pid::new(0x1FFF), Pid::NULL);
        std::panic::catch_unwind(|| Pid::new(0x2000)).unwrap_err();
        assert_eq!(Pid::try_new(0x1FFF), Some(Pid::NULL));
        assert_eq!(Pid::try_new(0x2000), None);

        std::panic::catch_unwind(|| Pid::read(&[])).unwrap_err();
        std::panic::catch_unwind(|| Pid::read(&[0x00])).unwrap_err();
        assert_eq!(Pid::read(&u16::to_be_bytes(0x0000)), Pid::new(0x0000));
        assert_eq!(Pid::read(&u16::to_be_bytes(0x2000)), Pid::new(0x0000));

        assert_eq!(Pid::default(), Pid::NULL);

        assert_eq!(Pid::PAT.clone(), Pid::PAT);
        assert!(Pid::new(0x0000) < Pid::new(0x0001));
        assert!(Pid::new(0x0001) > Pid::new(0x0000));
        assert_eq!(
            [Pid::PAT, Pid::CAT, Pid::NIT].into_iter().max(),
            Some(Pid::NIT),
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

    #[test]
    fn test_pid_table() {
        let table = PidTable::from_fn(|i| i);
        assert_eq!(table[Pid::PAT], Pid::PAT);
        assert_eq!(
            table.clone().into_inner(),
            crate::utils::boxed_array(|i| Pid::new(i as u16)),
        );
        assert_eq!(
            Box::<[Pid; Pid::MAX as usize + 1]>::from(table.clone()),
            crate::utils::boxed_array(|i| Pid::new(i as u16)),
        );
        assert_eq!(
            PidTable::<_>::from(crate::utils::boxed_array(|i| Pid::new(i as u16))),
            table,
        );

        let slice: &[Pid; Pid::MAX as usize + 1] = &*table;
        assert_eq!(slice, &*(0..=Pid::MAX).map(Pid::new).collect::<Vec<Pid>>());

        assert_eq!(table.iter().find(|pid| pid.get() >= 0x10), Some(&Pid::NIT));

        let mut table2 = table.clone();
        for pid in table2.iter_mut() {
            *pid = Pid::PAT;
        }
        assert_eq!(table2, PidTable::from_fn(|_| Pid::PAT));
        for pid in 0..=Pid::MAX {
            table2[Pid::new(pid)] = Pid::CAT;
        }
        assert_eq!(table2, PidTable::from_fn(|_| Pid::CAT));

        assert!(table
            .clone()
            .into_iter()
            .enumerate()
            .all(|(i, pid)| i == pid.get() as usize));
    }
}
