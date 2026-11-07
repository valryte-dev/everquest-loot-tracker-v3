use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct CapturedWindow {
    pub title: String,
    pub width: u32,
    pub height: u32,
    /// Top-down BGRA pixels.
    pub pixels: Vec<u8>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureWindow {
    pub id: String,
    pub title: String,
    pub process_name: String,
}

impl CapturedWindow {
    pub fn bmp_data_url(&self) -> String {
        let pixel_bytes = self.pixels.len() as u32;
        let file_size = 54 + pixel_bytes;
        let mut bmp = Vec::with_capacity(file_size as usize);
        bmp.extend_from_slice(b"BM");
        bmp.extend_from_slice(&file_size.to_le_bytes());
        bmp.extend_from_slice(&[0; 4]);
        bmp.extend_from_slice(&54u32.to_le_bytes());
        bmp.extend_from_slice(&40u32.to_le_bytes());
        bmp.extend_from_slice(&(self.width as i32).to_le_bytes());
        // Negative height keeps the captured top-down row order.
        bmp.extend_from_slice(&(-(self.height as i32)).to_le_bytes());
        bmp.extend_from_slice(&1u16.to_le_bytes());
        bmp.extend_from_slice(&32u16.to_le_bytes());
        bmp.extend_from_slice(&0u32.to_le_bytes());
        bmp.extend_from_slice(&pixel_bytes.to_le_bytes());
        bmp.extend_from_slice(&[0; 16]);
        bmp.extend_from_slice(&self.pixels);
        format!("data:image/bmp;base64,{}", base64(&bmp))
    }
}

fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let value = ((chunk[0] as u32) << 16)
            | ((chunk.get(1).copied().unwrap_or(0) as u32) << 8)
            | chunk.get(2).copied().unwrap_or(0) as u32;
        result.push(ALPHABET[((value >> 18) & 63) as usize] as char);
        result.push(ALPHABET[((value >> 12) & 63) as usize] as char);
        result.push(if chunk.len() > 1 {
            ALPHABET[((value >> 6) & 63) as usize] as char
        } else {
            '='
        });
        result.push(if chunk.len() > 2 {
            ALPHABET[(value & 63) as usize] as char
        } else {
            '='
        });
    }
    result
}

#[cfg(target_os = "windows")]
mod windows {
    use super::{CaptureWindow, CapturedWindow};
    use std::{ffi::c_void, mem, path::Path};

    type Hwnd = isize;
    type Hdc = isize;
    type Hgdiobj = isize;
    type Handle = isize;

    #[repr(C)]
    #[derive(Default)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    #[repr(C)]
    struct BitmapInfoHeader {
        size: u32,
        width: i32,
        height: i32,
        planes: u16,
        bit_count: u16,
        compression: u32,
        size_image: u32,
        x_pels_per_meter: i32,
        y_pels_per_meter: i32,
        clr_used: u32,
        clr_important: u32,
    }

    #[repr(C)]
    struct BitmapInfo {
        header: BitmapInfoHeader,
        colors: [u32; 1],
    }

    #[link(name = "user32")]
    unsafe extern "system" {
        fn EnumWindows(
            callback: unsafe extern "system" fn(Hwnd, isize) -> i32,
            value: isize,
        ) -> i32;
        fn IsWindowVisible(window: Hwnd) -> i32;
        fn IsIconic(window: Hwnd) -> i32;
        fn GetWindowTextLengthW(window: Hwnd) -> i32;
        fn GetWindowTextW(window: Hwnd, text: *mut u16, maximum: i32) -> i32;
        fn GetWindowRect(window: Hwnd, rect: *mut Rect) -> i32;
        fn GetForegroundWindow() -> Hwnd;
        fn GetWindowThreadProcessId(window: Hwnd, process_id: *mut u32) -> u32;
        fn GetDC(window: Hwnd) -> Hdc;
        fn ReleaseDC(window: Hwnd, dc: Hdc) -> i32;
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn OpenProcess(access: u32, inherit_handle: i32, process_id: u32) -> Handle;
        fn QueryFullProcessImageNameW(
            process: Handle,
            flags: u32,
            path: *mut u16,
            size: *mut u32,
        ) -> i32;
        fn CloseHandle(handle: Handle) -> i32;
    }

    #[link(name = "gdi32")]
    unsafe extern "system" {
        fn CreateCompatibleDC(dc: Hdc) -> Hdc;
        fn CreateCompatibleBitmap(dc: Hdc, width: i32, height: i32) -> Hgdiobj;
        fn SelectObject(dc: Hdc, object: Hgdiobj) -> Hgdiobj;
        fn BitBlt(
            destination: Hdc,
            x: i32,
            y: i32,
            width: i32,
            height: i32,
            source: Hdc,
            source_x: i32,
            source_y: i32,
            operation: u32,
        ) -> i32;
        fn GetDIBits(
            dc: Hdc,
            bitmap: Hgdiobj,
            start: u32,
            lines: u32,
            bits: *mut c_void,
            info: *mut BitmapInfo,
            usage: u32,
        ) -> i32;
        fn DeleteObject(object: Hgdiobj) -> i32;
        fn DeleteDC(dc: Hdc) -> i32;
    }

