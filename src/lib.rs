pub use url::Url;

#[cfg(target_os = "linux")]
pub use self::linux::*;

#[cfg(target_os = "windows")]
pub use self::windows::*;

#[cfg(target_os = "macos")]
pub use self::macos::*;

#[cfg_attr(serde, derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct WindowInfo {
    /// Window title
    pub title: String,
    /// Unique window id
    pub id: WindowId,
    /// Window ize and position 
    pub bounds: WindowBounds,
    /// Information about the owning process
    pub owner: WindowOwner,
    /// Current tab URL if the active window is browser (MacOS only)
    pub url: Option<Url>,
}

pub type WindowId = usize;
pub type ProcessId = usize;
pub type BundleId = usize;

#[cfg_attr(serde, derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct WindowBounds {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[cfg_attr(serde, derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct OwnerInfo {
    pub name: String,
    pub id: ProcessId,
    pub bundle: Option<BundleId>,
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;

    pub fn active_window() -> WindowInfo {
        todo!()
    }
}

#[cfg(target_os = "windows")]
mod windows {
    use super::*;

    use user32::HWID;

    pub fn active_window() -> Option<WindowInfo> {
        let id = user32::GetForegroundWindow();

        
        Some(WindowInfo { 
            
        })
    }

    fn window_title(id: HWID) -> Option<String> {
        let name_length = user32::GetWindowTextLengthW(id);
        
        if name_length < 0 {
            return None;
        }

        let mut name_utf16 = vec![0; name_length as usize + 2];
        user32::GetWindowTextW(wid, name_utf16.as_mut_ptr(), name_length + 2);

        let name_end = name_utf16.len() - name_utf16.iter().rev().position(|&x| x != 0)?;

        Some(String::from_utf16_lossy(&name_utf16[..name_end]))
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::*;

    pub fn active_window() -> WindowInfo {
        todo!()
    }
}