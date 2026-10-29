// Native WebKit smoke test of the built capture UI, using only original pixels
// and mocked IPC. This does not test screen capture, AX access, or installation.
// Run from the repository root after `npm run build`:
//   mkdir -p artifacts/hover-detection
//   xcrun swiftc tests/macos-capture-webview-smoke.swift -o artifacts/hover-detection/macos-capture-webview-smoke
//   artifacts/hover-detection/macos-capture-webview-smoke dist > artifacts/hover-detection/macos-capture-webview-smoke.jsonl 2> artifacts/hover-detection/macos-capture-webview-smoke.stderr.log
// Each case opens the production index through tauri://, discovers hashed assets
// normally, and has a six-second deadline. No screenshot or clipboard API is used.
import AppKit
import WebKit

final class CaptureSmoke: NSObject, NSApplicationDelegate, WKURLSchemeHandler, WKScriptMessageHandler, WKNavigationDelegate {
    let root: URL
    let cases = ["startup-gate", "startup-failure", "module-worker", "worker-constructor-fallback", "worker-runtime-fallback", "native-element", "native-stale-tab", "worker-stale-tab"]
    var caseIndex = 0
    var view: WKWebView?
    var window: NSWindow?
    var timer: Timer?
    var deadline = Date()
    var evaluating = false
    var workerRequests = 0
    var failed = false
    var finished = false
    var currentCase: String { cases[caseIndex] }

