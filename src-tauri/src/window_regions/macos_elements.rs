//! Bounded, geometry-only AX traversal within the saved foreground window.
use super::{ElementBounds, WindowBounds};

const MAX_NODES: usize = 128;
const MAX_DEPTH: usize = 24;
const MAX_PARENTS: usize = 32;
const MAX_CANDIDATES: usize = 8;

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Window,
    Container,
    Control,
    Text,
}
struct Info {
    bounds: Option<ElementBounds>,
    kind: Kind,
}
// This small adapter lets the same traversal exercise malformed/slow AX trees in
// tests without requesting access to a real desktop or reading any UI text.
trait Access {
    type Node: Clone + Eq;
    fn active(&self) -> bool;
    fn hit(&mut self, position: (f64, f64)) -> Option<Self::Node>;
    fn window(&mut self, node: &Self::Node) -> Option<Self::Node>;
    fn windows(&mut self) -> Vec<Self::Node>;
    fn info(&mut self, node: &Self::Node) -> Option<Info>;
    fn parent(&mut self, node: &Self::Node) -> Option<Self::Node>;
    fn children(&mut self, node: &Self::Node, limit: usize) -> Option<Vec<Self::Node>>;
}
fn valid(r: ElementBounds) -> bool {
    [r.x, r.y, r.width, r.height, r.x + r.width, r.y + r.height]
        .iter()
        .all(|v| v.is_finite())
        && r.width >= 2.0
        && r.height >= 2.0
}
fn contains(r: ElementBounds, p: (f64, f64)) -> bool {
    valid(r) && p.0 >= r.x && p.1 >= r.y && p.0 < r.x + r.width && p.1 < r.y + r.height
}
fn encloses(outer: ElementBounds, inner: ElementBounds) -> bool {
    outer.x <= inner.x
        && outer.y <= inner.y
        && outer.x + outer.width >= inner.x + inner.width
        && outer.y + outer.height >= inner.y + inner.height
}
fn same(a: ElementBounds, b: ElementBounds) -> bool {
    a.x == b.x && a.y == b.y && a.width == b.width && a.height == b.height
}
fn matches_window(r: ElementBounds, host: ElementBounds) -> bool {
    valid(r)
        && (r.x - host.x).abs() <= 1.0
        && (r.y - host.y).abs() <= 1.0
        && (r.x + r.width - host.x - host.width).abs() <= 1.0
        && (r.y + r.height - host.y - host.height).abs() <= 1.0
}
struct Search<'a, A: Access> {
    access: &'a mut A,
    host: ElementBounds,
    point: (f64, f64),
    visited: usize,
}
impl<A: Access> Search<'_, A> {
    fn info(&mut self, node: &A::Node) -> Option<Info> {
        if !self.access.active() || self.visited >= MAX_NODES {
            return None;
        }
        self.visited += 1;
        self.access.info(node)
    }
    fn usable(&self, info: &Info) -> Option<ElementBounds> {
        info.bounds.filter(|r| {
            info.kind != Kind::Text
                && contains(*r, self.point)
                && encloses(self.host, *r)
                && !same(*r, self.host)
        })
    }
    fn is_host(&mut self, node: &A::Node) -> bool {
        self.info(node).is_some_and(|info| {
            info.kind == Kind::Window && info.bounds.is_some_and(|r| matches_window(r, self.host))
        })
    }
    // A missing geometry wrapper is traversable. Two matching sibling branches
    // are ambiguous: retain their ancestor rather than choosing by array order.
    fn descend(
        &mut self,
        node: A::Node,
        depth: usize,
        seen: &mut Vec<A::Node>,
    ) -> (bool, Vec<ElementBounds>) {
        if depth >= MAX_DEPTH || seen.contains(&node) {
            return (false, Vec::new());
        }
        seen.push(node.clone());
        let Some(mut info) = self.info(&node) else {
            return (false, Vec::new());
        };
        // CGWindow rounds to integer edges; AX may keep fractional coordinates.
        // Normalize only the already matched root, not an out-of-window child.
        if depth == 0
            && info.kind == Kind::Window
            && info.bounds.is_some_and(|r| matches_window(r, self.host))
        {
            info.bounds = Some(self.host);
        }
        let geometry = info.bounds.filter(|r| valid(*r));
        if geometry.is_some_and(|r| !contains(r, self.point) || !encloses(self.host, r)) {
            return (false, Vec::new());
        }
        let own: Vec<_> = self.usable(&info).into_iter().collect();
        let matches = geometry.is_some();
        if info.kind == Kind::Control || info.kind == Kind::Text {
            return (matches, own);
        }
        let Some(children) = self
            .access
            .children(&node, MAX_NODES.saturating_sub(self.visited))
        else {
            return (matches, own);
        };
        let mut chosen = None;
        for child in children {
            if !self.access.active() || self.visited >= MAX_NODES {
                return (matches, own);
            }
            let (hit, path) = self.descend(child, depth + 1, seen);
            if hit {
                if chosen.is_some() {
                    return (matches, own);
                }
                chosen = Some(path);
            }
        }
        if let Some(mut path) = chosen {
            path.extend(own);
            (true, path)
        } else {
            (matches, own)
        }
    }
}
fn collect<A: Access>(access: &mut A, host: WindowBounds, point: (f64, f64)) -> Vec<ElementBounds> {
    let bounds = ElementBounds {
        x: f64::from(host.x),
        y: f64::from(host.y),
        width: f64::from(host.width),
        height: f64::from(host.height),
    };
    if !contains(bounds, point) || !access.active() {
        return Vec::new();
    }
    let mut search = Search {
        access,
        host: bounds,
        point,
        visited: 0,
    };
    let hit = search.access.hit(point);
    let mut root = hit
        .as_ref()
        .and_then(|node| search.access.window(node))
        .filter(|node| search.is_host(node));
    let mut parents = Vec::new();
    let mut direct_kind = None;
    if let Some(hit) = hit {
        let mut next = Some(hit);
        let mut seen = Vec::new();
        for _ in 0..MAX_PARENTS {
            let Some(node) = next else { break };
            if seen.contains(&node) {
                break;
            }
            seen.push(node.clone());
            let Some(info) = search.info(&node) else {
                break;
            };
            direct_kind.get_or_insert(info.kind);
            if info.kind == Kind::Window {
                if info.bounds.is_some_and(|r| matches_window(r, bounds)) {
                    root = Some(node);
                } else {
                    // A point result from another window is not a trusted path.
                    root = None;
                    parents.clear();
                }
                break;
            }
            if let Some(r) = search.usable(&info) {
                parents.push(r);
            }
            // Zero-size, missing and non-containing hit geometries still have
            // meaningful parents (e.g. virtual rows inside a sidebar).
            next = search.access.parent(&node);
        }
    }
    if root.is_none() {
        parents.clear();
        // Only this PID's bounded AXWindows list, never the desktop AX root.
        for window in search.access.windows() {
            if search.is_host(&window) {
                if root.is_some() {
                    return Vec::new(); // Same geometry cannot identify two windows.
                }
                root = Some(window);
            }
        }
    }
    let Some(root) = root else {
        return Vec::new();
    };
    let mut candidates = parents;
    if candidates.is_empty() || direct_kind == Some(Kind::Container) {
        let (_, descendants) = search.descend(root, 0, &mut Vec::new());
        // The direct point/parent path remains preferred over unrelated overlap.
        if candidates.first().is_none_or(|first| {
            descendants
                .first()
                .is_some_and(|child| encloses(*first, *child))
        }) {
            candidates.splice(0..0, descendants);
        }
    }
    // Retain collected ancestors on timeout. Native query discards all results
    // if its generation has changed while an AX call was in progress.
    let Some(first) = candidates.first().copied() else {
        return Vec::new();
    };
    candidates.sort_by(|a, b| (a.width * a.height).total_cmp(&(b.width * b.height)));
    let mut path = vec![first];
    for r in candidates {
        let previous = *path.last().unwrap();
        if !same(previous, r) && encloses(r, previous) {
            path.push(r);
        }
    }
    // Deep wrappers must not consume the entire Tab path before the actual
    // sidebar/panel ancestors. Keep both the smallest controls and outer panels.
    if path.len() > MAX_CANDIDATES {
        path.drain(MAX_CANDIDATES / 2..path.len() - MAX_CANDIDATES / 2);
    }
    path
}

