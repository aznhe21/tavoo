use std::io;

use windows::core::HRESULT;
use windows::Win32::Foundation::*;

const E_FILE_NOT_FOUND: HRESULT = ERROR_FILE_NOT_FOUND.to_hresult();
const E_FILE_EXISTS: HRESULT = ERROR_FILE_EXISTS.to_hresult();
const E_ALREADY_EXISTS: HRESULT = ERROR_ALREADY_EXISTS.to_hresult();
const E_INVALID_PARAMETER: HRESULT = ERROR_INVALID_PARAMETER.to_hresult();
const E_TIMEOUT: HRESULT = ERROR_TIMEOUT.to_hresult();
const E_CALL_NOT_IMPLEMENTED: HRESULT = ERROR_CALL_NOT_IMPLEMENTED.to_hresult();
const E_NOT_ENOUGH_MEMORY: HRESULT = ERROR_NOT_ENOUGH_MEMORY.to_hresult();

pub fn hr_to_io(hr: HRESULT) -> io::Error {
    match hr {
        E_FILE_NOT_FOUND => io::ErrorKind::NotFound,
        E_ACCESSDENIED => io::ErrorKind::PermissionDenied,
        E_FILE_EXISTS | E_ALREADY_EXISTS => io::ErrorKind::AlreadyExists,
        E_INVALID_PARAMETER => io::ErrorKind::InvalidInput,
        E_TIMEOUT => io::ErrorKind::TimedOut,
        E_CALL_NOT_IMPLEMENTED => io::ErrorKind::Unsupported,
        E_NOT_ENOUGH_MEMORY => io::ErrorKind::OutOfMemory,
        _ => io::ErrorKind::Other,
    }
    .into()
}

pub fn io_to_hr(e: io::Error) -> HRESULT {
    match e.kind() {
        io::ErrorKind::NotFound => E_FILE_NOT_FOUND,
        io::ErrorKind::PermissionDenied => E_ACCESSDENIED,
        io::ErrorKind::AlreadyExists => E_FILE_EXISTS,
        io::ErrorKind::InvalidInput => E_INVALID_PARAMETER,
        io::ErrorKind::TimedOut => E_TIMEOUT,
        io::ErrorKind::Unsupported => E_CALL_NOT_IMPLEMENTED,
        io::ErrorKind::OutOfMemory => E_NOT_ENOUGH_MEMORY,
        _ => E_FAIL,
    }
}