    init(root: URL) { self.root = root; super.init() }
    func log(_ object: [String: Any]) {
        if let data = try? JSONSerialization.data(withJSONObject: object, options: [.sortedKeys]), let line = String(data: data, encoding: .utf8) {
            print(line); fflush(stdout)
        }
    }
    func applicationDidFinishLaunching(_ notification: Notification) {
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 960, height: 600), styleMask: [.titled], backing: .buffered, defer: false)
        window?.title = "Starshot — original fixture WebKit smoke test"
        nextCase()
        window?.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
    }
    func nextCase() {
        workerRequests = 0; evaluating = false
        let config = WKWebViewConfiguration()
        config.websiteDataStore = .nonPersistent()
        config.setURLSchemeHandler(self, forURLScheme: "tauri")
        config.userContentController.add(self, name: "probe")
        config.userContentController.addUserScript(WKUserScript(source: fixtureScript(), injectionTime: .atDocumentStart, forMainFrameOnly: true))
        let next = WKWebView(frame: NSRect(x: 0, y: 0, width: 960, height: 600), configuration: config)
        next.navigationDelegate = self
        view?.configuration.userContentController.removeScriptMessageHandler(forName: "probe")
        view?.stopLoading(); view = next; window?.contentView = next
        log(["event": "case-start", "case": currentCase])
        next.load(URLRequest(url: URL(string: "tauri://localhost/index.html?view=capture")!))
        deadline = Date().addingTimeInterval(6)
        timer = Timer.scheduledTimer(withTimeInterval: 0.1, repeats: true) { [weak self] _ in self?.poll() }
    }
    func fixtureScript() -> String {
        let scenario = String(data: try! JSONSerialization.data(withJSONObject: currentCase, options: [.fragmentsAllowed]), encoding: .utf8)!
        return """
        (() => {
          const scenario = \(scenario), state = { workerPosted: false, workerReply: false, fallback: false, requests: 0, resolved: 0, cycled: false, cycleAt: 0, errors: [], readyCalls: 0, readyError: '', readyCompleted: false, domDecoded: false, workers: 0, decodeGateChecked: false, acknowledgementGateChecked: false };
          const report = body => window.webkit.messageHandlers.probe.postMessage(body);
          window.addEventListener('error', event => { state.errors.push(event.message || 'window-error'); report({event:'window-error',message:event.message||'',file:event.filename||''}); });
          window.addEventListener('unhandledrejection', event => { state.errors.push(String(event.reason)); report({event:'unhandled-rejection',message:String(event.reason)}); });
          document.addEventListener('securitypolicyviolation', event => { state.errors.push('CSP '+event.effectiveDirective); report({event:'csp',directive:event.effectiveDirective,blocked:event.blockedURI}); });
          const decode = HTMLImageElement.prototype.decode;
          HTMLImageElement.prototype.decode = async function() {
            if (!this.classList.contains('capture-source')) return decode.call(this);
            if (scenario === 'startup-gate') {
              await new Promise(resolve => setTimeout(resolve, 180));
              if (state.readyCalls || state.requests || state.workers) state.errors.push('presentation or detection ran before displayed image decode');
              state.decodeGateChecked = true;
            }
            if (scenario === 'startup-failure') throw Error('fixture: displayed image decode failed');
            await decode.call(this); state.domDecoded = this.isConnected;
          };
          if (scenario.startsWith('startup-')) {
            window.requestAnimationFrame = () => 1;
            window.cancelAnimationFrame = () => {};
          }
          window.__TAURI_INTERNALS__ = { invoke: async (command, args) => {
            if (command === 'get_capture_frame') {
              const canvas = document.createElement('canvas'); canvas.width = 1200; canvas.height = 750;
              const ctx = canvas.getContext('2d'); ctx.fillStyle = '#ffffff'; ctx.fillRect(0,0,1200,750);
              ctx.fillStyle = '#2c3448'; ctx.fillRect(120,140,240,160);
              return {dataUrl:canvas.toDataURL('image/png'),width:1200,height:750,scaleFactor:1,pointer:{x:200,y:200},elementDetection:scenario.startsWith('native-')||scenario.startsWith('startup-'),regions:[{x:40,y:40,width:1000,height:650,label:'模拟窗口'}]};
            }
            if (command === 'capture_overlay_ready') {
              state.readyCalls++;
              if (args?.error) { state.readyError = args.error; return; }
              const image = document.querySelector('.capture-source');
              if (!state.domDecoded || !image?.isConnected || !image.complete || image.naturalWidth !== 1200 || document.querySelector('.capture-loading') || !document.querySelector('.capture-selection')) state.errors.push('native presentation ran before decoded image and committed UI');
              if (scenario === 'startup-gate') {
                await new Promise(resolve => setTimeout(resolve, 180));
                if (state.requests || state.workers) state.errors.push('detection ran before native presentation acknowledgement');
                state.acknowledgementGateChecked = true;
              }
              state.readyCompleted = true; return;
            }
            if (command === 'invalidate_capture_element') return;
            if (command === 'get_capture_element') {
              if (!state.readyCompleted) state.errors.push('native query ran before presentation acknowledgement');
              state.requests++;
              if (args?.point?.x !== 200 || args?.point?.y !== 200) state.errors.push('initial pointer was not preserved');
              await new Promise(resolve => setTimeout(resolve, scenario === 'native-stale-tab' ? 400 : 10));
              state.resolved++;
              return [{x:160,y:180,width:80,height:30,label:'模拟控件'}];
            }
            throw new Error('Unexpected IPC: '+command);
          }};
          const NativeWorker = window.Worker;
          if (scenario.startsWith('startup-')) {
            window.Worker = class { constructor(...args) { state.workers++; if (!state.readyCompleted) state.errors.push('worker ran before presentation acknowledgement'); return new NativeWorker(...args); } };
          } else if (scenario === 'worker-constructor-fallback') {
            window.Worker = class { constructor() { state.fallback = true; throw new Error('Intentional fixture constructor failure'); } };
          } else if (scenario === 'worker-runtime-fallback') {
            window.Worker = class { postMessage() { setTimeout(() => { state.fallback = true; this.onerror?.(new Event('error')); }, 10); } terminate() {} };
          } else if (scenario === 'worker-stale-tab') {
            window.Worker = class {
              constructor(...args) {
                this.worker = new NativeWorker(...args);
                this.worker.onmessage = event => setTimeout(() => { state.workerReply = true; this.onmessage?.(event); }, 400);
                this.worker.onerror = event => this.onerror?.(event);
              }
              postMessage(...args) { state.workerPosted = true; this.worker.postMessage(...args); }
              terminate() { this.worker.terminate(); }
            };
          }
          window.__captureSmoke = { sample() {
            const stage = document.querySelector('.capture-stage');
            const dimensions = document.querySelector('.capture-dimensions strong')?.textContent?.trim() || '';
            const label = document.querySelector('.capture-dimensions span')?.textContent?.trim() || '';
            const pointer = document.querySelector('.capture-magnifier small')?.textContent?.trim() || '';
            const error = document.querySelector('[role=alert]')?.textContent?.trim() || '';
            const snapshot = {dimensions,label,pointer,error,ready:!!stage&&!document.querySelector('.capture-loading'),requests:state.requests,resolved:state.resolved,readyCalls:state.readyCalls,domDecoded:state.domDecoded,decodeGateChecked:state.decodeGateChecked,acknowledgementGateChecked:state.acknowledgementGateChecked};
            if (scenario === 'startup-failure') {
              if (!state.readyError) return snapshot;
              const passed = state.readyCalls === 1 && state.readyError.includes('fixture: displayed image decode failed') && !state.readyCompleted && !state.domDecoded && !state.requests && !state.workers && !state.errors.length;
              return {...snapshot,result:passed?'pass':'fail',reason:passed?'':state.errors.join('; ')||'failed initialization was not reported without presentation/detection'};
            }
            if (state.errors.length || error) return {...snapshot,result:'fail',reason:state.errors.join('; ')||error};
            if (!stage) return snapshot;
            if (scenario === 'native-stale-tab' && !state.cycled && state.requests && dimensions === '240 × 160') {
              window.dispatchEvent(new KeyboardEvent('keydown',{key:'Tab',bubbles:true})); state.cycled = true; state.cycleAt = performance.now(); return snapshot;
            }
            if (scenario === 'worker-stale-tab' && !state.cycled && state.workerPosted && !state.workerReply && dimensions === '1000 × 650') {
              window.dispatchEvent(new KeyboardEvent('keydown',{key:'Tab',bubbles:true})); state.cycled = true; state.cycleAt = performance.now(); return snapshot;
            }
            let expected = '240 × 160', complete = dimensions === expected;
            if (scenario === 'startup-gate') { expected = '80 × 30'; complete = dimensions === expected && state.resolved > 0 && state.decodeGateChecked && state.acknowledgementGateChecked && state.readyCompleted; }
            else if (scenario === 'native-element') { expected = '80 × 30'; complete = dimensions === expected && state.resolved > 0; }
            else if (scenario === 'native-stale-tab') { expected = '1000 × 650'; complete = state.cycled && state.resolved > 0 && performance.now()-state.cycleAt > 100; }
            else if (scenario === 'worker-stale-tab') { expected = '1200 × 750'; complete = state.cycled && state.workerReply && performance.now()-state.cycleAt > 100; }
            if (scenario.includes('fallback')) complete = complete && state.fallback;
            if (!complete) return snapshot;
            const passed = dimensions === expected && pointer === '200, 200';
            return {...snapshot,result:passed?'pass':'fail',reason:passed?'':'expected '+expected+' at pointer 200, 200'};
          }};
        })();
        """
    }
    func poll() {
        if finished { return }
        if Date() > deadline { finishCase(false, ["reason": "six-second deadline exceeded"]); return }
        if evaluating { return }
        guard let activeView = view else { return }
        evaluating = true
        let index = caseIndex
        activeView.evaluateJavaScript("window.__captureSmoke?.sample() ?? {ready:false}") { [weak self] value, error in
            guard let self, !self.finished, index == self.caseIndex else { return }
            self.evaluating = false
            if let error { self.finishCase(false, ["reason": String(describing: error)]); return }
            guard var snapshot = value as? [String: Any] else { return }
            if let result = snapshot["result"] as? String {
                let requiresWorker = !self.currentCase.contains("fallback") && self.currentCase != "startup-failure"
                let success = result == "pass" && (!requiresWorker || self.workerRequests > 0)
                if result == "pass" && !success { snapshot["reason"] = "production Worker was never requested through tauri scheme" }
                self.finishCase(success, snapshot)
            }
        }
    }
    func finishCase(_ success: Bool, _ snapshot: [String: Any]) {
        guard !finished else { return }
        timer?.invalidate(); timer = nil
        log(["event": "case-result", "case": currentCase, "result": success ? "pass" : "fail", "workerRequests": workerRequests, "state": snapshot])
        failed = failed || !success
        caseIndex += 1
        if caseIndex < cases.count { nextCase(); return }
        finished = true
        view?.stopLoading(); view?.configuration.userContentController.removeScriptMessageHandler(forName: "probe")
        window?.close()
        log(["event": "finished", "result": failed ? "fail" : "pass", "cases": cases.count])
        NSApp.stop(nil)
        if let event = NSEvent.otherEvent(with: .applicationDefined, location: .zero, modifierFlags: [], timestamp: 0, windowNumber: 0, context: nil, subtype: 0, data1: 0, data2: 0) { NSApp.postEvent(event, atStart: true) }
    }
    func webView(_ webView: WKWebView, start task: WKURLSchemeTask) {
        guard let url = task.request.url else { task.didFailWithError(NSError(domain: "CaptureSmoke", code: 400)); return }
        let path = url.path == "/" ? "/index.html" : url.path
        let file = root.appendingPathComponent(String(path.dropFirst())).standardizedFileURL.resolvingSymlinksInPath()
        guard file.path.hasPrefix(root.path + "/"), let data = try? Data(contentsOf: file) else { task.didFailWithError(NSError(domain: "CaptureSmoke", code: 404)); return }
        if webView === view, path.contains("capture-regions.worker"), path.hasSuffix(".js") {
            workerRequests += 1; log(["event": "worker-request", "case": currentCase, "path": path])
        }
        let mime = path.hasSuffix(".html") ? "text/html" : path.hasSuffix(".css") ? "text/css" : path.hasSuffix(".js") ? "text/javascript" : path.hasSuffix(".svg") ? "image/svg+xml" : "application/octet-stream"
        var headers = ["Content-Type": "\(mime); charset=utf-8", "Content-Length": "\(data.count)"]
        if path.hasSuffix(".html") {
            headers["Content-Security-Policy"] = "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; connect-src ipc: http://ipc.localhost; object-src 'none'; frame-src 'none'; base-uri 'none'"
        }
        task.didReceive(HTTPURLResponse(url: url, statusCode: 200, httpVersion: "HTTP/1.1", headerFields: headers)!)
        task.didReceive(data); task.didFinish()
    }
    func webView(_ webView: WKWebView, stop task: WKURLSchemeTask) {}
    func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: Error) { if webView === view { finishCase(false, ["reason": String(describing: error)]) } }
    func userContentController(_ userContentController: WKUserContentController, didReceive message: WKScriptMessage) {
        guard message.webView === view, let body = message.body as? [String: Any] else { return }
        log(["event": "page-log", "case": currentCase, "data": body])
    }
}

guard CommandLine.arguments.count == 2 else { fputs("Usage: macos-capture-webview-smoke /path/to/dist\n", stderr); exit(2) }
let root = URL(fileURLWithPath: CommandLine.arguments[1], isDirectory: true).standardizedFileURL.resolvingSymlinksInPath()
guard FileManager.default.fileExists(atPath: root.appendingPathComponent("index.html").path) else { fputs("Missing dist/index.html; run npm run build first.\n", stderr); exit(2) }
let app = NSApplication.shared
app.setActivationPolicy(.accessory)
let smoke = CaptureSmoke(root: root)
app.delegate = smoke
app.run()
exit(smoke.failed ? 1 : 0)
