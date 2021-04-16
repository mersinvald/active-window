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
pub type ProcessId = i64;
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
    use std::{
        ffi::CStr,
        mem::MaybeUninit,
        os::raw::{c_int, c_ulong},
        ptr,
    };

    use once_cell::sync::Lazy;
    use procfs::process::Process;
    use x11_dl::xlib::{Atom, Display, Success, XTextProperty, Xlib, XA_CARDINAL, XA_WM_NAME};

    use super::*;

    type Window = c_ulong;

    static XLIB: Lazy<Xlib> = Lazy::new(|| Xlib::open().unwrap());

    pub fn active_window() -> Option<WindowInfo> {
        fn inner(display: *mut Display) -> Option<WindowInfo> {
            unsafe {
                // Get window currently in focus
                let mut window = MaybeUninit::<c_ulong>::uninit();
                let mut revert_to = MaybeUninit::<c_int>::uninit();
                let status =
                    (XLIB.XGetInputFocus)(display, window.as_mut_ptr(), revert_to.as_mut_ptr());

                let window = window.assume_init();

                if status == 0 || window == 0 {
                    return None;
                }

                Some(WindowInfo {
                    title: get_title(display, window)?,
                    id: window as usize,
                    bounds: get_bounds(display, window)?,
                    owner: get_owner(display, window)?,
                    url: None,
                })
            }
        }

        unsafe {
            let display = (XLIB.XOpenDisplay)(ptr::null());

            let result = inner(display);

            (XLIB.XCloseDisplay)(display);

            result
        }
    }

    const MAX_PROPERTY_LENGTH: i64 = 1024;

    unsafe fn get_title(display: *mut Display, window: Window) -> Option<String> {
        let mut wm_name_atom = (XLIB.XInternAtom)(
            display,
            CStr::from_bytes_with_nul_unchecked(b"_NET_WM_NAME\0").as_ptr(),
            0,
        );

        // Fallback to WM_NAME
        if wm_name_atom == 0 {
            wm_name_atom = XA_WM_NAME;
        }

        let mut property = MaybeUninit::uninit();
        let status = (XLIB.XGetTextProperty)(display, window, property.as_mut_ptr(), wm_name_atom);

        if status == 0 {
            return None;
        }

        let property = property.assume_init();

        if property.nitems == 0 || property.encoding == 0 || property.value.is_null() {
            if !property.value.is_null() {
                (XLIB.XFree)(property.value as *mut _);
            }
            return None;
        }

        let title = text_property_to_string(&property);

        (XLIB.XFree)(property.value as *mut _);

        title
    }

    unsafe fn get_bounds(display: *mut Display, window: Window) -> Option<BoundsInfo> {
        let mut root = 0;
        let mut x = 0;
        let mut y = 0;
        let mut w = 0;
        let mut h = 0;
        let mut bw = 0;
        let mut d = 0;

        let status = (XLIB.XGetGeometry)(
            display,
            window,
            ptr::addr_of_mut!(root),
            ptr::addr_of_mut!(x),
            ptr::addr_of_mut!(y),
            ptr::addr_of_mut!(w),
            ptr::addr_of_mut!(h),
            ptr::addr_of_mut!(bw),
            ptr::addr_of_mut!(d),
        );

        if status == 0 {
            return None;
        }

        Some(BoundsInfo {
            x: x as i32,
            y: y as i32,
            width: w as i32,
            height: h as i32,
        })
    }

    unsafe fn get_owner(display: *mut Display, window: Window) -> Option<OwnerInfo> {
        let wm_pid_atom = (XLIB.XInternAtom)(
            display,
            CStr::from_bytes_with_nul_unchecked(b"_NET_WM_PID\0").as_ptr(),
            0,
        );

        let mut actual_type: Atom = 0;
        let mut actual_format: c_int = 0;
        let mut nitems: c_ulong = 0;
        let mut bytes_after: c_ulong = 0;
        let mut value: *mut u8 = ptr::null_mut();

        let status = (XLIB.XGetWindowProperty)(
            display,
            window,
            wm_pid_atom,
            0,
            MAX_PROPERTY_LENGTH,
            0,
            XA_CARDINAL,
            ptr::addr_of_mut!(actual_type),
            ptr::addr_of_mut!(actual_format),
            ptr::addr_of_mut!(nitems),
            ptr::addr_of_mut!(bytes_after),
            ptr::addr_of_mut!(value),
        );

        if status != Success.into() || value.is_null() {
            if !value.is_null() {
                (XLIB.XFree)(value.cast());
            }
            return None;
        }

        let owner_pid: u32 = value.cast::<u32>().read();
        (XLIB.XFree)(value.cast());

        let process = Process::new(owner_pid as i32).ok()?;

        let path = process.exe().ok()?;
        let name = path.file_name()?.to_str()?.to_string();

        Some(OwnerInfo {
            name,
            path,
            id: owner_pid as i64,
            bundle: None,
        })
    }

    unsafe fn text_property_to_string(prop: &XTextProperty) -> Option<String> {
        match prop.format {
            8 => {
                let data = std::slice::from_raw_parts(prop.value, prop.nitems as usize);
                String::from_utf8(data.to_vec()).ok()
            }
            16 => {
                let data =
                    std::slice::from_raw_parts(prop.value as *const u16, prop.nitems as usize);
                String::from_utf16(data).ok()
            }
            32 => {
                let data =
                    std::slice::from_raw_parts(prop.value as *const u32, prop.nitems as usize);
                data.iter().copied().map(std::char::from_u32).collect()
            }
            _ => None,
        }
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
            id: owner_id as i64,
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
        _param: LPARAM,
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

        Some((owner_id as i64, owner_handle))
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::*;

    pub fn active_window() -> WindowInfo {
        todo!()
    }
}
