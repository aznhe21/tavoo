//! 固定長のリングバッファ。

use std::fmt;
use std::iter;
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::ptr;

/// 最低限の機能だけ実装することで最適化したリングバッファ。
///
/// 要素は最大でも`CAP`しか保持せず、それ以上押し込もうとすると古い要素から消される。
pub struct RingBuf<T, const CAP: usize> {
    ptr: *mut T,
    len: usize,
    first: usize,
    _marker: PhantomData<Box<[MaybeUninit<T>; CAP]>>,
}

impl<T, const CAP: usize> RingBuf<T, CAP> {
    /// 空の`RingBuf`を生成する。
    #[inline]
    pub fn new() -> RingBuf<T, CAP> {
        let mut buf = Vec::<T>::with_capacity(CAP);
        unsafe { buf.set_len(CAP) };
        RingBuf {
            ptr: Box::into_raw(buf.into_boxed_slice()) as *mut T,
            len: 0,
            first: 0,
            _marker: PhantomData,
        }
    }

    /// リングバッファの要素数を返す。
    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    /// リングバッファに要素がない場合に`true`を返す。
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// 新しい要素を押し込む。
    ///
    /// 既に`CAP`分の要素を保持している場合、最も古い要素が消される。
    pub fn push(&mut self, value: T) {
        if self.len < CAP {
            // Safety: self.lenはバッファの範囲内であり、かつwriteにより未初期化値はDropされない
            unsafe {
                self.ptr.add(self.len).write(value);
            }
            self.len += 1;
        } else {
            // 上書き
            debug_assert!(self.first < CAP);
            // Safety: self.firstはバッファの範囲内であり、かつそこの要素は書き込み済み
            unsafe {
                *self.ptr.add(self.first) = value;
            }

            self.first += 1;
            if self.first == CAP {
                self.first = 0;
            }
        }
    }

    fn as_mut_slices(&mut self) -> (&mut [T], &mut [T]) {
        debug_assert!(self.first < CAP);
        // Safety: self.firstはバッファの範囲内
        unsafe {
            (
                &mut *ptr::slice_from_raw_parts_mut(
                    self.ptr.add(self.first),
                    self.len - self.first,
                ),
                &mut *ptr::slice_from_raw_parts_mut(self.ptr, self.first),
            )
        }
    }

    /// 内容を消去する。
    pub fn clear(&mut self) {
        struct Dropper<T>(*mut [T]);
        impl<T> Drop for Dropper<T> {
            fn drop(&mut self) {
                unsafe {
                    ptr::drop_in_place(self.0);
                }
            }
        }

        let (front, back) = self.as_mut_slices();
        // Safety: frontとbackのどちらも有効かつ重複のないスライス
        unsafe {
            let drop_back = back as *mut _;
            let drop_front = front as *mut _;
            self.len = 0;
            self.first = 0;

            let _back_dropper = Dropper(drop_back);
            ptr::drop_in_place(drop_front);
        }
    }

    /// `RingBuf`用イテレータを生成する。
    #[inline]
    pub fn iter(&self) -> Iter<T, CAP> {
        Iter {
            ptr: self.ptr,
            len: self.len,
            first: self.first,
            _marker: PhantomData,
        }
    }
}

unsafe impl<T, const CAP: usize> Send for RingBuf<T, CAP> {}

impl<T, const CAP: usize> Drop for RingBuf<T, CAP> {
    fn drop(&mut self) {
        struct Dropper<T>(*mut [T]);
        impl<T> Drop for Dropper<T> {
            fn drop(&mut self) {
                unsafe {
                    ptr::drop_in_place(self.0);
                }
            }
        }

        let (front, back) = self.as_mut_slices();
        // Safety: frontとbackのどちらも有効かつ重複のないポインタ
        unsafe {
            let _back_dropper = Dropper(back);
            ptr::drop_in_place(front);
        }

        // Safety: self.ptrはBox<[MaybeUninit<T>; CAP]>と同等
        unsafe {
            let _ = Box::from_raw(ptr::slice_from_raw_parts_mut(
                self.ptr as *mut MaybeUninit<T>,
                CAP,
            ));
        }
    }
}

impl<T, const CAP: usize> Default for RingBuf<T, CAP> {
    #[inline]
    fn default() -> Self {
        RingBuf::new()
    }
}

impl<T: fmt::Debug, const CAP: usize> fmt::Debug for RingBuf<T, CAP> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_list().entries(self).finish()
    }
}

impl<'a, T, const CAP: usize> IntoIterator for &'a RingBuf<T, CAP> {
    type Item = &'a T;
    type IntoIter = Iter<'a, T, CAP>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// [`RingBuf`]用のイテレータ。
pub struct Iter<'a, T, const CAP: usize> {
    ptr: *mut T,
    len: usize,
    first: usize,
    _marker: PhantomData<&'a mut [T; CAP]>,
}

impl<'a, T, const CAP: usize> Iterator for Iter<'a, T, CAP> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.len == 0 {
            None
        } else {
            debug_assert!(self.first < CAP);
            // Safety: self.firstはバッファの範囲内であり、かつ書き込み済み
            let v = unsafe { &*self.ptr.add(self.first) };
            self.len -= 1;
            self.first += 1;
            if self.first == CAP {
                self.first = 0;
            }

            Some(v)
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }

    #[inline]
    fn count(self) -> usize {
        self.len
    }
}

