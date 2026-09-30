//! Буфер обмена: сохранить всё, что там лежит, положить свой текст,
//! вернуть как было. Работает только из потока исполнителя — у него
//! есть своё невидимое окно-владелец буфера.

use super::wide;
use std::ptr::{copy_nonoverlapping, null, null_mut};
use std::sync::OnceLock;
use std::thread::sleep;
use std::time::Duration;
use windows_sys::Win32::Foundation::{GlobalFree, HWND};
use windows_sys::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, EnumClipboardFormats, GetClipboardData,
    GetClipboardSequenceNumber, OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
};
use windows_sys::Win32::System::Memory::{
    GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{CreateWindowExW, HWND_MESSAGE};

const CF_UNICODETEXT: u32 = 13;
/// Больше не сохраняем: огромная копия из Excel иначе съест память и время.
const SAVE_LIMIT: usize = 64 * 1024 * 1024;

/// Невидимое окно-владелец. С OpenClipboard(NULL) по документации
/// SetClipboardData может не сработать — не рискуем.
pub fn owner_window() -> HWND {
    let class = wide("STATIC");
    unsafe {
        CreateWindowExW(
            0,
            class.as_ptr(),
            null(),
            0,
            0,
            0,
            0,
            0,
            HWND_MESSAGE,
            null_mut(),
            null_mut(),
            null(),
        )
    }
}

struct Open;

impl Drop for Open {
    fn drop(&mut self) {
        unsafe { CloseClipboard() };
    }
}

/// Буфер может быть занят другой программой на мгновение — пробуем ещё.
fn open(owner: HWND) -> Result<Open, String> {
    for _ in 0..15 {
        if unsafe { OpenClipboard(owner) } != 0 {
            return Ok(Open);
        }
        sleep(Duration::from_millis(20));
    }
    Err("буфер обмена занят другой программой".into())
}

fn registered(name: &str, cell: &'static OnceLock<u32>) -> u32 {
    *cell.get_or_init(|| unsafe { RegisterClipboardFormatW(wide(name).as_ptr()) })
}

fn private_formats() -> [u32; 3] {
    static EXCLUDE: OnceLock<u32> = OnceLock::new();
    static HISTORY: OnceLock<u32> = OnceLock::new();
    static CLOUD: OnceLock<u32> = OnceLock::new();
    [
        registered("ExcludeClipboardContentFromMonitorProcessing", &EXCLUDE),
        registered("CanIncludeInClipboardHistory", &HISTORY),
        registered("CanUploadToCloudClipboard", &CLOUD),
    ]
}

/// Форматы, которые хранятся не в памяти, а как объекты GDI или рисуются
/// владельцем. Их не скопировать байтами; картинки при этом всё равно
/// сохраняются — Windows держит их ещё и в формате DIB.
fn skip_format(f: u32) -> bool {
    matches!(f, 2 | 3 | 9 | 14 | 0x80..=0x8E | 0x200..=0x3FF) || private_formats().contains(&f)
}

/// Содержимое буфера: пары «формат — байты».
#[derive(Default)]
pub struct Saved(Vec<(u32, Vec<u8>)>);

pub fn save(owner: HWND) -> Result<Saved, String> {
    let _open = open(owner)?;
    let mut out = Vec::new();
    let mut total = 0usize;
    let mut fmt = 0u32;
    loop {
        fmt = unsafe { EnumClipboardFormats(fmt) };
        if fmt == 0 {
            break;
        }
        if skip_format(fmt) {
            continue;
        }
        unsafe {
            let h = GetClipboardData(fmt);
            if h.is_null() {
                continue;
            }
            let size = GlobalSize(h);
            if size == 0 || total + size > SAVE_LIMIT {
                continue;
            }
            let p = GlobalLock(h) as *const u8;
            if p.is_null() {
                continue;
            }
            let mut bytes = vec![0u8; size];
            copy_nonoverlapping(p, bytes.as_mut_ptr(), size);
            GlobalUnlock(h);
            total += size;
            out.push((fmt, bytes));
        }
    }
    Ok(Saved(out))
}

fn put(fmt: u32, bytes: &[u8]) -> Result<(), String> {
    unsafe {
        let h = GlobalAlloc(GMEM_MOVEABLE, bytes.len().max(1));
        if h.is_null() {
            return Err("не хватило памяти для буфера обмена".into());
        }
        let p = GlobalLock(h) as *mut u8;
        if p.is_null() {
            GlobalFree(h);
            return Err("не хватило памяти для буфера обмена".into());
        }
        copy_nonoverlapping(bytes.as_ptr(), p, bytes.len());
        GlobalUnlock(h);
        // При успехе память принадлежит системе, освобождать нельзя.
        if SetClipboardData(fmt, h).is_null() {
            GlobalFree(h);
            return Err("Windows не принял данные в буфер обмена".into());
        }
    }
    Ok(())
}

/// Пометки «не записывать в журнал буфера (Win+V), не отправлять в облако,
/// менеджерам буфера не сохранять» — наша подмена не должна засорять историю.
fn mark_private() {
    let [exclude, history, cloud] = private_formats();
    let _ = put(exclude, &[0]);
    let _ = put(history, &0u32.to_le_bytes());
    let _ = put(cloud, &0u32.to_le_bytes());
}

/// Положить текст. Возвращает номер версии буфера — по нему потом видно,
/// не скопировал ли человек что-то своё, пока мы ждали.
pub fn set_text(owner: HWND, text: &str) -> Result<u32, String> {
    // Программы Windows ждут переносы \r\n.
    let normalized = text.replace("\r\n", "\n").replace('\n', "\r\n");
    let bytes: Vec<u8> = normalized
        .encode_utf16()
        .chain(std::iter::once(0))
        .flat_map(u16::to_le_bytes)
        .collect();
    {
        let _open = open(owner)?;
        unsafe { EmptyClipboard() };
        put(CF_UNICODETEXT, &bytes)?;
        mark_private();
    }
    Ok(sequence())
}

pub fn restore(owner: HWND, saved: &Saved) -> Result<(), String> {
    let _open = open(owner)?;
    unsafe { EmptyClipboard() };
    for (fmt, bytes) in &saved.0 {
        // Отдельный формат может не встать (например, устаревший) —
        // остальные всё равно возвращаем.
        let _ = put(*fmt, bytes);
    }
    if !saved.0.is_empty() {
        mark_private();
    }
    Ok(())
}

pub fn sequence() -> u32 {
    unsafe { GetClipboardSequenceNumber() }
}
