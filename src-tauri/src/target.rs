use serde::Serialize;
#[derive(Clone, Copy, Debug, Serialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Surface {
    pub rect: Rect,
    pub data_url: String,
}
#[derive(Clone, Serialize)]
pub struct Target {
    pub rect: Rect,
    pub surface: Option<Surface>,
    pub failure: Option<String>,
}
impl Target {
    pub fn status(&self) -> String {
        self.failure
            .clone()
            .unwrap_or_else(|| "按点击位置撞击，局部画面已获取".into())
    }
}
// A fixed click-centered deformation area and padded screenshot, clipped to the main display.
// Neither size nor position depends on accessibility elements or application windows.
fn areas(x: f64, y: f64, width: f64, height: f64) -> Option<(Rect, Rect)> {
    if ![x, y, width, height].iter().all(|v| v.is_finite())
        || width <= 0.
        || height <= 0.
        || x < 0.
        || y < 0.
        || x >= width
        || y >= height
    {
        return None;
    }
    let left = (x - 40.).max(0.);
    let top = (y - 24.).max(0.);
    let right = (x + 40.).min(width);
    let bottom = (y + 24.).min(height);
    let rect = Rect {
        x: left,
        y: top,
        width: right - left,
        height: bottom - top,
    };
    let px = (left - 64.).max(0.);
    let py = (top - 64.).max(0.);
    let patch = Rect {
        x: px,
        y: py,
        width: (right + 64.).min(width) - px,
        height: (bottom + 64.).min(height) - py,
    };
    Some((rect, patch))
}
fn failed(rect: Rect, reason: &str) -> Target {
    Target {
        rect,
        surface: None,
        failure: Some(format!("{reason}，本次未播放撞击")),
    }
}
#[cfg(target_os = "macos")]
mod native {
    use super::*;
    use base64::Engine;
    static CAPTURE: std::sync::Mutex<()> = std::sync::Mutex::new(());
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct NativeRect {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    }
    extern "C" {
        fn mb_capture(rect: NativeRect, scale: f64, excluded: u32, length: *mut usize) -> *mut u8;
        fn mb_free(bytes: *mut std::ffi::c_void);
        fn mb_screen_permission(request: bool) -> bool;
        fn mb_window_id(window: *mut std::ffi::c_void) -> u32;
    }
    pub fn screen_permission(request: bool) -> bool {
        unsafe { mb_screen_permission(request) }
    }
    pub fn window_id(window: *mut std::ffi::c_void) -> u32 {
        unsafe { mb_window_id(window) }
    }
    pub fn capture(x: f64, y: f64, width: f64, height: f64, scale: f64, excluded: u32) -> Target {
        let Some((rect, patch)) = areas(x, y, width, height) else {
            return failed(
                Rect {
                    x: 0.,
                    y: 0.,
                    width: 0.,
                    height: 0.,
                },
                "点击位置无效",
            );
        };
        if !scale.is_finite() || scale <= 0. {
            return failed(rect, "显示缩放无效");
        }
        let Ok(_guard) = CAPTURE.try_lock() else {
            return failed(rect, "局部画面正在获取，请重试");
        };
        if !screen_permission(false) {
            return failed(rect, "屏幕录制权限不可用，请在系统设置中授权");
        }
        let mut length = 0;
        let bytes = unsafe {
            mb_capture(
                NativeRect {
                    x: patch.x,
                    y: patch.y,
                    width: patch.width,
                    height: patch.height,
                },
                scale.min(2.),
                excluded,
                &mut length,
            )
        };
        if bytes.is_null() {
            return failed(
                rect,
                "局部画面获取失败或超时，请重试（需要 macOS 14 或以上）",
            );
        }
        if length == 0 || length > 8 * 1024 * 1024 {
            unsafe { mb_free(bytes.cast()) };
            return failed(rect, "局部画面数据无效");
        }
        let encoded = base64::engine::general_purpose::STANDARD
            .encode(unsafe { std::slice::from_raw_parts(bytes, length) });
        unsafe { mb_free(bytes.cast()) };
        Target {
            rect,
            surface: Some(Surface {
                rect: patch,
                data_url: format!("data:image/png;base64,{encoded}"),
            }),
            failure: None,
        }
    }
}
#[cfg(target_os = "macos")]
pub use native::*;
#[cfg(not(target_os = "macos"))]
pub fn screen_permission(_: bool) -> bool {
    false
}
#[cfg(not(target_os = "macos"))]
pub fn capture(x: f64, y: f64, width: f64, height: f64, _: f64, _: u32) -> Target {
    let rect = areas(x, y, width, height).map(|a| a.0).unwrap_or(Rect {
        x: 0.,
        y: 0.,
        width: 0.,
        height: 0.,
    });
    failed(rect, "当前平台尚未实现局部画面捕获")
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn click_areas_cover_point_and_stay_inside_display() {
        for (x, y) in [(0., 0.), (1., 1.), (500., 350.), (999., 699.)] {
            let (r, p) = areas(x, y, 1000., 700.).unwrap();
            for area in [r, p] {
                assert!(area.x >= 0. && area.y >= 0. && area.width > 0. && area.height > 0.);
                assert!(area.x + area.width <= 1000. && area.y + area.height <= 700.);
                assert!(
                    area.x <= x
                        && x < area.x + area.width
                        && area.y <= y
                        && y < area.y + area.height
                );
            }
            assert!(p.width <= 208. && p.height <= 176.);
        }
    }
    #[test]
    fn interior_area_is_centered_at_click_with_fixed_padding() {
        let (r, p) = areas(500., 350., 1000., 700.).unwrap();
        assert_eq!((r.x + r.width / 2., r.y + r.height / 2.), (500., 350.));
        assert_eq!(
            (r.width, r.height, p.width, p.height),
            (80., 48., 208., 176.)
        );
    }
    #[test]
    fn invalid_clicks_do_not_create_capture_regions() {
        for (x, y, w, h) in [
            (f64::NAN, 0., 1000., 700.),
            (-1., 0., 1000., 700.),
            (1000., 0., 1000., 700.),
            (0., 700., 1000., 700.),
            (0., 0., 0., 700.),
        ] {
            assert!(areas(x, y, w, h).is_none());
        }
    }
}
