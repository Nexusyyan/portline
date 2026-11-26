use anyhow::Result;
use std::path::Path;

/// Command-line flag appended to the OS login startup entry so Portline can start
/// silently minimized to the system tray and auto-start the proxy core cleanly.
pub const AUTOSTART_ARG: &str = "--autostart";

/// Formats the quoted startup command line for the given executable path.
pub fn launch_command_for_exe(exe_path: &Path) -> String {
    format!("\"{}\" {}", exe_path.display(), AUTOSTART_ARG)
}

/// Returns whether Portline is currently registered to launch at user login.
pub fn is_autostart_enabled() -> bool {
    platform::is_enabled()
}

/// Registers or removes the OS user-level login startup entry for the current executable.
/// Uses current-user mechanisms only (no administrator elevation required).
pub fn set_autostart_enabled(enabled: bool) -> Result<()> {
    if enabled {
        let exe = std::env::current_exe()?;
        platform::enable(&exe)
    } else {
        platform::disable()
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use super::*;
    use anyhow::anyhow;
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::ERROR_FILE_NOT_FOUND;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW,
        RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE,
        REG_OPTION_NON_VOLATILE, REG_SZ, REG_VALUE_TYPE,
    };

    const RUN_SUBKEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
    const VALUE_NAME: &str = "Portline";

    fn to_wide_null(s: &str) -> Vec<u16> {
        OsStr::new(s).encode_wide().chain(Some(0)).collect()
    }

    pub fn is_enabled() -> bool {
        let subkey = to_wide_null(RUN_SUBKEY);
        let value_name = to_wide_null(VALUE_NAME);
        let mut hkey = HKEY::default();
        unsafe {
            if RegOpenKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(subkey.as_ptr()),
                0,
                KEY_READ,
                &mut hkey,
            )
            .is_err()
            {
                return false;
            }
            let mut size: u32 = 0;
            let mut ty = REG_VALUE_TYPE(0);
            let exists = RegQueryValueExW(
                hkey,
                PCWSTR(value_name.as_ptr()),
                None,
                Some(&mut ty),
                None,
                Some(&mut size),
            )
            .is_ok()
                && size > 2;
            let _ = RegCloseKey(hkey);
            exists
        }
    }

    pub fn enable(exe_path: &Path) -> Result<()> {
        let subkey = to_wide_null(RUN_SUBKEY);
        let value_name = to_wide_null(VALUE_NAME);
        let cmd = launch_command_for_exe(exe_path);
        let wide_cmd = to_wide_null(&cmd);
        let byte_len = (wide_cmd.len() * std::mem::size_of::<u16>()) as u32;

        let mut hkey = HKEY::default();
        unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(subkey.as_ptr()),
                0,
                None,
                REG_OPTION_NON_VOLATILE,
                KEY_SET_VALUE,
                None,
                &mut hkey,
                None,
            )
            .ok()
            .map_err(|e| anyhow!("Failed to open Windows Run key: {}", e))?;

            let slice =
                std::slice::from_raw_parts(wide_cmd.as_ptr() as *const u8, byte_len as usize);
            let status = RegSetValueExW(
                hkey,
                PCWSTR(value_name.as_ptr()),
                0,
                REG_SZ,
                Some(slice),
            );
            let _ = RegCloseKey(hkey);
            status
                .ok()
                .map_err(|e| anyhow!("Failed to write Windows Run value: {}", e))
        }
    }

    pub fn disable() -> Result<()> {
        let subkey = to_wide_null(RUN_SUBKEY);
        let value_name = to_wide_null(VALUE_NAME);
        let mut hkey = HKEY::default();
        unsafe {
            let open_res = RegOpenKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(subkey.as_ptr()),
                0,
                KEY_SET_VALUE,
                &mut hkey,
            );
            if open_res == ERROR_FILE_NOT_FOUND {
                return Ok(());
            }
            open_res
                .ok()
                .map_err(|e| anyhow!("Failed to open Windows Run key: {}", e))?;

            let status = RegDeleteValueW(hkey, PCWSTR(value_name.as_ptr()));
            let _ = RegCloseKey(hkey);
            if status == ERROR_FILE_NOT_FOUND {
                return Ok(());
            }
            status
                .ok()
                .map_err(|e| anyhow!("Failed to remove Windows Run value: {}", e))
        }
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use super::*;
    use anyhow::anyhow;
    use std::path::PathBuf;

    fn plist_path() -> Result<PathBuf> {
        let home = std::env::var("HOME").map_err(|_| anyhow!("HOME not set"))?;
        Ok(PathBuf::from(home)
            .join("Library")
            .join("LaunchAgents")
            .join("com.portline.desktop.plist"))
    }

    pub fn is_enabled() -> bool {
        plist_path().map(|p| p.exists()).unwrap_or(false)
    }

    pub fn enable(exe_path: &Path) -> Result<()> {
        let path = plist_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let escaped = exe_path
            .to_string_lossy()
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;");
        let content = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
<plist version=\"1.0\">\n\
<dict>\n\
  <key>Label</key>\n\
  <string>com.portline.desktop</string>\n\
  <key>ProgramArguments</key>\n\
  <array>\n\
    <string>{}</string>\n\
    <string>{}</string>\n\
  </array>\n\
  <key>RunAtLoad</key>\n\
  <true/>\n\
</dict>\n\
</plist>\n",
            escaped, AUTOSTART_ARG
        );
        std::fs::write(path, content)?;
        Ok(())
    }

    pub fn disable() -> Result<()> {
        let path = plist_path()?;
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        Ok(())
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
mod platform {
    use super::*;
    use anyhow::anyhow;
    use std::path::PathBuf;

    fn desktop_file_path() -> Result<PathBuf> {
        if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
            if !xdg.trim().is_empty() {
                return Ok(PathBuf::from(xdg).join("autostart").join("portline.desktop"));
            }
        }
        let home = std::env::var("HOME").map_err(|_| anyhow!("HOME not set"))?;
        Ok(PathBuf::from(home)
            .join(".config")
            .join("autostart")
            .join("portline.desktop"))
    }

    pub fn is_enabled() -> bool {
        desktop_file_path().map(|p| p.exists()).unwrap_or(false)
    }

    pub fn enable(exe_path: &Path) -> Result<()> {
        let path = desktop_file_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = format!(
            "[Desktop Entry]\n\
Type=Application\n\
Name=Portline\n\
Exec={}\n\
Terminal=false\n\
X-GNOME-Autostart-enabled=true\n",
            launch_command_for_exe(exe_path)
        );
        std::fs::write(path, content)?;
        Ok(())
    }

    pub fn disable() -> Result<()> {
        let path = desktop_file_path()?;
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        Ok(())
    }
}
