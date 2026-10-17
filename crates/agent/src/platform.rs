//! OS-build diagnostics used to select version-specific native adapters.
//!
//! These predicates describe documented API floors, not end-to-end feature
//! support. Driver installation, API activation, consent, and hardware still
//! need runtime checks and physical acceptance tests.

use serde::Serialize;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct WindowsVersion {
    pub major: u32,
    pub minor: u32,
    pub build: u32,
}

impl WindowsVersion {
    /// OS-build floor documented for IddCx 1.11 runtime-compatible drivers.
    pub fn meets_iddcx_111_os_floor(&self) -> bool {
        self.major >= 10 && self.build >= 17_134
    }

    /// OS-build floor documented for HWND-targeted WGC interop.
    pub fn meets_wgc_create_for_window_os_floor(&self) -> bool {
        self.major >= 10 && self.build >= 18_362
    }

    pub fn release_label(&self) -> String {
        match (self.major, self.minor, self.build) {
            (10, _, build) if build >= 22_000 => "Windows 11".into(),
            (10, _, _) => "Windows 10".into(),
            (6, 3, _) => "Windows 8.1".into(),
            (6, 2, _) => "Windows 8".into(),
            (6, 1, _) => "Windows 7".into(),
            (6, 0, _) => "Windows Vista".into(),
            (5, 1, _) => "Windows XP".into(),
            (5, 2, _) => "Windows XP/Server 2003".into(),
            _ => format!("Windows {}.{}", self.major, self.minor),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct HostPlatform {
    pub os_name: String,
    pub version: Option<WindowsVersion>,
    pub architecture: String,
    /// These fields indicate documented build floors only, not runtime success.
    pub meets_iddcx_111_os_floor: bool,
    pub meets_wgc_create_for_window_os_floor: bool,
}

impl HostPlatform {
    pub fn detect() -> Self {
        let version = detect_windows_version();
        let os_name = version
            .as_ref()
            .map(WindowsVersion::release_label)
            .unwrap_or_else(|| std::env::consts::OS.to_owned());
        Self {
            os_name,
            meets_iddcx_111_os_floor: version
                .as_ref()
                .is_some_and(WindowsVersion::meets_iddcx_111_os_floor),
            meets_wgc_create_for_window_os_floor: version
                .as_ref()
                .is_some_and(WindowsVersion::meets_wgc_create_for_window_os_floor),
            version,
            architecture: std::env::consts::ARCH.to_owned(),
        }
    }
}

#[cfg(windows)]
fn detect_windows_version() -> Option<WindowsVersion> {
    #[repr(C)]
    struct OsVersionInfoW {
        size: u32,
        major: u32,
        minor: u32,
        build: u32,
        platform_id: u32,
        service_pack: [u16; 128],
    }

    #[link(name = "ntdll")]
    unsafe extern "system" {
        fn RtlGetVersion(version: *mut OsVersionInfoW) -> i32;
    }

    let mut info = OsVersionInfoW {
        size: std::mem::size_of::<OsVersionInfoW>() as u32,
        major: 0,
        minor: 0,
        build: 0,
        platform_id: 0,
        service_pack: [0; 128],
    };
    // RtlGetVersion returns STATUS_SUCCESS (zero) and is not subject to the
    // application-manifest version virtualization used by older APIs.
    let status = unsafe { RtlGetVersion(&mut info) };
    (status == 0).then_some(WindowsVersion {
        major: info.major,
        minor: info.minor,
        build: info.build,
    })
}

#[cfg(not(windows))]
fn detect_windows_version() -> Option<WindowsVersion> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_floor_helpers_match_documented_build_boundaries() {
        let win10_1803 = WindowsVersion {
            major: 10,
            minor: 0,
            build: 17_134,
        };
        assert!(win10_1803.meets_iddcx_111_os_floor());
        assert!(!win10_1803.meets_wgc_create_for_window_os_floor());

        let win10_1903 = WindowsVersion {
            build: 18_362,
            ..win10_1803.clone()
        };
        assert!(win10_1903.meets_iddcx_111_os_floor());
        assert!(win10_1903.meets_wgc_create_for_window_os_floor());

        let older = WindowsVersion {
            build: 7_600,
            ..win10_1803
        };
        assert!(!older.meets_iddcx_111_os_floor());
        assert!(!older.meets_wgc_create_for_window_os_floor());
    }

    #[test]
    fn release_label_distinguishes_windows_10_and_11_builds() {
        let version = WindowsVersion {
            major: 10,
            minor: 0,
            build: 22_000,
        };
        assert_eq!(version.release_label(), "Windows 11");
        assert_eq!(
            WindowsVersion {
                build: 19_041,
                ..version
            }
            .release_label(),
            "Windows 10"
        );
        assert_eq!(
            WindowsVersion {
                major: 6,
                minor: 1,
                build: 7_601,
            }
            .release_label(),
            "Windows 7"
        );
    }

    #[cfg(windows)]
    #[test]
    fn detects_the_actual_host_windows_build() {
        let host = HostPlatform::detect();
        let version = host
            .version
            .expect("RtlGetVersion must return the Windows build");
        assert_eq!(version.major, 10);
        assert!(version.build >= 17_134);
        assert!(host.architecture == "x86_64" || host.architecture == "aarch64");
    }
}