impl<'a, T, const CAP: usize> ExactSizeIterator for Iter<'a, T, CAP> {
    #[inline]
    fn len(&self) -> usize {
        self.len
    }
}

impl<'a, T, const CAP: usize> iter::FusedIterator for Iter<'a, T, CAP> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[inline]
    fn arr<T>(arr: &mut [T]) -> &mut [T] {
        arr
    }

    #[test]
    fn test_ring_buf() {
        let mut buf = RingBuf::<u32, 4>::new();
        assert!(buf.is_empty());
        assert_eq!(buf.len(), 0);
        let mut iter = buf.iter();
        assert_eq!(iter.len(), 0);
        assert_eq!(iter.next(), None);
        assert_eq!(iter.len(), 0);
        assert_eq!(iter.next(), None);
        assert_eq!(buf.as_mut_slices(), (arr(&mut []), arr(&mut [])));

        buf.push(0);
        buf.push(1);
        buf.push(2);
        buf.push(3);
        assert_eq!(buf.len(), 4);
        let mut iter = buf.iter();
        assert_eq!(iter.len(), 4);
        assert_eq!(iter.next(), Some(&0));
        assert_eq!(iter.len(), 3);
        assert_eq!(iter.next(), Some(&1));
        assert_eq!(iter.len(), 2);
        assert_eq!(iter.next(), Some(&2));
        assert_eq!(iter.len(), 1);
        assert_eq!(iter.next(), Some(&3));
        assert_eq!(iter.len(), 0);
        assert_eq!(iter.next(), None);
        assert_eq!(iter.len(), 0);
        assert_eq!(iter.next(), None);
        assert_eq!(buf.as_mut_slices(), (arr(&mut [0, 1, 2, 3]), arr(&mut [])));

        // 上書き
        buf.push(4);
        assert_eq!(buf.len(), 4);
        let mut iter = buf.iter();
        assert_eq!(iter.len(), 4);
        assert_eq!(iter.next(), Some(&1));
        assert_eq!(iter.len(), 3);
        assert_eq!(iter.next(), Some(&2));
        assert_eq!(iter.len(), 2);
        assert_eq!(iter.next(), Some(&3));
        assert_eq!(iter.len(), 1);
        assert_eq!(iter.next(), Some(&4));
        assert_eq!(iter.len(), 0);
        assert_eq!(iter.next(), None);
        assert_eq!(iter.len(), 0);
        assert_eq!(iter.next(), None);
        assert_eq!(buf.as_mut_slices(), (arr(&mut [1, 2, 3]), arr(&mut [4])));

        buf.clear();
        assert!(buf.is_empty());
        assert_eq!(buf.len(), 0);
        let mut iter = buf.iter();
        assert_eq!(iter.len(), 0);
        assert_eq!(iter.next(), None);
        assert_eq!(iter.len(), 0);
        assert_eq!(iter.next(), None);
        assert_eq!(buf.as_mut_slices(), (arr(&mut []), arr(&mut [])));
    }

    #[test]
    fn test_ring_buf_drop() {
        use parking_lot::Mutex;

        static DROPPED: Mutex<Vec<u32>> = Mutex::new(Vec::new());
        struct Collector(u32);
        impl Drop for Collector {
            fn drop(&mut self) {
                DROPPED.lock().push(self.0);
            }
        }

        DROPPED.lock().clear();
        let _ = RingBuf::<Collector, 1>::new();
        assert_eq!(*DROPPED.lock(), vec![]);

        // 普通のDrop
        DROPPED.lock().clear();
        let mut buf = RingBuf::<Collector, 1>::new();
        buf.push(Collector(0));
        assert_eq!(*DROPPED.lock(), vec![]);
        drop(buf);
        assert_eq!(*DROPPED.lock(), vec![0]);

        // 上書き
        DROPPED.lock().clear();
        let mut buf = RingBuf::<Collector, 1>::new();
        buf.push(Collector(0));
        assert_eq!(*DROPPED.lock(), vec![]);
        buf.push(Collector(1));
        assert_eq!(*DROPPED.lock(), vec![0]);
        drop(buf);
        assert_eq!(*DROPPED.lock(), vec![0, 1]);

        // clear
        DROPPED.lock().clear();
        let mut buf = RingBuf::<Collector, 1>::new();
        buf.push(Collector(0));
        assert_eq!(*DROPPED.lock(), vec![]);
        buf.clear();
        assert_eq!(*DROPPED.lock(), vec![0]);
        drop(buf);
        assert_eq!(*DROPPED.lock(), vec![0]);

        // 上書き+clear
        DROPPED.lock().clear();
        let mut buf = RingBuf::<Collector, 2>::new();
        buf.push(Collector(0));
        buf.push(Collector(1));
        buf.push(Collector(2));
        assert_eq!(*DROPPED.lock(), vec![0]);
        buf.clear();
        assert_eq!(*DROPPED.lock(), vec![0, 1, 2]);
        drop(buf);
        assert_eq!(*DROPPED.lock(), vec![0, 1, 2]);
    }
}
