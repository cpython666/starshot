// Original AppKit geometry fixture for cross-process AX selection checks.
// Build: xcrun swiftc tests/macos-panel-fixture.swift -o /tmp/starshot-panel-fixture
// Run the binary, read its single JSON readiness line, then terminate with SIGTERM.
// No screenshots, clipboard access, permission changes, or third-party app reads.
import AppKit
import Darwin

private struct FixtureRegion {
    let name: String
    let rect: NSRect
    let color: NSColor
}

private final class FixtureView: NSView {
    var regions: [FixtureRegion] = []
    // The first matching target is the deepest element. A zero-height AX wrapper
    // deliberately has a separate drawn hit region to exercise parent fallback.
    var targets: [(rect: NSRect, element: NSAccessibilityElement)] = []

    override func draw(_ dirtyRect: NSRect) {
        NSColor.white.setFill()
        bounds.fill()
        for region in regions {
            region.color.setFill()
            region.rect.fill()
            let label = region.name as NSString
            label.draw(at: NSPoint(x: region.rect.minX + 8, y: region.rect.minY + 8),
                       withAttributes: [.font: NSFont.systemFont(ofSize: 13),
                                        .foregroundColor: NSColor.black])
        }
    }

    override func accessibilityHitTest(_ point: NSPoint) -> Any? {
        guard let window else { return super.accessibilityHitTest(point) }
        let local = convert(window.convertPoint(fromScreen: point), from: nil)
        return targets.first { $0.rect.contains(local) }?.element
            ?? super.accessibilityHitTest(point)
    }
}

private final class FixtureDelegate: NSObject, NSApplicationDelegate {
    private var window: NSWindow!
    private var view: FixtureView!
    // NSAccessibilityElement instances must remain owned by their UI vendor.
    private var elements: [NSAccessibilityElement] = []

    private func screenRect(_ rect: NSRect) -> NSRect {
        window.convertToScreen(view.convert(rect, to: nil))
    }

    private func cgRect(_ rect: NSRect) -> [String: Double] {
        // AX positions and CGWindow bounds use top-left global screen points;
        // AppKit accessibilityFrame uses bottom-left global screen points.
        let primaryTop = NSScreen.screens.first!.frame.maxY
        return ["x": rect.minX, "y": primaryTop - rect.maxY,
                "width": rect.width, "height": rect.height]
    }

    private func element(_ name: String, _ rect: NSRect,
                         role: NSAccessibility.Role = .group,
                         parent: Any) -> NSAccessibilityElement {
        let node = NSAccessibilityElement()
        node.setAccessibilityElement(true)
        node.setAccessibilityRole(role)
        node.setAccessibilityLabel(name)
        node.setAccessibilityFrame(screenRect(rect))
        node.setAccessibilityParent(parent)
        node.setAccessibilityChildren([])
        elements.append(node)
        return node
    }

