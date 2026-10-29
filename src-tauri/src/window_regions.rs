//! Window geometry for capture prediction. No window titles, app names, image
//! capture, or accessibility tree access is needed for this snapshot.

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WindowBounds {
    pub pid: u32,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub native: Option<NativeWindow>,
}

/// Native identity stays in Rust/the owned helper protocol; never supplied by a WebView.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct NativeWindow {
    pub hwnd: isize,
    pub thread_id: u32,
    pub process_started: u64,
    pub class_atom: u16,
    pub z_order: u32,
}

#[cfg(all(feature = "desktop", target_os = "windows"))]
pub mod windows;

#[cfg(any(target_os = "macos", test))]
#[derive(Clone, Copy)]
struct Metadata {
    pid: u32,
    layer: i32,
    alpha: f64,
    on_screen: bool,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

#[cfg(any(target_os = "macos", test))]
fn candidate(item: Metadata, own_pid: u32, popup_layer: i32) -> Option<WindowBounds> {
    // Normal, floating, modal and popup windows are useful candidates. Desktop
    // elements and higher overlay/dragging/screensaver/cursor layers are not.
    if item.pid == own_pid
        || !item.on_screen
        || item.layer < 0
        || item.layer > popup_layer
        || !item.alpha.is_finite()
        || item.alpha <= 0.0
        || ![item.x, item.y, item.width, item.height]
            .iter()
            .all(|value| value.is_finite())
        || item.width < 1.0
        || item.height < 1.0
    {
        return None;
    }
    let left = item.x.floor();
    let top = item.y.floor();
    let right = (item.x + item.width).ceil();
    let bottom = (item.y + item.height).ceil();
    if left < f64::from(i32::MIN)
        || top < f64::from(i32::MIN)
        || right > f64::from(i32::MAX)
        || bottom > f64::from(i32::MAX)
    {
        return None;
    }
    Some(WindowBounds {
        pid: item.pid,
        native: None,
        x: left as i32,
        y: top as i32,
        width: (right - left) as u32,
        height: (bottom - top) as u32,
    })
}

#[cfg(all(
    feature = "desktop",
    not(any(target_os = "macos", target_os = "windows"))
))]
pub fn snapshot(own_pid: u32) -> Result<Vec<WindowBounds>, String> {
    let windows = xcap::Window::all().map_err(|error| error.to_string())?;
    Ok(windows
        .into_iter()
        .filter(|window| {
            window.pid().ok() != Some(own_pid) && !window.is_minimized().unwrap_or(false)
        })
        .filter_map(|window| {
            Some(WindowBounds {
                pid: window.pid().ok()?,
                native: None,
                x: window.x().ok()?,
                y: window.y().ok()?,
                width: window.width().ok()?,
                height: window.height().ok()?,
            })
        })
        .collect())
}

#[cfg(all(feature = "desktop", target_os = "macos"))]
pub use macos::snapshot;

#[cfg(all(feature = "desktop", target_os = "windows"))]
pub use windows::snapshot;

#[derive(Clone, Copy, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct ImagePoint {
    pub x: u32,
    pub y: u32,
}

