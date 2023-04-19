use std::ffi::CStr;

use android_system_properties::AndroidSystemProperties;
use once_cell::sync::OnceCell;

// From https://android.googlesource.com/platform/ndk/+/android-4.2.2_r1.2/docs/system/libc/OVERVIEW.html
// The system property named 'persist.sys.timezone' contains the name of the current timezone.

static PROPERTIES: OnceCell<AndroidSystemProperties> = OnceCell::new();

pub(crate) fn get_timezone_inner() -> Result<String, crate::GetTimezoneError> {
    PROPERTIES
        .get_or_init(AndroidSystemProperties::new)
        .get_from_cstr(unsafe { CStr::from_bytes_with_nul_unchecked(b"persist.sys.timezone\0") })
        .ok_or(crate::GetTimezoneError::OsError)
}