    private func testCase(_ name: String, point: NSPoint,
                          expected: [NSRect]) -> [String: Any] {
        let screenPoint = window.convertPoint(toScreen: view.convert(point, to: nil))
        let primaryTop = NSScreen.screens.first!.frame.maxY
        return ["name": name,
                "point": ["x": screenPoint.x, "y": primaryTop - screenPoint.y],
                "expected": expected.map { cgRect(screenRect($0)) }]
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 800, height: 520),
                          styleMask: [.titled, .closable],
                          backing: .buffered, defer: false)
        window.title = "Starshot Original Panel Fixture"
        window.isReleasedWhenClosed = false
        window.isMovable = false
        let screen = NSScreen.screens.first!
        window.setFrameTopLeftPoint(NSPoint(x: screen.visibleFrame.minX + 60,
                                           y: screen.visibleFrame.maxY - 60))
        view = FixtureView(frame: NSRect(x: 0, y: 0, width: 800, height: 520))
        view.setAccessibilityElement(false)
        window.contentView = view

        let sidebarRect = NSRect(x: 0, y: 0, width: 220, height: 460)
        let editorRect = NSRect(x: 220, y: 0, width: 580, height: 460)
        let toolbarRect = NSRect(x: 0, y: 460, width: 800, height: 60)
        let rowRect = NSRect(x: 16, y: 344, width: 188, height: 56)
        let buttonRect = NSRect(x: 32, y: 356, width: 136, height: 32)
        let fallbackRowRect = NSRect(x: 16, y: 260, width: 188, height: 56)
        let wrapperHitRect = NSRect(x: 32, y: 272, width: 136, height: 32)
        let wrapperAXRect = NSRect(x: 32, y: 288, width: 136, height: 0)
        let editorBodyRect = NSRect(x: 248, y: 44, width: 524, height: 360)
        let editorButtonRect = NSRect(x: 268, y: 340, width: 144, height: 40)
        let toolbarButtonRect = NSRect(x: 20, y: 474, width: 120, height: 32)

        let sidebar = element("Sidebar", sidebarRect, parent: window!)
        let editor = element("Editor", editorRect, parent: window!)
        let toolbar = element("Toolbar", toolbarRect, parent: window!)
        let row = element("Navigation row", rowRect, parent: sidebar)
        let button = element("Navigation button", buttonRect, role: .button, parent: row)
        let fallbackRow = element("Fallback row", fallbackRowRect, parent: sidebar)
        let wrapper = element("Zero-height wrapper", wrapperAXRect, parent: fallbackRow)
        let editorBody = element("Editor body", editorBodyRect, parent: editor)
        let editorButton = element("Editor button", editorButtonRect, role: .button,
                                   parent: editorBody)
        let toolbarButton = element("Toolbar button", toolbarButtonRect, role: .button,
                                    parent: toolbar)
        row.setAccessibilityChildren([button])
        fallbackRow.setAccessibilityChildren([wrapper])
        sidebar.setAccessibilityChildren([row, fallbackRow])
        editorBody.setAccessibilityChildren([editorButton])
        editor.setAccessibilityChildren([editorBody])
        toolbar.setAccessibilityChildren([toolbarButton])
        view.setAccessibilityChildren([sidebar, editor, toolbar])
        window.setAccessibilityChildren([sidebar, editor, toolbar])
        view.targets = [(buttonRect, button), (wrapperHitRect, wrapper),
                        (editorButtonRect, editorButton), (toolbarButtonRect, toolbarButton),
                        (rowRect, row), (fallbackRowRect, fallbackRow),
                        (editorBodyRect, editorBody), (sidebarRect, sidebar),
                        (editorRect, editor), (toolbarRect, toolbar)]
        view.regions = [
            FixtureRegion(name: "Sidebar", rect: sidebarRect, color: .init(white: 0.86, alpha: 1)),
            FixtureRegion(name: "Editor", rect: editorRect, color: .init(white: 0.96, alpha: 1)),
            FixtureRegion(name: "Toolbar", rect: toolbarRect, color: .init(white: 0.76, alpha: 1)),
            FixtureRegion(name: "Row", rect: rowRect, color: .init(white: 0.72, alpha: 1)),
            FixtureRegion(name: "Navigation", rect: buttonRect, color: .init(white: 0.94, alpha: 1)),
            FixtureRegion(name: "Fallback row", rect: fallbackRowRect, color: .init(white: 0.72, alpha: 1)),
            FixtureRegion(name: "Zero AX height", rect: wrapperHitRect, color: .init(white: 0.94, alpha: 1)),
            FixtureRegion(name: "Editor body", rect: editorBodyRect, color: .init(white: 0.89, alpha: 1)),
            FixtureRegion(name: "Action", rect: editorButtonRect, color: .init(white: 0.98, alpha: 1)),
            FixtureRegion(name: "Tool", rect: toolbarButtonRect, color: .init(white: 0.94, alpha: 1)),
        ]
        window.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
        view.displayIfNeeded()

        let metadata: [String: Any] = [
            "pid": ProcessInfo.processInfo.processIdentifier,
            "window": cgRect(window.frame),
            "cases": [
                testCase("sidebar-button", point: NSPoint(x: 100, y: 372),
                         expected: [buttonRect, rowRect, sidebarRect]),
                testCase("zero-frame-parent", point: NSPoint(x: 100, y: 288),
                         expected: [fallbackRowRect, sidebarRect]),
                testCase("editor-button", point: NSPoint(x: 340, y: 360),
                         expected: [editorButtonRect, editorBodyRect, editorRect]),
                testCase("editor-body", point: NSPoint(x: 600, y: 200),
                         expected: [editorBodyRect, editorRect]),
                testCase("toolbar-button", point: NSPoint(x: 80, y: 490),
                         expected: [toolbarButtonRect, toolbarRect]),
            ],
        ]
        let data = try! JSONSerialization.data(withJSONObject: metadata, options: [.sortedKeys])
        print(String(data: data, encoding: .utf8)!)
        fflush(stdout)
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { true }
}

let app = NSApplication.shared
private let delegate = FixtureDelegate()
app.setActivationPolicy(.accessory)
app.delegate = delegate
app.run()
