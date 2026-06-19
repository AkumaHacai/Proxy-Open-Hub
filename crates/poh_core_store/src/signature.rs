//! Authenticode (code-signing) verification for installed core executables.
//!
//! This is the second integrity layer on top of the SHA-256 pinning the store
//! already does: SHA-256 proves "these are exactly the bytes we pinned", while
//! Authenticode proves "these bytes carry a valid publisher signature trusted by
//! Windows". It is required before any automatic install/update UI is exposed
//! (see `LLM_Cloud/todone_arh.md`, Workstream 5).
//!
//! On non-Windows builds (CI/dev) verification is unavailable and always returns
//! [`SignatureStatus::Unknown`]; callers must treat `Unknown` as "not verified".

use std::path::Path;

use poh_core::SignatureStatus;

/// Verify the Authenticode signature embedded in `path`.
///
/// - [`SignatureStatus::Verified`] - the file is signed and the whole chain is
///   trusted by the OS trust store.
/// - [`SignatureStatus::Unsigned`] - the file has no embedded signature.
/// - [`SignatureStatus::Unknown`] - signature present but untrusted/expired/
///   tampered, the subject is not a recognised form, or verification is not
///   available on this platform. Never treat `Unknown` as trusted.
pub fn authenticode_status(path: &Path) -> SignatureStatus {
    #[cfg(windows)]
    {
        windows_impl::authenticode_status(path)
    }

    #[cfg(not(windows))]
    {
        let _ = path;
        SignatureStatus::Unknown
    }
}

#[cfg(windows)]
mod windows_impl {
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;
    use std::ptr;

    use poh_core::SignatureStatus;
    use windows_sys::core::GUID;
    use windows_sys::Win32::Security::WinTrust::{
        WinVerifyTrust, WINTRUST_DATA, WINTRUST_FILE_INFO, WTD_CHOICE_FILE, WTD_REVOKE_NONE,
        WTD_STATEACTION_CLOSE, WTD_STATEACTION_VERIFY, WTD_UI_NONE,
    };

    // WINTRUST_ACTION_GENERIC_VERIFY_V2 - {00AAC56B-CD44-11d0-8CC2-00C04FC295EE}.
    const GENERIC_VERIFY_V2: GUID = GUID {
        data1: 0x00AA_C56B,
        data2: 0xCD44,
        data3: 0x11D0,
        data4: [0x8C, 0xC2, 0x00, 0xC0, 0x4F, 0xC2, 0x95, 0xEE],
    };

    // HRESULT returned by WinVerifyTrust when the file carries no signature.
    const TRUST_E_NOSIGNATURE: i32 = 0x800B_0100_u32 as i32;

    pub(super) fn authenticode_status(path: &Path) -> SignatureStatus {
        let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
        wide.push(0);

        let mut file_info: WINTRUST_FILE_INFO = unsafe { std::mem::zeroed() };
        file_info.cbStruct = std::mem::size_of::<WINTRUST_FILE_INFO>() as u32;
        file_info.pcwszFilePath = wide.as_ptr();
        file_info.hFile = ptr::null_mut();
        file_info.pgKnownSubject = ptr::null_mut();

        let mut data: WINTRUST_DATA = unsafe { std::mem::zeroed() };
        data.cbStruct = std::mem::size_of::<WINTRUST_DATA>() as u32;
        data.dwUIChoice = WTD_UI_NONE;
        data.fdwRevocationChecks = WTD_REVOKE_NONE;
        data.dwUnionChoice = WTD_CHOICE_FILE;
        data.Anonymous.pFile = &mut file_info;
        data.dwStateAction = WTD_STATEACTION_VERIFY;

        let mut action = GENERIC_VERIFY_V2;
        let result = unsafe {
            WinVerifyTrust(
                ptr::null_mut(),
                &mut action,
                &mut data as *mut WINTRUST_DATA as *mut core::ffi::c_void,
            )
        };

        // Always release the state handle the VERIFY action allocated.
        data.dwStateAction = WTD_STATEACTION_CLOSE;
        unsafe {
            WinVerifyTrust(
                ptr::null_mut(),
                &mut action,
                &mut data as *mut WINTRUST_DATA as *mut core::ffi::c_void,
            );
        }

        match result {
            0 => SignatureStatus::Verified,
            TRUST_E_NOSIGNATURE => SignatureStatus::Unsigned,
            // Signed-but-untrusted, expired, tampered, or unrecognised subject.
            // Conservatively "not verified".
            _ => SignatureStatus::Unknown,
        }
    }
}
