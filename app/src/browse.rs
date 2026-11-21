//! The Browse button in Settings: Windows' own folder picker, and the check that what it
//! returned is a WoW folder.

use crate::settings::wide;
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};
use windows_sys::Win32::Foundation::MAX_PATH;
use windows_sys::Win32::System::Com::{
    COINIT_APARTMENTTHREADED, CoInitializeEx, CoTaskMemFree, CoUninitialize,
};
use windows_sys::Win32::UI::Shell::{
    BIF_EDITBOX, BIF_NEWDIALOGSTYLE, BIF_RETURNONLYFSDIRS, BROWSEINFOW, SHBrowseForFolderW,
    SHGetPathFromIDListW,
};

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

/// Shows the picker on a thread of its own (it needs COM, and a command thread has none) and
/// returns the chosen folder, or None when the window is closed without a choice.
fn pick_folder(owner: isize) -> Option<PathBuf> {
    std::thread::spawn(move || {
        // SAFETY: COM is started and stopped on this thread around the one dialog.
        unsafe {
            if CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as u32) < 0 {
                return None;
            }
        }
        let title = wide("Choose the folder that contains _forever_");
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
        // SAFETY: `info` and the buffers it points to outlive the call; the returned item
        // list is read once and freed.
        unsafe {
            let pidl = SHBrowseForFolderW(&info);
            let path = if pidl.is_null() {
                None
            } else {
                let mut buf = [0u16; MAX_PATH as usize];
                let ok = SHGetPathFromIDListW(pidl, buf.as_mut_ptr()) != 0;
                CoTaskMemFree(pidl.cast());
                ok.then(|| {
                    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
                    PathBuf::from(String::from_utf16_lossy(&buf[..end]))
                })
            };
            CoUninitialize();
            path
        }
    })
    .join()
    .ok()
    .flatten()
}

/// Opens the folder picker. Ok(None): closed without a choice. Ok(Some(path)): the World of
/// Warcraft folder to save. Err: one line saying why the choice is not one.
#[tauri::command(async)]
pub fn browse_wow(app: AppHandle) -> Result<Option<String>, String> {
    let owner = app
        .get_webview_window("main")
        .and_then(|w| w.hwnd().ok())
        .map_or(0, |h| h.0 as isize);
    match pick_folder(owner) {
        None => Ok(None),
        Some(p) => wow_root(&p).map(|r| Some(r.display().to_string())),
    }
}
