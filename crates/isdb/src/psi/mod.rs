//! PSI用のモジュール。

pub mod desc;
pub mod table;

use fxhash::FxHashMap;
use thiserror::Error;

use crate::utils::BytesExt;

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
    /// テーブル識別。
    pub table_id: u8,
    /// セクションシンタクス。
    pub syntax: Option<PsiSectionSyntax>,
    /// PSIのデータ。
    pub data: &'a [u8],
    /// CRC。
    pub crc32: u32,
}

impl<'a> PsiSection<'a> {
    /// PSIセクションをパースし、[`PsiSection`]とセクション長を返す。
    pub fn parse(buf: &'a [u8]) -> Result<(PsiSection<'a>, usize), PsiError> {
        if buf.len() < 3 {
            return Err(PsiError::InsufficientLength);
        }

        let table_id = buf[0];
        if table_id == 0xFF {
            return Err(PsiError::EndOfPsi);
        }
        let section_syntax_indicator = buf[1] & 0b10000000 != 0;
        let section_length = buf[1..=2].read_be_16() & 0b0000_1111_1111_1111;

        let Some(psi) = buf.get(..3 + section_length as usize) else {
            return Err(PsiError::InsufficientLength);
        };

        if !crate::crc::calc32(psi) {
            return Err(PsiError::Crc32(psi.len()));
        }

        let (syntax, data) = if section_syntax_indicator {
            if psi.len() < 3 + 4 + 5 {
                return Err(PsiError::Corrupted(psi.len()));
            }

            let table_id_extension = psi[3..=4].read_be_16();
            let version_number = (psi[5] & 0b00111110) >> 1;
            let current_next_indicator = psi[5] & 0b00000001 != 0;
            let section_number = psi[6];
            let last_section_number = psi[7];

            let ss = PsiSectionSyntax {
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

        let crc32 = psi[psi.len() - 4..].read_be_32();

        Ok((
            PsiSection {
                table_id,
                syntax,
                data,
                crc32,
            },
            psi.len(),
        ))
    }
}

/// PSIセクションのシンタクス。
#[derive(Debug)]
pub struct PsiSectionSyntax {
    /// テーブル識別拡張。
    pub table_id_extension: u16,
    /// バージョン番号（5ビット）。
    pub version_number: u8,
    /// カレントネクスト指示。
    pub current_next_indicator: bool,
    /// セクション番号。
    pub section_number: u8,
    /// 最終セクション番号。
    pub last_section_number: u8,
}

/// PSIテーブルを表すトレイト。
pub trait PsiTable<'a>: Sized {
    /// PSIテーブルを読み取る。
    fn read(psi: &PsiSection<'a>) -> Option<Self>;
}

/// PSIテーブルのバージョン管理。
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Repository {
    // サブテーブルごとの、セクション番号と対応するバージョン番号の配列。
    subtable_versions: FxHashMap<(u8, u16), Vec<u8>>,
}

impl Repository {
    /// バージョン管理のための`Repository`を生成する。
    #[inline]
    pub fn new() -> Repository {
        Repository::default()
    }

    /// `psi`から`T`で指定したセクションを読み取る。
    ///
    /// サブテーブルのバージョンが更新されている場合や
    /// （セクションシンタクスがないために）バージョン管理が必要ない場合には
    /// `T`を使ってサブテーブルを読み込む。
    /// すなわち、`T`がバージョン管理されるべきかどうかに関わらず、
    /// サブテーブルの読み込みに`Repository`を使用して構わない。
    ///
    /// サブテーブルのバージョンが同一であり更新がない場合は`None`を返す。
    /// また`psi`から`T`を読み取れない場合やセクションシンタクスが不正な場合にも`None`を返す。
    pub fn read<'a, T: PsiTable<'a>>(&mut self, psi: &PsiSection<'a>) -> Option<T> {
        let Some(syntax) = psi.syntax.as_ref() else {
            return T::read(psi);
        };

        let len = (syntax.last_section_number + 1) as usize;
        let idx = syntax.section_number as usize;
        if idx >= len {
            return None;
        }

        let versions = self
            .subtable_versions
            .entry((psi.table_id, syntax.table_id_extension))
            .or_insert_with(Default::default);

        if versions.len() != len {
            // バージョン番号は5ビットであるため0x20以上は無効値
            versions.resize(len, 0xFF);
        }

        // Safety: 冒頭で確認済み
        unsafe { crate::utils::assume!(idx < versions.len()) }
        if versions[idx] == syntax.version_number {
            return None;
        }
        versions[idx] = syntax.version_number;

        T::read(psi)
    }

    /// `psi`が示すサブテーブルのバージョンを未読み取りとする。
    ///
    /// `read`で読み取ったテーブルがまだ処理出来る段階にない場合（PAT前のPMTなど）に、
    /// このメソッドを使用して未読み取りとすることで再度処理出来るようにする。
    pub fn unset(&mut self, psi: &PsiSection) {
        let Some(syntax) = psi.syntax.as_ref() else {
            return;
        };

        let len = (syntax.last_section_number + 1) as usize;
        let idx = syntax.section_number as usize;
        if idx >= len {
            return;
        }

        let Some(versions) = self
            .subtable_versions
            .get_mut(&(psi.table_id, syntax.table_id_extension))
        else {
            return;
        };

        if idx <= versions.len() {
            return;
        }

        // バージョン番号は5ビットであるため0x20以上は無効値
        versions[idx] = 0xFF;
    }

    /// `Repository`の内容を消去して初期化する。
    #[inline]
    pub fn clear(&mut self) {
        self.subtable_versions.clear();
    }
}