/// On macOS tao scales cursor coordinates by the primary display, while window
/// coordinates use the window display. Normalize each before taking the delta.
pub fn cursor_in_image(
    cursor: (f64, f64),
    cursor_scale: f64,
    origin: (f64, f64),
    window_scale: f64,
    viewport: (u32, u32),
    image: (u32, u32),
) -> Option<ImagePoint> {
    if ![
        cursor.0,
        cursor.1,
        origin.0,
        origin.1,
        cursor_scale,
        window_scale,
    ]
    .iter()
    .all(|value| value.is_finite())
        || cursor_scale <= 0.0
        || window_scale <= 0.0
        || viewport.0 == 0
        || viewport.1 == 0
        || image.0 == 0
        || image.1 == 0
    {
        return None;
    }
    let x = cursor.0 / cursor_scale - origin.0 / window_scale;
    let y = cursor.1 / cursor_scale - origin.1 / window_scale;
    let width = f64::from(viewport.0) / window_scale;
    let height = f64::from(viewport.1) / window_scale;
    if x < 0.0 || y < 0.0 || x >= width || y >= height {
        return None;
    }
    Some(ImagePoint {
        x: ((x / width * f64::from(image.0)).floor() as u32).min(image.0 - 1),
        y: ((y / height * f64::from(image.1)).floor() as u32).min(image.1 - 1),
    })
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct ElementBounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[cfg(any(target_os = "macos", test))]
fn can_query_element(trusted: bool, pid: u32, position: (f64, f64)) -> bool {
    trusted && pid > 0 && pid <= i32::MAX as u32 && position.0.is_finite() && position.1.is_finite()
}

pub fn element_in_image(
    bounds: ElementBounds,
    screen_origin: (i32, i32),
    screen_size: (u32, u32),
    image_size: (u32, u32),
    host: crate::imaging::Rect,
    point: ImagePoint,
) -> Option<crate::imaging::Rect> {
    if screen_size.0 == 0
        || screen_size.1 == 0
        || ![bounds.x, bounds.y, bounds.width, bounds.height]
            .iter()
            .all(|n| n.is_finite())
        || bounds.width <= 0.0
        || bounds.height <= 0.0
        || host.validate(image_size.0, image_size.1).is_err()
    {
        return None;
    }
    let sx = f64::from(image_size.0) / f64::from(screen_size.0);
    let sy = f64::from(image_size.1) / f64::from(screen_size.1);
    let left = ((bounds.x - f64::from(screen_origin.0)) * sx)
        .floor()
        .max(f64::from(host.x));
    let top = ((bounds.y - f64::from(screen_origin.1)) * sy)
        .floor()
        .max(f64::from(host.y));
    let right = ((bounds.x + bounds.width - f64::from(screen_origin.0)) * sx)
        .ceil()
        .min(f64::from(host.x + host.width));
    let bottom = ((bounds.y + bounds.height - f64::from(screen_origin.1)) * sy)
        .ceil()
        .min(f64::from(host.y + host.height));
    if right - left < 2.0
        || bottom - top < 2.0
        || f64::from(point.x) < left
        || f64::from(point.x) >= right
        || f64::from(point.y) < top
        || f64::from(point.y) >= bottom
    {
        return None;
    }
    Some(crate::imaging::Rect {
        x: left as u32,
        y: top as u32,
        width: (right - left) as u32,
        height: (bottom - top) as u32,
    })
}

#[cfg(feature = "desktop")]
pub fn element_detection_available() -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::trusted()
    }
    #[cfg(target_os = "windows")]
    {
        true
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        false
    }
}

#[cfg(feature = "desktop")]
pub fn element_at(pid: u32, position: (f64, f64)) -> Option<ElementBounds> {
    // The non-macOS path has no AX implementation or permission side effect.
    #[cfg(target_os = "macos")]
    {
        macos::element_at(pid, position)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (pid, position);
        None
    }
}

#[cfg(all(feature = "desktop", target_os = "macos"))]
mod macos {
    use super::{can_query_element, candidate, ElementBounds, Metadata, WindowBounds};
    use std::ffi::{c_char, c_void, CStr};

