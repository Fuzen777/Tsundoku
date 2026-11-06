//! Facts about the PC, for Copy Details' bug reports.

/// Windows' name, edition, release and build, such as "Windows 11 Home 24H2 (build 26100.4061)", read from the
/// registry, where Windows keeps them for Settings → System → About.
#[cfg(windows)]
pub fn windows_version() -> String {
    const KEY: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";
    describe_windows(
        registry::text(KEY, "ProductName").as_deref(),
        registry::text(KEY, "DisplayVersion").as_deref(),
        registry::text(KEY, "CurrentBuild").as_deref(),
        registry::number(KEY, "UBR"),
    )
}

#[cfg(not(windows))]
pub fn windows_version() -> String {
    std::env::consts::OS.to_owned()
}

/// The first build of Windows 11, whose registry still calls it Windows 10.
const FIRST_WINDOWS_11_BUILD: u32 = 22000;

fn describe_windows(
    product: Option<&str>,
    release: Option<&str>,
    build: Option<&str>,
    revision: Option<u32>,
) -> String {
    let build = build.map(str::trim).filter(|build| !build.is_empty());
    let mut name = product
        .map(str::trim)
        .filter(|product| !product.is_empty())
        .unwrap_or("Windows")
        .to_owned();
    let windows_11 = build
        .and_then(|build| build.parse::<u32>().ok())
        .is_some_and(|build| build >= FIRST_WINDOWS_11_BUILD);
    if windows_11 {
        name = name.replacen("Windows 10", "Windows 11", 1);
    }
    if let Some(release) = release.map(str::trim).filter(|release| !release.is_empty()) {
        name = format!("{name} {release}");
    }
    match (build, revision) {
        (Some(build), Some(revision)) => format!("{name} (build {build}.{revision})"),
        (Some(build), None) => format!("{name} (build {build})"),
        (None, _) => name,
    }
}

#[cfg(windows)]
mod registry {
    use std::ptr;

    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::System::Registry::{
        HKEY_LOCAL_MACHINE, RRF_RT_REG_DWORD, RRF_RT_REG_SZ, RegGetValueW,
    };

    /// A text value under `HKEY_LOCAL_MACHINE`.
    pub fn text(key: &str, name: &str) -> Option<String> {
        let data = value(key, name, RRF_RT_REG_SZ)?;
        let characters: Vec<u16> = data
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_le_bytes(*pair))
            .take_while(|&character| character != 0)
            .collect();
        Some(String::from_utf16_lossy(&characters))
    }

    /// A number (DWORD) value under `HKEY_LOCAL_MACHINE`.
    pub fn number(key: &str, name: &str) -> Option<u32> {
        let data = value(key, name, RRF_RT_REG_DWORD)?;
        Some(u32::from_le_bytes(data.get(..4)?.try_into().ok()?))
    }

    fn value(key: &str, name: &str, kind: u32) -> Option<Vec<u8>> {
        let key = wide(key);
        let name = wide(name);
        let mut size = 0u32;
        // SAFETY: the key and name are NUL-terminated and live through both calls; the first only reads the size, and
        // the second writes at most `size` bytes into `data`, which has that many.
        unsafe {
            let status = RegGetValueW(
                HKEY_LOCAL_MACHINE,
                key.as_ptr(),
                name.as_ptr(),
                kind,
                ptr::null_mut(),
                ptr::null_mut(),
                &mut size,
            );
            if status != ERROR_SUCCESS {
                return None;
            }
            let mut data = vec![0u8; size as usize];
            let status = RegGetValueW(
                HKEY_LOCAL_MACHINE,
                key.as_ptr(),
                name.as_ptr(),
                kind,
                ptr::null_mut(),
                data.as_mut_ptr().cast(),
                &mut size,
            );
            (status == ERROR_SUCCESS).then(|| {
                data.truncate(size as usize);
                data
            })
        }
    }

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain([0]).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_windows_11_though_the_registry_says_10() {
        assert_eq!(
            describe_windows(
                Some("Windows 10 Home"),
                Some("24H2"),
                Some("26100"),
                Some(4061)
            ),
            "Windows 11 Home 24H2 (build 26100.4061)"
        );
        assert_eq!(
            describe_windows(Some("Windows 10 Pro"), Some("22H2"), Some("19045"), None),
            "Windows 10 Pro 22H2 (build 19045)"
        );
        assert_eq!(describe_windows(None, None, None, None), "Windows");
    }

    #[cfg(windows)]
    #[test]
    fn reads_this_pcs_windows() {
        let version = windows_version();
        assert!(version.starts_with("Windows"), "{version}");
        assert!(version.contains("(build "), "{version}");
    }
}
