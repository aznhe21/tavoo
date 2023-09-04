use std::fmt;
use std::fmt::Write;
use std::ops;

use windows::core::PCWSTR;

#[derive(PartialEq, Eq, Hash)]
pub struct WideStr([u16]);

impl WideStr {
    #[inline]
    #[must_use]
    pub unsafe fn from_bytes_with_nul_unchecked(slice: &[u16]) -> &WideStr {
        unsafe { &*(slice as *const [u16] as *const WideStr) }
    }

    #[inline]
    pub const fn as_pcwstr(&self) -> PCWSTR {
        PCWSTR(self.0.as_ptr())
    }
}

impl fmt::Display for WideStr {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for c in char::decode_utf16(self.0.iter().copied().take_while(|&c| c != 0)) {
            f.write_char(c.unwrap())?;
        }
        Ok(())
    }
}

impl fmt::Debug for WideStr {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_char('"')?;
        for c in char::decode_utf16(self.0.iter().copied().take_while(|&c| c != 0)) {
            let c = c.unwrap();
            if c == '\'' {
                f.write_char('\'')?;
            } else {
                for c in c.escape_debug() {
                    f.write_char(c)?;
                }
            }
        }
        f.write_char('"')
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct WideString(Box<[u16]>);

impl WideString {
    #[inline]
    pub fn from_str(s: &str) -> WideString {
        WideString(s.encode_utf16().chain([0]).collect())
    }
}

impl From<&str> for WideString {
    #[inline]
    fn from(s: &str) -> WideString {
        WideString::from_str(s)
    }
}

impl ops::Deref for WideString {
    type Target = WideStr;

    #[inline]
    fn deref(&self) -> &WideStr {
        unsafe { WideStr::from_bytes_with_nul_unchecked(&*self.0) }
    }
}

impl fmt::Display for WideString {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        (&**self).fmt(f)
    }
}

impl fmt::Debug for WideString {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        (&**self).fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wide_string() {
        let ws = WideString::from_str("");
        assert_eq!(ws, WideString(vec![0].into_boxed_slice()));
        assert_eq!(ws.to_string(), "");
        assert_eq!(unsafe { ws.as_pcwstr().to_string() }.unwrap(), "");

        let ws = WideString::from_str("hoge");
        assert_eq!(
            ws,
            WideString(vec!['h' as u16, 'o' as u16, 'g' as u16, 'e' as u16, 0].into_boxed_slice())
        );
        assert_eq!(ws.to_string(), "hoge");
        assert_eq!(unsafe { ws.as_pcwstr().to_string() }.unwrap(), "hoge");
    }
}