#[cfg(all(feature = "desktop", target_os = "macos"))]
pub fn query(
    host: WindowBounds,
    position: (f64, f64),
    revision: &std::sync::atomic::AtomicU64,
    expected: u64,
) -> Vec<ElementBounds> {
    native::query(host, position, revision, expected)
}

#[cfg(all(feature = "desktop", target_os = "macos"))]
mod native {
    use super::*;
    use std::{
        ffi::{c_char, c_void, CStr},
        sync::atomic::{AtomicU64, Ordering},
        time::{Duration, Instant},
    };
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
    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFRelease(value: Ref);
        fn CFRetain(value: Ref) -> Ref;
        fn CFEqual(a: Ref, b: Ref) -> u8;
        fn CFGetTypeID(value: Ref) -> usize;
        fn CFArrayGetTypeID() -> usize;
        fn CFArrayGetCount(array: Ref) -> isize;
        fn CFArrayGetValueAtIndex(array: Ref, index: isize) -> Ref;
        fn CFStringGetTypeID() -> usize;
        fn CFStringGetCString(value: Ref, buffer: *mut c_char, length: isize, encoding: u32) -> u8;
        fn CFStringCreateWithCString(allocator: Ref, value: *const c_char, encoding: u32) -> Ref;
    }
    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        fn AXIsProcessTrusted() -> bool;
        fn AXUIElementGetTypeID() -> usize;
        fn AXUIElementGetPid(element: Ref, pid: *mut i32) -> i32;
        fn AXUIElementCreateApplication(pid: i32) -> Ref;
        fn AXUIElementSetMessagingTimeout(element: Ref, timeout: f32) -> i32;
        fn AXUIElementCopyElementAtPosition(app: Ref, x: f32, y: f32, value: *mut Ref) -> i32;
        fn AXUIElementCopyAttributeValue(element: Ref, name: Ref, value: *mut Ref) -> i32;
        fn AXUIElementGetAttributeValueCount(element: Ref, name: Ref, count: *mut isize) -> i32;
        fn AXUIElementCopyAttributeValues(
            element: Ref,
            name: Ref,
            start: isize,
            count: isize,
            values: *mut Ref,
        ) -> i32;
        fn AXValueGetTypeID() -> usize;
        fn AXValueGetValue(value: Ref, kind: i32, output: *mut c_void) -> bool;
    }
    struct Owned(Ref);
    impl Drop for Owned {
        fn drop(&mut self) {
            unsafe { CFRelease(self.0) }
        }
    }
    impl Clone for Owned {
        fn clone(&self) -> Self {
            Self(unsafe { CFRetain(self.0) })
        }
    }
    impl PartialEq for Owned {
        fn eq(&self, other: &Self) -> bool {
            unsafe { CFEqual(self.0, other.0) != 0 }
        }
    }
    impl Eq for Owned {}
    fn own(value: Ref) -> Option<Owned> {
        (!value.is_null()).then(|| Owned(value))
    }
    fn key(value: &CStr) -> Option<Owned> {
        own(unsafe { CFStringCreateWithCString(std::ptr::null(), value.as_ptr(), 0x0800_0100) })
    }
    struct Native<'a> {
        app: Owned,
        pid: u32,
        deadline: Instant,
        revision: &'a AtomicU64,
        expected: u64,
    }
    impl Native<'_> {
        fn prepare(&self, node: &Owned) -> Option<()> {
            if !self.active() {
                return None;
            }
            let seconds = self
                .deadline
                .saturating_duration_since(Instant::now())
                .as_secs_f32();
            if seconds <= 0.0 {
                return None;
            }
            (unsafe { AXUIElementSetMessagingTimeout(node.0, seconds) } == 0).then_some(())
        }
        fn attribute(&self, node: &Owned, name: &CStr, type_id: usize) -> Option<Owned> {
            let name = key(name)?;
            self.prepare(node)?;
            let mut value = std::ptr::null();
            let status = unsafe { AXUIElementCopyAttributeValue(node.0, name.0, &mut value) };
            let value = own(value)?;
            (status == 0 && unsafe { CFGetTypeID(value.0) } == type_id).then_some(value)
        }
        fn related(&self, node: &Owned, name: &CStr) -> Option<Owned> {
            self.attribute(node, name, unsafe { AXUIElementGetTypeID() })
        }
        fn array(&self, node: &Owned, name: &CStr, limit: usize) -> Option<Vec<Owned>> {
            if limit == 0 {
                return None;
            }
            let name = key(name)?;
            self.prepare(node)?;
            let mut count = 0;
            if unsafe { AXUIElementGetAttributeValueCount(node.0, name.0, &mut count) } != 0
                || count < 0
                || count as usize > limit
            {
                return None;
            }
            if count == 0 {
                return Some(Vec::new());
            }
            self.prepare(node)?;
            let mut values = std::ptr::null();
            let status =
                unsafe { AXUIElementCopyAttributeValues(node.0, name.0, 0, count, &mut values) };
            let values = own(values)?;
            if status != 0
                || unsafe { CFGetTypeID(values.0) } != unsafe { CFArrayGetTypeID() }
                || unsafe { CFArrayGetCount(values.0) } != count
            {
                return None;
            }
            let mut result = Vec::new();
            for index in 0..count {
                let value = unsafe { CFArrayGetValueAtIndex(values.0, index) };
                if value.is_null()
                    || unsafe { CFGetTypeID(value) } != unsafe { AXUIElementGetTypeID() }
                {
                    return None;
                }
                result.push(Owned(unsafe { CFRetain(value) }));
            }
            Some(result)
        }
        fn geometry(&self, node: &Owned) -> Option<ElementBounds> {
            let position = self.attribute(node, c"AXPosition", unsafe { AXValueGetTypeID() })?;
            let size = self.attribute(node, c"AXSize", unsafe { AXValueGetTypeID() })?;
            let mut point = Point::default();
            let mut dimensions = Size::default();
            if !unsafe { AXValueGetValue(position.0, 1, (&mut point as *mut Point).cast()) }
                || !unsafe { AXValueGetValue(size.0, 2, (&mut dimensions as *mut Size).cast()) }
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
    impl Access for Native<'_> {
        type Node = Owned;
        fn active(&self) -> bool {
            self.revision.load(Ordering::Acquire) == self.expected && Instant::now() < self.deadline
        }
        fn hit(&mut self, point: (f64, f64)) -> Option<Owned> {
            self.prepare(&self.app)?;
            let mut value = std::ptr::null();
            let status = unsafe {
                AXUIElementCopyElementAtPosition(
                    self.app.0,
                    point.0 as f32,
                    point.1 as f32,
                    &mut value,
                )
            };
            let value = own(value)?;
            (status == 0 && unsafe { CFGetTypeID(value.0) } == unsafe { AXUIElementGetTypeID() })
                .then_some(value)
        }
        fn window(&mut self, node: &Owned) -> Option<Owned> {
            self.related(node, c"AXWindow")
        }
        fn windows(&mut self) -> Vec<Owned> {
            self.array(&self.app, c"AXWindows", 32).unwrap_or_default()
        }
        fn info(&mut self, node: &Owned) -> Option<Info> {
            self.prepare(node)?;
            let mut pid = 0;
            if unsafe { AXUIElementGetPid(node.0, &mut pid) } != 0 || pid != self.pid as i32 {
                return None;
            }
            let role = self.attribute(node, c"AXRole", unsafe { CFStringGetTypeID() })?;
            let mut buffer = [0_i8; 96];
            if unsafe {
                CFStringGetCString(
                    role.0,
                    buffer.as_mut_ptr(),
                    buffer.len() as isize,
                    0x0800_0100,
                )
            } == 0
            {
                return None;
            }
            let name = unsafe { CStr::from_ptr(buffer.as_ptr()) };
            let kind = match name.to_bytes() {
                b"AXWindow" => Kind::Window,
                b"AXStaticText" => Kind::Text,
                b"AXImage" | b"AXButton" | b"AXCheckBox" | b"AXRadioButton" | b"AXPopUpButton"
                | b"AXTextField" | b"AXTextArea" | b"AXComboBox" | b"AXSlider"
                | b"AXIncrementor" | b"AXMenuItem" | b"AXLink" => Kind::Control,
                _ => Kind::Container,
            };
            Some(Info {
                bounds: self.geometry(node),
                kind,
            })
        }
        fn parent(&mut self, node: &Owned) -> Option<Owned> {
            self.related(node, c"AXParent")
        }
        fn children(&mut self, node: &Owned, limit: usize) -> Option<Vec<Owned>> {
            self.array(node, c"AXChildren", limit)
        }
    }
    pub fn query(
        host: WindowBounds,
        position: (f64, f64),
        revision: &AtomicU64,
        expected: u64,
    ) -> Vec<ElementBounds> {
        if !super::super::can_query_element(unsafe { AXIsProcessTrusted() }, host.pid, position)
            || revision.load(Ordering::Acquire) != expected
        {
            return Vec::new();
        }
        let Some(app) = own(unsafe { AXUIElementCreateApplication(host.pid as i32) }) else {
            return Vec::new();
        };
        let mut access = Native {
            app,
            pid: host.pid,
            deadline: Instant::now() + Duration::from_millis(120),
            revision,
            expected,
        };
        let result = collect(&mut access, host, position);
        if revision.load(Ordering::Acquire) != expected {
            Vec::new()
        } else {
            result
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Clone)]
    struct Node {
        kind: Kind,
        bounds: Option<ElementBounds>,
        parent: Option<usize>,
        children: Vec<usize>,
        foreign: bool,
    }
    struct Fake {
        nodes: Vec<Node>,
        hit: Option<usize>,
        window: Option<usize>,
        windows: Vec<usize>,
        calls: usize,
        stop: usize,
    }
    impl Access for Fake {
        type Node = usize;
        fn active(&self) -> bool {
            self.calls < self.stop
        }
        fn hit(&mut self, _: (f64, f64)) -> Option<usize> {
            self.hit
        }
        fn window(&mut self, _: &usize) -> Option<usize> {
            self.window
        }
        fn windows(&mut self) -> Vec<usize> {
            self.windows.clone()
        }
        fn info(&mut self, id: &usize) -> Option<Info> {
            self.calls += 1;
            let n = &self.nodes[*id];
            (!n.foreign).then_some(Info {
                bounds: n.bounds,
                kind: n.kind,
            })
        }
        fn parent(&mut self, id: &usize) -> Option<usize> {
            self.nodes[*id].parent
        }
        fn children(&mut self, id: &usize, limit: usize) -> Option<Vec<usize>> {
            (self.nodes[*id].children.len() <= limit).then(|| self.nodes[*id].children.clone())
        }
    }
    fn rect(x: f64, y: f64, w: f64, h: f64) -> ElementBounds {
        ElementBounds {
            x,
            y,
            width: w,
            height: h,
        }
    }
    fn host() -> WindowBounds {
        WindowBounds {
            pid: 7,
            x: 0,
            y: 0,
            width: 1000,
            height: 800,
            native: None,
        }
    }
    fn fixture() -> Fake {
        Fake {
            nodes: vec![
                Node {
                    kind: Kind::Window,
                    bounds: Some(rect(0., 0., 1000., 800.)),
                    parent: None,
                    children: vec![1],
                    foreign: false,
                },
                Node {
                    kind: Kind::Container,
                    bounds: None,
                    parent: Some(0),
                    children: vec![2, 5],
                    foreign: false,
                },
                Node {
                    kind: Kind::Container,
                    bounds: Some(rect(0., 40., 250., 720.)),
                    parent: Some(1),
                    children: vec![3],
                    foreign: false,
                },
                Node {
                    kind: Kind::Control,
                    bounds: Some(rect(30., 100., 180., 40.)),
                    parent: Some(2),
                    children: vec![4],
                    foreign: false,
                },
                Node {
                    kind: Kind::Text,
                    bounds: Some(rect(40., 105., 50., 0.)),
                    parent: Some(3),
                    children: vec![],
                    foreign: false,
                },
                Node {
                    kind: Kind::Container,
                    bounds: Some(rect(250., 40., 750., 720.)),
                    parent: Some(1),
                    children: vec![],
                    foreign: false,
                },
            ],
            hit: Some(4),
            window: Some(0),
            windows: vec![0],
            calls: 0,
            stop: usize::MAX,
        }
    }
    fn sizes(path: Vec<ElementBounds>) -> Vec<(u32, u32)> {
        path.into_iter()
            .map(|r| (r.width as u32, r.height as u32))
            .collect()
    }
    #[test]
    fn zero_height_text_keeps_walking_to_button_and_sidebar_parents() {
        let mut a = fixture();
        assert_eq!(
            sizes(collect(&mut a, host(), (50., 115.))),
            vec![(180, 40), (250, 720)]
        );
        assert!(a.calls <= MAX_NODES);
    }
    #[test]
    fn broad_or_missing_hit_descends_matched_window_through_geometryless_wrapper() {
        for hit in [Some(0), None] {
            let mut a = fixture();
            a.hit = hit;
            assert_eq!(
                sizes(collect(&mut a, host(), (50., 115.))),
                vec![(180, 40), (250, 720)]
            );
        }
        let mut a = fixture();
        a.hit = Some(0);
        assert_eq!(
            sizes(collect(&mut a, host(), (100., 300.))),
            vec![(250, 720)]
        );
    }
    #[test]
    fn overlapping_siblings_fail_back_to_the_known_ancestor() {
        let mut a = fixture();
        a.hit = Some(0);
        a.nodes[5].bounds = Some(rect(20., 40., 750., 720.));
        assert!(collect(&mut a, host(), (50., 115.)).is_empty());
    }
    #[test]
    fn rounded_root_and_unavailable_hit_geometry_keep_the_real_panel() {
        let mut a = fixture();
        a.hit = None;
        a.nodes[0].bounds = Some(rect(-0.5, -0.5, 1001., 801.));
        assert_eq!(
            sizes(collect(&mut a, host(), (50., 115.))),
            vec![(180, 40), (250, 720)]
        );
        let mut a = fixture();
        a.nodes[4].bounds = None;
        assert_eq!(
            sizes(collect(&mut a, host(), (50., 115.))),
            vec![(180, 40), (250, 720)]
        );
    }
    #[test]
    fn deadline_keeps_known_parents_and_ambiguous_windows_are_rejected() {
        let mut a = fixture();
        a.hit = Some(2);
        a.stop = 4;
        assert_eq!(
            sizes(collect(&mut a, host(), (50., 115.))),
            vec![(250, 720)]
        );
        let mut a = fixture();
        a.hit = None;
        a.nodes.push(a.nodes[0].clone());
        a.windows.push(6);
        assert!(collect(&mut a, host(), (50., 115.)).is_empty());
    }
    #[test]
    fn deep_ancestor_path_keeps_inner_controls_and_outer_panels_within_eight() {
        let mut a = fixture();
        a.nodes.truncate(1);
        for i in 0..20 {
            a.nodes.push(Node {
                kind: if i == 0 {
                    Kind::Control
                } else {
                    Kind::Container
                },
                bounds: Some(rect(
                    100. - i as f64,
                    100. - i as f64,
                    20. + 2. * i as f64,
                    20. + 2. * i as f64,
                )),
                parent: Some(if i == 19 { 0 } else { i + 2 }),
                children: Vec::new(),
                foreign: false,
            });
        }
        a.hit = Some(1);
        let path = collect(&mut a, host(), (110., 110.));
        assert_eq!(path.len(), MAX_CANDIDATES);
        assert_eq!(path[0].width, 20.);
        assert_eq!(
            path.iter().map(|r| r.width as u32).collect::<Vec<_>>(),
            vec![20, 22, 24, 26, 52, 54, 56, 58]
        );
        assert!(path
            .windows(2)
            .all(|p| encloses(p[1], p[0]) && !same(p[0], p[1])));
    }
    #[test]
    fn deep_or_wide_subtrees_stop_without_unbounded_enumeration() {
        let mut a = fixture();
        a.hit = None;
        a.nodes[0].children = vec![1; MAX_NODES + 1];
        assert!(collect(&mut a, host(), (50., 115.)).is_empty());
        assert!(a.calls <= MAX_NODES);
        let mut a = fixture();
        a.nodes.truncate(1);
        a.hit = None;
        for i in 0..40 {
            a.nodes.push(Node {
                kind: Kind::Container,
                bounds: None,
                parent: None,
                children: if i == 39 { Vec::new() } else { vec![i + 2] },
                foreign: false,
            });
        }
        assert!(collect(&mut a, host(), (50., 115.)).is_empty());
        assert!(a.calls <= MAX_DEPTH + 1);
    }
    #[test]
    fn cycles_foreign_processes_stale_hosts_and_budgets_are_bounded() {
        let mut a = fixture();
        a.nodes[2].parent = Some(4);
        assert_eq!(
            sizes(collect(&mut a, host(), (50., 115.))),
            vec![(180, 40), (250, 720)]
        );
        assert!(a.calls < 10);
        let mut a = fixture();
        a.hit = Some(0);
        a.nodes[2].foreign = true;
        assert!(collect(&mut a, host(), (50., 115.)).is_empty());
        let mut a = fixture();
        a.nodes[0].bounds = Some(rect(20., 0., 1000., 800.));
        assert!(collect(&mut a, host(), (50., 115.)).is_empty());
        let mut a = fixture();
        a.stop = 0;
        assert!(collect(&mut a, host(), (50., 115.)).is_empty());
        assert_eq!(a.calls, 0);
        let mut a = fixture();
        a.hit = None;
        a.nodes[1].children = vec![1];
        assert!(collect(&mut a, host(), (50., 115.)).is_empty());
        assert!(a.calls <= MAX_NODES);
    }
}
