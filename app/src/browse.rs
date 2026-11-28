//! The Browse button in Settings: Windows' own folder picker, and the check that what it
//! returned is a WoW folder.

use crate::commands::blocking;
use crate::settings::wide;
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};
use windows_sys::Win32::Foundation::MAX_PATH;
use windows_sys::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoCreateInstance,
    CoInitializeEx, CoTaskMemFree, CoUninitialize,
};
use windows_sys::Win32::UI::Shell::{
    BIF_EDITBOX, BIF_NEWDIALOGSTYLE, BIF_RETURNONLYFSDIRS, BROWSEINFOW, SHBrowseForFolderW,
    SHGetPathFromIDListW, ShellExecuteW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows_sys::core::GUID;

const CLSID_FILE_OPEN_DIALOG: GUID = GUID {
    data1: 0xDC1C_5A9C,
    data2: 0xE88A,
    data3: 0x4DDE,
    data4: [0xA5, 0xA1, 0x60, 0xF8, 0x2A, 0x20, 0xAE, 0xF7],
};
const IID_IFILE_DIALOG: GUID = GUID {
    data1: 0x42F8_5136,
    data2: 0xDB7E,
    data3: 0x439C,
    data4: [0x85, 0xF1, 0xE4, 0x07, 0x5D, 0x13, 0x5F, 0xC8],
};
const FOS_PICKFOLDERS: u32 = 0x0000_0020;
const FOS_FORCEFILESYSTEM: u32 = 0x0000_0040;
const FOS_PATHMUSTEXIST: u32 = 0x0000_0800;
const SIGDN_FILESYSPATH: u32 = 0x8005_8000;
const E_CANCELLED: i32 = 0x8007_04C7u32 as i32;

/// The World of Warcraft folder for a picked one: the folder itself when it holds
/// `_forever_`, its parent when the pick is `_forever_`. One plain line when it is neither.
pub fn wow_root(picked: &Path) -> Result<PathBuf, String> {
    if scarlet::gamedir::is_wow_root(picked) {
        return Ok(picked.to_path_buf());
    }
    if picked.file_name().is_some_and(|n| n == "_forever_")
        && let Some(parent) = picked.parent()
        && scarlet::gamedir::is_wow_root(parent)
    {
        return Ok(parent.to_path_buf());
    }
    Err(format!(
        "{} has no _forever_ folder. Choose the folder that contains it.",
        picked.display()
    ))
}

unsafe fn pick_modern(owner: isize, title: &[u16]) -> Result<Option<PathBuf>, ()> {
    let mut dlg: *mut c_void = std::ptr::null_mut();
    // SAFETY: COM is initialized on this STA thread; CLSID/IID match IFileDialog.
    let hr = unsafe {
        CoCreateInstance(
            &CLSID_FILE_OPEN_DIALOG,
            std::ptr::null_mut(),
            CLSCTX_INPROC_SERVER,
            &IID_IFILE_DIALOG,
            &mut dlg,
        )
    };
    if hr < 0 || dlg.is_null() {
        return Err(());
    }
    let vtbl = unsafe { *(dlg as *const *const *const c_void) };
    let release: unsafe extern "system" fn(*mut c_void) -> u32 =
        unsafe { std::mem::transmute(*vtbl.add(2)) };
    let show: unsafe extern "system" fn(*mut c_void, *mut c_void) -> i32 =
        unsafe { std::mem::transmute(*vtbl.add(3)) };
    let set_options: unsafe extern "system" fn(*mut c_void, u32) -> i32 =
        unsafe { std::mem::transmute(*vtbl.add(9)) };
    let get_options: unsafe extern "system" fn(*mut c_void, *mut u32) -> i32 =
        unsafe { std::mem::transmute(*vtbl.add(10)) };
    let set_title: unsafe extern "system" fn(*mut c_void, *const u16) -> i32 =
        unsafe { std::mem::transmute(*vtbl.add(17)) };
    let get_result: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> i32 =
        unsafe { std::mem::transmute(*vtbl.add(20)) };

    let mut opts = 0u32;
    let _ = unsafe { get_options(dlg, &mut opts) };
    let _ = unsafe {
        set_options(
            dlg,
            opts | FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST,
        )
    };
    let _ = unsafe { set_title(dlg, title.as_ptr()) };

    let shown = unsafe { show(dlg, owner as *mut c_void) };
    let picked = if shown >= 0 {
        let mut item: *mut c_void = std::ptr::null_mut();
        if unsafe { get_result(dlg, &mut item) } >= 0 && !item.is_null() {
            let ivtbl = unsafe { *(item as *const *const *const c_void) };
            let item_release: unsafe extern "system" fn(*mut c_void) -> u32 =
                unsafe { std::mem::transmute(*ivtbl.add(2)) };
            let get_name: unsafe extern "system" fn(*mut c_void, u32, *mut *mut u16) -> i32 =
                unsafe { std::mem::transmute(*ivtbl.add(5)) };
            let mut raw: *mut u16 = std::ptr::null_mut();
            let path =
                if unsafe { get_name(item, SIGDN_FILESYSPATH, &mut raw) } >= 0 && !raw.is_null() {
                    let mut len = 0usize;
                    while unsafe { *raw.add(len) } != 0 {
                        len += 1;
                    }
                    let slice = unsafe { std::slice::from_raw_parts(raw, len) };
                    let p = PathBuf::from(String::from_utf16_lossy(slice));
                    unsafe { CoTaskMemFree(raw.cast()) };
                    Some(p)
                } else {
                    None
                };
            unsafe { item_release(item) };
            Ok(path)
        } else {
            Ok(None)
        }
    } else if shown == E_CANCELLED {
        Ok(None)
    } else {
        Err(())
    };
    unsafe { release(dlg) };
    picked
}

fn pick_folder(owner: isize, heading: &'static str) -> Option<PathBuf> {
    std::thread::spawn(move || {
        // SAFETY: COM is started and stopped on this thread around the one dialog.
        unsafe {
            if CoInitializeEx(
                std::ptr::null(),
                (COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) as u32,
            ) < 0
            {
                return None;
            }
        }
        let title = wide(heading);
        let path = match unsafe { pick_modern(owner, &title) } {
            Ok(chosen) => chosen,
            Err(()) => {
                let mut name = [0u16; MAX_PATH as usize];
                let info = BROWSEINFOW {
                    hwndOwner: owner as *mut c_void,
                    pidlRoot: std::ptr::null_mut(),
                    pszDisplayName: name.as_mut_ptr(),
                    lpszTitle: title.as_ptr(),
                    ulFlags: BIF_RETURNONLYFSDIRS | BIF_NEWDIALOGSTYLE | BIF_EDITBOX,
                    lpfn: None,
                    lParam: 0,
                    iImage: 0,
                };
                // SAFETY: `info` and the buffers it points to outlive the call; pidl is freed.
                unsafe {
                    let pidl = SHBrowseForFolderW(&info);
                    if pidl.is_null() {
                        None
                    } else {
                        let mut buf = [0u16; MAX_PATH as usize];
                        let ok = SHGetPathFromIDListW(pidl, buf.as_mut_ptr()) != 0;
                        CoTaskMemFree(pidl.cast());
                        ok.then(|| {
                            let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
                            PathBuf::from(String::from_utf16_lossy(&buf[..end]))
                        })
                    }
                }
            }
        };
        unsafe { CoUninitialize() };
        path
    })
    .join()
    .ok()
    .flatten()
}

/// Opens the folder picker. Ok(None): closed without a choice. Ok(Some(path)): the World of
/// Warcraft folder to save. Err: one line saying why the choice is not one.
#[tauri::command]
pub async fn browse_wow(app: AppHandle) -> Result<Option<String>, String> {
    let owner = app
        .get_webview_window("main")
        .and_then(|w| w.hwnd().ok())
        .map_or(0, |h| h.0 as isize);
    blocking(move || match pick_folder(owner, "Choose the folder that contains _forever_") {
        None => Ok(None),
        Some(p) => wow_root(&p).map(|r| Some(r.display().to_string())),
    })
    .await
}

/// Opens the folder picker for the data folder. None: closed without a choice.
#[tauri::command]
pub async fn browse_data(app: AppHandle) -> Option<String> {
    let owner = app
        .get_webview_window("main")
        .and_then(|w| w.hwnd().ok())
        .map_or(0, |h| h.0 as isize);
    blocking(move || {
        pick_folder(owner, "Choose the data folder").map(|p| p.display().to_string())
    })
    .await
}

/// Shows a folder, or the folder that holds a file, in Explorer.
#[tauri::command]
pub async fn open_folder(path: String) -> Result<(), String> {
    blocking(move || {
        let given = PathBuf::from(&path);
        let dir = if given.is_dir() {
            given
        } else {
            given.parent().map(Path::to_path_buf).unwrap_or_default()
        };
        if !dir.is_dir() {
            return Err(format!("{} does not exist.", dir.display()));
        }
        // SAFETY: both strings are NUL-terminated and outlive the call; the other arguments are null.
        let rc = unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                wide("open").as_ptr(),
                wide(&dir.display().to_string()).as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                SW_SHOWNORMAL,
            )
        } as usize;
        if rc > 32 {
            Ok(())
        } else {
            Err(format!("Windows could not open the folder (code {rc})."))
        }
    })
    .await
}
