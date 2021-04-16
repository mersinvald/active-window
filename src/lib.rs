use std::path::PathBuf;

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
    pub bounds: BoundsInfo,
    /// Information about the owning process
    pub owner: OwnerInfo,
    /// Current tab URL if the active window is browser (MacOS only)
    pub url: Option<Url>,
}

pub type WindowId = usize;
pub type ProcessId = u32;
pub type BundleId = usize;

#[cfg_attr(serde, derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct BoundsInfo {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[cfg_attr(serde, derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct OwnerInfo {
    pub name: String,
    pub path: PathBuf,
    pub id: ProcessId,
    pub bundle: Option<BundleId>,
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;

    pub fn active_window() -> Option<WindowInfo> {
        todo!()
    }
}

#[cfg(target_os = "windows")]
mod windows {
    use std::{
        mem::MaybeUninit,
        path::{Path, PathBuf},
    };

    use super::*;

    use winapi::{
        ctypes::c_void,
        shared::{
            minwindef::{FALSE, LPARAM, TRUE},
            windef::{HWND, RECT},
        },
        um::{
            handleapi::CloseHandle,
            processthreadsapi::OpenProcess,
            winbase::QueryFullProcessImageNameW,
            winnt::PROCESS_QUERY_LIMITED_INFORMATION,
            winuser::{
                EnumChildWindows, GetForegroundWindow, GetWindowRect, GetWindowTextLengthW,
                GetWindowTextW, GetWindowThreadProcessId,
            },
        },
    };

    type ProcessHandle = *mut c_void;

    pub fn active_window() -> Option<WindowInfo> {
        let handle = unsafe { GetForegroundWindow() };
        if handle.is_null() {
            return None;
        }

        Some(WindowInfo {
            id: handle as WindowId,
            title: get_title(handle)?,
            bounds: get_bounds(handle)?,
            owner: get_owner(handle)?,
            url: None,
        })
    }

    fn get_title(handle: HWND) -> Option<String> {
        let name_length = unsafe { GetWindowTextLengthW(handle) };

        if name_length < 0 {
            return None;
        }

        // Two extra bytes for possible null characters
        let mut name_utf16 = vec![0; name_length as usize + 2];
        unsafe {
            GetWindowTextW(handle, name_utf16.as_mut_ptr(), name_length + 2);
        }

        // Find last non-null character position
        let name_end = name_utf16.len() - name_utf16.iter().rev().position(|&x| x != 0)?;

        Some(String::from_utf16_lossy(&name_utf16[..name_end]))
    }

    fn get_bounds(handle: HWND) -> Option<BoundsInfo> {
        let mut rect = MaybeUninit::<RECT>::uninit();

        let rect = unsafe {
            GetWindowRect(handle, rect.as_mut_ptr());
            rect.assume_init()
        };

        Some(BoundsInfo {
            x: rect.left,
            y: rect.top,
            width: rect.right - rect.left,
            height: rect.bottom - rect.top,
        })
    }

    fn get_owner(handle: HWND) -> Option<OwnerInfo> {
        let mut owner_id: u32 = 0;

        let owner_handle = unsafe {
            GetWindowThreadProcessId(handle, &mut owner_id as *mut u32);
            OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, owner_id)
        };

        if owner_handle.is_null() {
            return None;
        }

        let mut owner_path = get_process_path(owner_handle)?;
        let mut owner_name = owner_path.file_name()?;

        // ApplicationFrameHost & Universal Windows Platform Support
        if owner_name == "ApplicationFrameHost.exe" {
            owner_path = get_subwindow_process_path(handle, &owner_path)?;
            owner_name = owner_path.file_name()?;
        }

        unsafe {
            CloseHandle(owner_handle);
        }

        Some(OwnerInfo {
            name: owner_name.to_str()?.to_string(),
            path: owner_path,
            id: owner_id,
            bundle: None,
        })
    }

    fn get_process_path(handle: ProcessHandle) -> Option<PathBuf> {
        // Set the path length to more than the Windows extended-length MAX_PATH length
        // The maximum path of 32,767 characters is approximate, because the "\\?\" prefix may be expanded to a longer string by the system at run time, and this expansion applies to the total length.
        const MAX_PATH_LENGTH: usize = 66000;
        const PATH_CHAR_COUNT: u32 = MAX_PATH_LENGTH as u32 / 2;

        let mut process_path_buffer = vec![0; MAX_PATH_LENGTH];
        let mut process_path_length = PATH_CHAR_COUNT;

        unsafe {
            QueryFullProcessImageNameW(
                handle,
                0,
                process_path_buffer.as_mut_ptr(),
                &mut process_path_length as *mut _,
            );
        }

        // Find last non-null character position
        let path_end = process_path_buffer.iter().position(|&x| x == 0)?;

        Some(
            String::from_utf16(&process_path_buffer[..path_end])
                .ok()?
                .into(),
        )
    }

    fn get_subwindow_process_path(
        active_window_handle: HWND,
        process_path: &Path,
    ) -> Option<PathBuf> {
        unsafe {
            SUBPROCESS_PATH = None;
            PROCESS_PATH = Some(process_path.to_owned());

            EnumChildWindows(
                active_window_handle,
                Some(get_subwindow_process_path_callback),
                0,
            );

            let subprocess_path = SUBPROCESS_PATH.take();
            let process_path = PROCESS_PATH.take();

            subprocess_path.or(process_path)
        }
    }

    static mut PROCESS_PATH: Option<PathBuf> = None;
    static mut SUBPROCESS_PATH: Option<PathBuf> = None;

    unsafe extern "system" fn get_subwindow_process_path_callback(
        hwnd: HWND,
        param: LPARAM,
    ) -> i32 {
        let handle = if let Some((_, handle)) = get_process_id_and_handle(hwnd) {
            handle
        } else {
            return FALSE;
        };

        let process_path = PROCESS_PATH.as_ref().unwrap();

        let path = get_process_path(handle).unwrap();
        if &path != process_path {
            SUBPROCESS_PATH = Some(path);
            return FALSE;
        }

        TRUE
    }

    fn get_process_id_and_handle(handle: HWND) -> Option<(ProcessId, ProcessHandle)> {
        let mut owner_id: u32 = 0;

        let owner_handle = unsafe {
            GetWindowThreadProcessId(handle, &mut owner_id as *mut u32);
            OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, owner_id)
        };

        if owner_handle.is_null() {
            return None;
        }

        Some((owner_id, owner_handle))
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::*;

    pub fn active_window() -> WindowInfo {
        todo!()
    }
}
