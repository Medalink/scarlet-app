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
    SHGetPathFromIDListW,
};
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
    // SAFETY: a COM object starts with a pointer to its vtable, and `dlg` is a live IFileDialog.
    let vtbl = unsafe { *(dlg as *const *const *const c_void) };
    // SAFETY: slot 2 of IUnknown's vtable is Release, which has this signature.
    let release: unsafe extern "system" fn(*mut c_void) -> u32 =
        unsafe { std::mem::transmute(*vtbl.add(2)) };
    // SAFETY: slot 3 of IModalWindow's vtable is Show, which has this signature.
    let show: unsafe extern "system" fn(*mut c_void, *mut c_void) -> i32 =
        unsafe { std::mem::transmute(*vtbl.add(3)) };
    // SAFETY: slot 9 of IFileDialog's vtable is SetOptions, which has this signature.
    let set_options: unsafe extern "system" fn(*mut c_void, u32) -> i32 =
        unsafe { std::mem::transmute(*vtbl.add(9)) };
    // SAFETY: slot 10 of IFileDialog's vtable is GetOptions, which has this signature.
    let get_options: unsafe extern "system" fn(*mut c_void, *mut u32) -> i32 =
        unsafe { std::mem::transmute(*vtbl.add(10)) };
    // SAFETY: slot 17 of IFileDialog's vtable is SetTitle, which has this signature.
    let set_title: unsafe extern "system" fn(*mut c_void, *const u16) -> i32 =
        unsafe { std::mem::transmute(*vtbl.add(17)) };
    // SAFETY: slot 20 of IFileDialog's vtable is GetResult, which has this signature.
    let get_result: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> i32 =
        unsafe { std::mem::transmute(*vtbl.add(20)) };

    let mut opts = 0u32;
    // SAFETY: `dlg` is a live IFileDialog and `opts` is a valid out pointer.
    let _ = unsafe { get_options(dlg, &mut opts) };
    // SAFETY: `dlg` is a live IFileDialog and the options are plain flags.
    let _ = unsafe {
        set_options(
            dlg,
            opts | FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST,
        )
    };
    // SAFETY: `dlg` is a live IFileDialog and `title` is NUL-terminated and outlives the dialog.
    let _ = unsafe { set_title(dlg, title.as_ptr()) };

    // SAFETY: `dlg` is a live IFileDialog and `owner` is a window handle of this process or null.
    let shown = unsafe { show(dlg, owner as *mut c_void) };
    let picked = if shown >= 0 {
        let mut item: *mut c_void = std::ptr::null_mut();
        // SAFETY: `dlg` is a live IFileDialog that was shown and `item` is a valid out pointer.
        let got = unsafe { get_result(dlg, &mut item) } >= 0;
        if got && !item.is_null() {
            // SAFETY: a COM object starts with a pointer to its vtable, and `item` is a live IShellItem.
            let ivtbl = unsafe { *(item as *const *const *const c_void) };
            // SAFETY: slot 2 of IUnknown's vtable is Release, which has this signature.
            let item_release: unsafe extern "system" fn(*mut c_void) -> u32 =
                unsafe { std::mem::transmute(*ivtbl.add(2)) };
            // SAFETY: slot 5 of IShellItem's vtable is GetDisplayName, which has this signature.
            let get_name: unsafe extern "system" fn(*mut c_void, u32, *mut *mut u16) -> i32 =
                unsafe { std::mem::transmute(*ivtbl.add(5)) };
            let mut raw: *mut u16 = std::ptr::null_mut();
            // SAFETY: `item` is a live IShellItem and `raw` is a valid out pointer.
            let named = unsafe { get_name(item, SIGDN_FILESYSPATH, &mut raw) } >= 0;
            let path = if named && !raw.is_null() {
                let mut len = 0usize;
                // SAFETY: on success `raw` is a NUL-terminated wide string, so every read up to the NUL is in bounds.
                while unsafe { *raw.add(len) } != 0 {
                    len += 1;
                }
                // SAFETY: `raw` holds `len` initialized units, as counted above, and is not freed until after the copy.
                let slice = unsafe { std::slice::from_raw_parts(raw, len) };
                let p = PathBuf::from(String::from_utf16_lossy(slice));
                // SAFETY: `raw` was allocated by the shell for the caller to free with CoTaskMemFree, and is not used again.
                unsafe { CoTaskMemFree(raw.cast()) };
                Some(p)
            } else {
                None
            };
            // SAFETY: `item` is a live IShellItem that is not used again.
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
    // SAFETY: `dlg` is the live IFileDialog from CoCreateInstance and is not used again.
    unsafe { release(dlg) };
    picked
}

fn pick_folder(owner: isize) -> Option<PathBuf> {
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
        let title = wide("Choose the folder that contains _forever_");
        // SAFETY: COM was initialized on this thread above, which is all pick_modern needs.
        let picked = unsafe { pick_modern(owner, &title) };
        let path = match picked {
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
        // SAFETY: balances the successful CoInitializeEx on this thread.
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
    blocking(move || match pick_folder(owner) {
        None => Ok(None),
        Some(p) => wow_root(&p).map(|r| Some(r.display().to_string())),
    })
    .await
}
