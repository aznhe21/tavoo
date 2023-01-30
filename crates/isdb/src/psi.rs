//! PSI用のモジュール。

use thiserror::Error;

/// [`PsiSection::parse`]で発生するエラー。
///
/// セクション長が確定したあとで発生するエラーにはセクション長が付随する。
#[derive(Debug, Error)]
pub enum PsiError {
    /// PSIセクションの長さが足りない。
    #[error("insufficient length of a PSI section")]
    InsufficientLength,

    /// PSIの終端に到達した。
    #[error("reached to end of PSI sections")]
    EndOfPsi,

    /// PSIセクションに最低限必要なバイト数がなく、壊れたセクションである。
    ///
    /// 内包する`usize`にはPSIのセクション長が入る。
    #[error("corrupt section")]
    Corrupted(usize),

    /// PSIセクションのCRC32が一致しない。
    ///
    /// 内包する`usize`にはPSIのセクション長が入る。
    #[error("crc32 error")]
    Crc32(usize),
}

/// PSIのセクション。
#[derive(Debug)]
pub struct PsiSection<'a> {
    /// table id
    pub table_id: u8,
    /// section syntax indicator
    pub section_syntax_indicator: bool,
    /// section length (12bit)
    pub section_length: u16,

    /// table syntax section
    pub syntax_section: Option<SyntaxSection>,

    /// table data
    pub data: &'a [u8],
    /// crc32
    pub crc32: u32,
}

impl<'a> PsiSection<'a> {
    /// PSIセクションをパースして[`PsiSection`]として返す。
    pub fn parse(buf: &'a [u8]) -> Result<PsiSection<'a>, PsiError> {
        if buf.len() < 3 {
            return Err(PsiError::InsufficientLength);
        }

        let table_id = buf[0];
        if table_id == 0xFF {
            return Err(PsiError::EndOfPsi);
        }
        let section_syntax_indicator = buf[1] & 0b10000000 != 0;
        let section_length = crate::utils::read_be_16(&buf[1..]) & 0b0000_1111_1111_1111;

        let Some(psi) = buf.get(..3 + section_length as usize) else {
            return Err(PsiError::InsufficientLength);
        };

        if !crate::crc32::calc(psi) {
            return Err(PsiError::Crc32(psi.len()));
        }

        let (syntax_section, data) = if section_syntax_indicator {
            if psi.len() < 3 + 4 + 5 {
                return Err(PsiError::Corrupted(psi.len()));
            }

            let table_id_extension = crate::utils::read_be_16(&psi[3..]);
            let version_number = (psi[5] & 0b00111110) >> 1;
            let current_next_indicator = psi[5] & 0b00000001 != 0;
            let section_number = psi[6];
            let last_section_number = psi[7];

            let ss = SyntaxSection {
                table_id_extension,
                version_number,
                current_next_indicator,
                section_number,
                last_section_number,
            };
            (Some(ss), &psi[8..psi.len() - 4])
        } else {
            if psi.len() < 3 + 4 {
                return Err(PsiError::Corrupted(psi.len()));
            }

            (None, &psi[3..psi.len() - 4])
        };

        let crc32 = crate::utils::read_be_32(&psi[psi.len() - 4..]);

        Ok(PsiSection {
            table_id,
            section_syntax_indicator,
            section_length,

            syntax_section,

            data,
            crc32,
        })
    }

    /// このセクション全体の長さを返す。
    #[inline]
    pub fn total_len(&self) -> usize {
        3 + self.section_length as usize
    }
}

/// table syntax section
#[derive(Debug)]
pub struct SyntaxSection {
    /// table id extension
    pub table_id_extension: u16,
    /// version number (5bit)
    pub version_number: u8,
    /// current/next indicator
    pub current_next_indicator: bool,
    /// section number
    pub section_number: u8,
    /// last section number
    pub last_section_number: u8,
}