    unsafe extern "system" fn find_eq_window(window: Hwnd, output: isize) -> i32 {
        if IsWindowVisible(window) == 0 || IsIconic(window) != 0 {
            return 1;
        }
        let Some(title) = window_title(window) else {
            return 1;
        };
        let normalized = title.to_ascii_lowercase();
        if normalized.contains("everquest") && !normalized.contains("loot tracker") {
            let found = &mut *(output as *mut Option<(Hwnd, String)>);
            *found = Some((window, title));
            return 0;
        }
        1
    }

    unsafe extern "system" fn collect_windows(window: Hwnd, output: isize) -> i32 {
        if IsWindowVisible(window) == 0 || IsIconic(window) != 0 {
            return 1;
        }
        let Some(title) = window_title(window) else {
            return 1;
        };
        let mut rect = Rect::default();
        if GetWindowRect(window, &mut rect) == 0
            || rect.right - rect.left < 100
            || rect.bottom - rect.top < 100
        {
            return 1;
        }
        let windows = &mut *(output as *mut Vec<CaptureWindow>);
        windows.push(CaptureWindow {
            id: window.to_string(),
            title,
            process_name: process_name(window).unwrap_or_default(),
        });
        1
    }

    fn window_title(window: Hwnd) -> Option<String> {
        let length = unsafe { GetWindowTextLengthW(window) };
        if length <= 0 {
            return None;
        }
        let mut buffer = vec![0u16; length as usize + 1];
        let copied = unsafe { GetWindowTextW(window, buffer.as_mut_ptr(), buffer.len() as i32) };
        (copied > 0).then(|| String::from_utf16_lossy(&buffer[..copied as usize]))
    }

    fn is_everquest_title(title: &str) -> bool {
        let normalized = title.to_ascii_lowercase();
        normalized.contains("everquest") && !normalized.contains("loot tracker")
    }

    fn process_name(window: Hwnd) -> Option<String> {
        let mut process_id = 0u32;
        unsafe {
            GetWindowThreadProcessId(window, &mut process_id);
        }
        if process_id == 0 {
            return None;
        }
        let process = unsafe { OpenProcess(0x1000, 0, process_id) };
        if process == 0 {
            return None;
        }
        let mut path = vec![0u16; 32_768];
        let mut length = path.len() as u32;
        let read =
            unsafe { QueryFullProcessImageNameW(process, 0, path.as_mut_ptr(), &mut length) };
        unsafe {
            CloseHandle(process);
        }
        if read == 0 || length == 0 {
            return None;
        }
        let full_path = String::from_utf16_lossy(&path[..length as usize]);
        Path::new(&full_path)
            .file_name()
            .and_then(|value| value.to_str())
            .map(str::to_owned)
    }

    fn matches_target(window: Hwnd, title: &str, target: &CaptureWindow) -> bool {
        if target.id.parse::<Hwnd>().ok() == Some(window) {
            return true;
        }
        if !target.process_name.is_empty()
            && process_name(window)
                .is_some_and(|value| value.eq_ignore_ascii_case(&target.process_name))
        {
            return true;
        }
        title.eq_ignore_ascii_case(&target.title)
    }

    fn locate() -> Result<(Hwnd, String), String> {
        let mut found = None;
        unsafe {
            EnumWindows(
                find_eq_window,
                (&mut found as *mut Option<(Hwnd, String)>) as isize,
            );
        }
        found.ok_or_else(|| "No visible EverQuest window was found.".to_owned())
    }

    pub fn list_windows() -> Vec<CaptureWindow> {
        let mut windows: Vec<CaptureWindow> = Vec::new();
        unsafe {
            EnumWindows(
                collect_windows,
                (&mut windows as *mut Vec<CaptureWindow>) as isize,
            );
        }
        windows.sort_by(|left, right| {
            left.title
                .to_ascii_lowercase()
                .cmp(&right.title.to_ascii_lowercase())
        });
        windows
    }