    type Ref = *const c_void;
    #[repr(C)]
    #[derive(Default)]
    struct Point {
        x: f64,
        y: f64,
    }
    #[repr(C)]
    #[derive(Default)]
    struct Size {
        width: f64,
        height: f64,
    }
    #[repr(C)]
    #[derive(Default)]
    struct Rect {
        origin: Point,
        size: Size,
    }

    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGWindowListCopyWindowInfo(options: u32, relative_to: u32) -> Ref;
        fn CGRectMakeWithDictionaryRepresentation(dictionary: Ref, rect: *mut Rect) -> bool;
        fn CGWindowLevelForKey(key: i32) -> i32;
    }
    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFRelease(value: Ref);
        fn CFGetTypeID(value: Ref) -> usize;
        fn CFArrayGetCount(array: Ref) -> isize;
        fn CFArrayGetValueAtIndex(array: Ref, index: isize) -> Ref;
        fn CFDictionaryGetTypeID() -> usize;
        fn CFDictionaryGetValue(dictionary: Ref, key: Ref) -> Ref;
        fn CFNumberGetTypeID() -> usize;
        fn CFNumberGetValue(number: Ref, kind: isize, value: *mut c_void) -> bool;
        fn CFBooleanGetTypeID() -> usize;
        fn CFBooleanGetValue(boolean: Ref) -> bool;
        fn CFStringCreateWithCString(allocator: Ref, value: *const c_char, encoding: u32) -> Ref;
    }
    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        fn AXIsProcessTrusted() -> bool;
        fn AXUIElementCreateApplication(pid: i32) -> Ref;
        fn AXUIElementSetMessagingTimeout(element: Ref, timeout: f32) -> i32;
        fn AXUIElementCopyElementAtPosition(
            application: Ref,
            x: f32,
            y: f32,
            element: *mut Ref,
        ) -> i32;
        fn AXUIElementCopyAttributeValue(element: Ref, attribute: Ref, value: *mut Ref) -> i32;
        fn AXValueGetTypeID() -> usize;
        fn AXValueGetValue(value: Ref, kind: i32, output: *mut c_void) -> bool;
    }
    struct Owned(Ref);
    impl Drop for Owned {
        fn drop(&mut self) {
            unsafe { CFRelease(self.0) }
        }
    }
    fn key(value: &CStr) -> Result<Owned, String> {
        let value =
            unsafe { CFStringCreateWithCString(std::ptr::null(), value.as_ptr(), 0x0800_0100) };
        if value.is_null() {
            Err("无法准备窗口几何信息".into())
        } else {
            Ok(Owned(value))
        }
    }
    unsafe fn number(dictionary: Ref, key: &Owned) -> Option<f64> {
        let value = CFDictionaryGetValue(dictionary, key.0);
        if value.is_null() || CFGetTypeID(value) != CFNumberGetTypeID() {
            return None;
        }
        let mut result = 0.0_f64;
        // kCFNumberFloat64Type = 6. Accept integer and floating metadata alike.
        CFNumberGetValue(value, 6, (&mut result as *mut f64).cast()).then_some(result)
    }
    unsafe fn on_screen(dictionary: Ref, key: &Owned) -> bool {
        let value = CFDictionaryGetValue(dictionary, key.0);
        // OptionOnScreenOnly already restricts the snapshot. An explicit false
        // is rejected; a missing optional key does not erase a valid rectangle.
        value.is_null() || CFGetTypeID(value) != CFBooleanGetTypeID() || CFBooleanGetValue(value)
    }

    pub fn trusted() -> bool {
        // The non-prompting check never requests or changes Accessibility access.
        unsafe { AXIsProcessTrusted() }
    }

    pub fn element_at(pid: u32, position: (f64, f64)) -> Option<ElementBounds> {
        if !can_query_element(trusted(), pid, position) {
            return None;
        }
        let started = std::time::Instant::now();
        let remaining = || {
            let seconds = 0.080 - started.elapsed().as_secs_f32();
            (seconds > 0.0).then_some(seconds)
        };
        unsafe {
            let app = AXUIElementCreateApplication(pid as i32);
            if app.is_null() {
                return None;
            }
            let app = Owned(app);
            if AXUIElementSetMessagingTimeout(app.0, remaining()?) != 0 {
                return None;
            }
            let mut element = std::ptr::null();
            let hit = AXUIElementCopyElementAtPosition(
                app.0,
                position.0 as f32,
                position.1 as f32,
                &mut element,
            );
            if element.is_null() {
                return None;
            }
            let element = Owned(element);
            if hit != 0 {
                return None;
            }
            let copy_geometry = |name: &CStr| -> Option<Owned> {
                let attribute = key(name).ok()?;
                // The SDK specifies that timeouts belong to each AX object, not
                // equal/child objects. Apply the remaining 80ms budget here too.
                if AXUIElementSetMessagingTimeout(element.0, remaining()?) != 0 {
                    return None;
                }
                let mut value = std::ptr::null();
                let copied = AXUIElementCopyAttributeValue(element.0, attribute.0, &mut value);
                if value.is_null() {
                    return None;
                }
                let value = Owned(value);
                (copied == 0 && CFGetTypeID(value.0) == AXValueGetTypeID()).then_some(value)
            };
            let position = copy_geometry(c"AXPosition")?;
            let size = copy_geometry(c"AXSize")?;
            let mut point = Point::default();
            let mut dimensions = Size::default();
            if !AXValueGetValue(position.0, 1, (&mut point as *mut Point).cast())
                || !AXValueGetValue(size.0, 2, (&mut dimensions as *mut Size).cast())
            {
                return None;
            }
            Some(ElementBounds {
                x: point.x,
                y: point.y,
                width: dimensions.width,
                height: dimensions.height,
            })
        }
    }

    pub fn snapshot(own_pid: u32) -> Result<Vec<WindowBounds>, String> {
        // One immutable front-to-back snapshot, before our capture overlays exist.
        // Unlike xcap's capturable-window inventory, geometry does not depend on
        // kCGWindowName or kCGWindowSharingState being present/nonzero.
        let list = unsafe { CGWindowListCopyWindowInfo(1 | 16, 0) };
        if list.is_null() {
            return Err("系统未返回窗口边界".into());
        }
        let list = Owned(list);
        let bounds_key = key(c"kCGWindowBounds")?;
        let pid_key = key(c"kCGWindowOwnerPID")?;
        let layer_key = key(c"kCGWindowLayer")?;
        let alpha_key = key(c"kCGWindowAlpha")?;
        let visible_key = key(c"kCGWindowIsOnscreen")?;
        // kCGPopUpMenuWindowLevelKey = 11; query the OS rather than hardcoding
        // its level, so normal floating and modal panels are retained.
        let popup_layer = unsafe { CGWindowLevelForKey(11) };
        let mut result = Vec::new();
        unsafe {
            for index in 0..CFArrayGetCount(list.0) {
                let dictionary = CFArrayGetValueAtIndex(list.0, index);
                if dictionary.is_null() || CFGetTypeID(dictionary) != CFDictionaryGetTypeID() {
                    continue;
                }
                let bounds = CFDictionaryGetValue(dictionary, bounds_key.0);
                if bounds.is_null() || CFGetTypeID(bounds) != CFDictionaryGetTypeID() {
                    continue;
                }
                let mut rect = Rect::default();
                if !CGRectMakeWithDictionaryRepresentation(bounds, &mut rect) {
                    continue;
                }
                let (Some(pid), Some(layer)) =
                    (number(dictionary, &pid_key), number(dictionary, &layer_key))
                else {
                    continue;
                };
                if !pid.is_finite()
                    || !(0.0..=f64::from(u32::MAX)).contains(&pid)
                    || !layer.is_finite()
                    || !(f64::from(i32::MIN)..=f64::from(i32::MAX)).contains(&layer)
                {
                    continue;
                }
                let item = Metadata {
                    pid: pid as u32,
                    layer: layer as i32,
                    alpha: number(dictionary, &alpha_key).unwrap_or(1.0),
                    on_screen: on_screen(dictionary, &visible_key),
                    x: rect.origin.x,
                    y: rect.origin.y,
                    width: rect.size.width,
                    height: rect.size.height,
                };
                if let Some(bounds) = candidate(item, own_pid, popup_layer) {
                    result.push(bounds);
                }
            }
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn initial_pointer_uses_common_coordinates_for_retina_and_mixed_dpi() {
        assert_eq!(
            cursor_in_image(
                (-1800.0, 300.0),
                1.0,
                (-1920.0, 0.0),
                1.0,
                (1920, 1080),
                (1920, 1080)
            ),
            Some(ImagePoint { x: 120, y: 300 })
        );
        assert_eq!(
            cursor_in_image(
                (800.0, 600.0),
                2.0,
                (200.0, 100.0),
                2.0,
                (2000, 1200),
                (2000, 1200)
            ),
            Some(ImagePoint { x: 600, y: 500 })
        );
        // Tao uses the primary display scale for cursor but the target's for origin.
        assert_eq!(
            cursor_in_image(
                (-2600.0, 300.0),
                2.0,
                (-1400.0, 0.0),
                1.0,
                (1400, 900),
                (2800, 1800)
            ),
            Some(ImagePoint { x: 200, y: 300 })
        );
        for cursor in [(100.0, 0.0), (-1.0, 0.0), (0.0, 80.0), (f64::NAN, 0.0)] {
            assert_eq!(
                cursor_in_image(cursor, 1.0, (0.0, 0.0), 1.0, (100, 80), (200, 160)),
                None
            );
        }
        assert_eq!(
            cursor_in_image((1.0, 1.0), 0.0, (0.0, 0.0), 1.0, (100, 80), (200, 160)),
            None
        );
    }
    #[test]
    fn ax_geometry_is_clipped_to_saved_host_and_must_contain_the_pointer() {
        let host = crate::imaging::Rect {
            x: 100,
            y: 20,
            width: 400,
            height: 160,
        };
        let element = ElementBounds {
            x: -1380.0,
            y: 20.0,
            width: 100.0,
            height: 50.0,
        };
        let map = |bounds, point| {
            element_in_image(bounds, (-1440, 0), (1440, 900), (2880, 1800), host, point)
        };
        assert_eq!(
            map(element, ImagePoint { x: 200, y: 60 }),
            Some(crate::imaging::Rect {
                x: 120,
                y: 40,
                width: 200,
                height: 100
            })
        );
        assert_eq!(
            map(
                ElementBounds {
                    x: -1500.0,
                    width: 300.0,
                    ..element
                },
                ImagePoint { x: 200, y: 60 }
            ),
            Some(crate::imaging::Rect {
                x: 100,
                y: 40,
                width: 380,
                height: 100
            })
        );
        assert_eq!(map(element, ImagePoint { x: 450, y: 60 }), None);
        assert_eq!(
            map(
                ElementBounds {
                    x: f64::NAN,
                    ..element
                },
                ImagePoint { x: 200, y: 60 }
            ),
            None
        );
        assert!(!can_query_element(false, 7, (10.0, 20.0)));
        assert!(!can_query_element(true, 0, (10.0, 20.0)));
        assert!(can_query_element(true, 7, (-100.0, 20.0)));
    }
    fn normal() -> Metadata {
        Metadata {
            pid: 7,
            layer: 0,
            alpha: 1.0,
            on_screen: true,
            x: -1400.0,
            y: 20.0,
            width: 800.0,
            height: 600.0,
        }
    }
    #[test]
    fn geometry_needs_neither_a_title_nor_a_sharing_permission() {
        // Metadata deliberately has no title/app-name/sharing-state fields.
        assert_eq!(
            candidate(normal(), 42, 101),
            Some(WindowBounds {
                native: None,
                pid: 7,
                x: -1400,
                y: 20,
                width: 800,
                height: 600
            })
        );
        let panels = [0, 3, 8, 101].map(|layer| Metadata { layer, ..normal() });
        assert_eq!(
            panels
                .into_iter()
                .filter_map(|item| candidate(item, 42, 101))
                .count(),
            4
        );
    }
    #[test]
    fn rejects_own_hidden_desktop_and_high_overlay_windows() {
        for item in [
            Metadata {
                pid: 42,
                ..normal()
            },
            Metadata {
                on_screen: false,
                ..normal()
            },
            Metadata {
                alpha: 0.0,
                ..normal()
            },
            Metadata {
                alpha: f64::NAN,
                ..normal()
            },
            Metadata {
                layer: -1,
                ..normal()
            },
            Metadata {
                layer: 102,
                ..normal()
            },
            Metadata {
                width: 0.0,
                ..normal()
            },
            Metadata {
                y: f64::INFINITY,
                ..normal()
            },
            Metadata {
                x: f64::from(i32::MAX),
                ..normal()
            },
        ] {
            assert_eq!(candidate(item, 42, 101), None);
        }
    }
    #[test]
    fn preserves_front_to_back_order_and_rounds_fractional_bounds_outward() {
        let windows = [
            Metadata {
                x: -20.25,
                y: 5.25,
                width: 60.5,
                height: 40.5,
                layer: 3,
                ..normal()
            },
            normal(),
        ];
        let result: Vec<_> = windows
            .into_iter()
            .filter_map(|item| candidate(item, 42, 101))
            .collect();
        assert_eq!(
            result[0],
            WindowBounds {
                native: None,
                pid: 7,
                x: -21,
                y: 5,
                width: 62,
                height: 41
            }
        );
        assert_eq!(result[1].width, 800);
    }
}