    fn capture_impl(
        require_foreground: bool,
        target: Option<&CaptureWindow>,
        region: Option<(f64, f64, f64, f64)>,
    ) -> Result<CapturedWindow, String> {
        let (window, title) = if require_foreground {
            let foreground = unsafe { GetForegroundWindow() };
            let title = window_title(foreground).filter(|title| {
                target
                    .map(|target| matches_target(foreground, title, target))
                    .unwrap_or_else(|| is_everquest_title(title))
            }).ok_or_else(|| {
                if let Some(target) = target {
                    format!(
                        "Bring the selected EQ window to the foreground: {}",
                        target.title
                    )
                } else {
                    "No matching EverQuest window is in the foreground. Select the EQ window on the Experience page.".to_owned()
                }
            })?;
            (foreground, title)
        } else if let Some(target) = target {
            list_windows()
                .into_iter()
                .find(|candidate| {
                    candidate.id == target.id
                        || (!target.process_name.is_empty()
                            && candidate
                                .process_name
                                .eq_ignore_ascii_case(&target.process_name))
                        || candidate.title.eq_ignore_ascii_case(&target.title)
                })
                .and_then(|candidate| {
                    candidate
                        .id
                        .parse::<Hwnd>()
                        .ok()
                        .map(|window| (window, candidate.title))
                })
                .ok_or_else(|| "The selected EQ window is not currently available.".to_owned())?
        } else {
            locate()?
        };
        let mut rect = Rect::default();
        if unsafe { GetWindowRect(window, &mut rect) } == 0 {
            return Err("Unable to read the EverQuest window bounds.".to_owned());
        }
        let window_width = rect.right - rect.left;
        let window_height = rect.bottom - rect.top;
        if window_width <= 0 || window_height <= 0 {
            return Err("The EverQuest window has no visible capture area.".to_owned());
        }
        let (source_x, source_y, width, height) = region
            .map(|(x, y, width, height)| {
                let left = (x.clamp(0.0, 1.0) * window_width as f64).round() as i32;
                let top = (y.clamp(0.0, 1.0) * window_height as f64).round() as i32;
                let width = (width.clamp(0.0, 1.0) * window_width as f64).round() as i32;
                let height = (height.clamp(0.0, 1.0) * window_height as f64).round() as i32;
                (
                    rect.left + left,
                    rect.top + top,
                    width.min(window_width - left),
                    height.min(window_height - top),
                )
            })
            .unwrap_or((rect.left, rect.top, window_width, window_height));
        if width <= 0 || height <= 0 {
            return Err("The selected capture area has no visible pixels.".to_owned());
        }
        let screen = unsafe { GetDC(0) };
        if screen == 0 {
            return Err("Unable to access the desktop image.".to_owned());
        }
        let memory = unsafe { CreateCompatibleDC(screen) };
        let bitmap = unsafe { CreateCompatibleBitmap(screen, width, height) };
        if memory == 0 || bitmap == 0 {
            unsafe {
                if bitmap != 0 {
                    DeleteObject(bitmap);
                }
                if memory != 0 {
                    DeleteDC(memory);
                }
                ReleaseDC(0, screen);
            }
            return Err("Unable to create the EverQuest capture buffer.".to_owned());
        }
        let previous = unsafe { SelectObject(memory, bitmap) };
        let copied = unsafe {
            BitBlt(
                memory, 0, 0, width, height, screen, source_x, source_y, 0x40CC0020,
            )
        };
        let mut pixels = vec![0u8; width as usize * height as usize * 4];
        let mut info = BitmapInfo {
            header: BitmapInfoHeader {
                size: mem::size_of::<BitmapInfoHeader>() as u32,
                width,
                height: -height,
                planes: 1,
                bit_count: 32,
                compression: 0,
                size_image: pixels.len() as u32,
                x_pels_per_meter: 0,
                y_pels_per_meter: 0,
                clr_used: 0,
                clr_important: 0,
            },
            colors: [0],
        };
        let rows = if copied != 0 {
            unsafe {
                GetDIBits(
                    memory,
                    bitmap,
                    0,
                    height as u32,
                    pixels.as_mut_ptr().cast(),
                    &mut info,
                    0,
                )
            }
        } else {
            0
        };
        unsafe {
            SelectObject(memory, previous);
            DeleteObject(bitmap);
            DeleteDC(memory);
            ReleaseDC(0, screen);
        }
        if rows == 0 {
            return Err(
                "EverQuest could not be captured. Keep it visible and try again.".to_owned(),
            );
        }
        for pixel in pixels.chunks_exact_mut(4) {
            pixel[3] = 255;
        }
        Ok(CapturedWindow {
            title,
            width: width as u32,
            height: height as u32,
            pixels,
        })
    }

    pub fn capture(
        require_foreground: bool,
        target: Option<&CaptureWindow>,
    ) -> Result<CapturedWindow, String> {
        capture_impl(require_foreground, target, None)
    }

    pub fn capture_region(
        require_foreground: bool,
        target: Option<&CaptureWindow>,
        region: (f64, f64, f64, f64),
    ) -> Result<CapturedWindow, String> {
        capture_impl(require_foreground, target, Some(region))
    }
}

#[cfg(target_os = "windows")]
pub use windows::{capture, capture_region, list_windows};

#[cfg(not(target_os = "windows"))]
pub fn capture(
    _require_foreground: bool,
    _target: Option<&CaptureWindow>,
) -> Result<CapturedWindow, String> {
    Err("Visual experience capture is currently available on Windows.".to_owned())
}

#[cfg(not(target_os = "windows"))]
pub fn capture_region(
    _require_foreground: bool,
    _target: Option<&CaptureWindow>,
    _region: (f64, f64, f64, f64),
) -> Result<CapturedWindow, String> {
    Err("Visual experience capture is currently available on Windows.".to_owned())
}

#[cfg(not(target_os = "windows"))]
pub fn list_windows() -> Vec<CaptureWindow> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emits_a_bmp_data_url() {
        let capture = CapturedWindow {
            title: "test".into(),
            width: 1,
            height: 1,
            pixels: vec![0, 0, 255, 255],
        };
        assert!(capture
            .bmp_data_url()
            .starts_with("data:image/bmp;base64,Qk"));
    }
}
