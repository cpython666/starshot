// Injected by the opt-in native acceptance build into its own real WebView2.
// Calls the actual Rust IPC. No mocks, external pages, capture or clipboard APIs.
(async () => {
  if (window.__starshotSmokeStarted) return;
  window.__starshotSmokeStarted = true;
  const fixture = __FIXTURE_JSON__;
  const invoke = (command, args = {}) => window.__TAURI_INTERNALS__.invoke(command, args);
  const wait = ms => new Promise(resolve => setTimeout(resolve, ms));
  const box = () => {
    const node = document.querySelector('.capture-selection');
    if (!node) return null;
    const r = node.getBoundingClientRect();
    return { x: r.x / innerWidth * fixture.screen.width, y: r.y / innerHeight * fixture.screen.height,
      width: r.width / innerWidth * fixture.screen.width, height: r.height / innerHeight * fixture.screen.height };
  };
  const move = p => invoke('native_smoke_move', { index: fixture.points.indexOf(p) });
  const records = { kind: 'real-tauri-webview2', devicePixelRatio, viewport: [innerWidth, innerHeight], screen: fixture.screen, rows: [] };
  try {
    for (let i = 0; i < 150 && (!document.querySelector('.capture-selection') || document.querySelector('.capture-loading')); i++) await wait(50);
    await wait(1300);
    records.stationary = box();
    const initialFrame = await invoke('get_capture_frame');
    records.initialFrame = { pointer: initialFrame.pointer, width: initialFrame.width, height: initialFrame.height };
    records.initialViewport = [innerWidth, innerHeight];
    records.initialSelectionPresent = !!document.querySelector('.capture-selection');
    const points = fixture.points;
    // Move the actual pointer only within OUR original overlay. Mixing synthetic
    // positions with an unmoved OS pointer lets later real events reset selection.
    for (let i = 0; i < 12; i++) { await move(points[i % 2]); await wait(12); }
    await move(points[1]); await wait(1100); records.rapidFinal = box();
    await move(points[2]); await wait(95);
    dispatchEvent(new KeyboardEvent('keydown', { key: 'Tab', bubbles: true }));
    await wait(50); // Vue must commit the Tab selection before taking its baseline.
    records.tabBefore = box(); await wait(1000); records.tabAfter = box();
    // Start a fresh semantic path. Tab expands, stops at the outermost frame;
    // Shift+Tab retraces that frozen path and stops at the original control.
    await move(points[1]); await wait(50); await move(points[2]); await wait(1100);
    records.parentPath = [box()];
    for (let i = 0; i < 10; i++) {
      dispatchEvent(new KeyboardEvent('keydown', { key: 'Tab', bubbles: true }));
      await wait(30); records.parentPath.push(box());
    }
    records.childPath = [box()];
    for (let i = 0; i < 10; i++) {
      dispatchEvent(new KeyboardEvent('keydown', { key: 'Shift', bubbles: true }));
      dispatchEvent(new KeyboardEvent('keydown', { key: 'Tab', shiftKey: true, bubbles: true }));
      dispatchEvent(new KeyboardEvent('keyup', { key: 'Shift', bubbles: true }));
      await wait(30); records.childPath.push(box());
    }
    let generation = 10000;
    for (const p of points) {
      const start = performance.now();
      const candidates = await invoke('get_capture_element', { point: { x: p.x, y: p.y }, generation: ++generation });
      records.rows.push({ id: p.id, candidates, wallMs: performance.now() - start });
    }
    // Revoke an in-flight request through the same production invalidation command.
    const start = performance.now();
    const pending = invoke('get_capture_element', { point: { x: points[0].x, y: points[0].y }, generation: ++generation });
    await wait(20); await invoke('invalidate_capture_element', { generation: ++generation });
    records.invalidated = { candidates: await pending, wallMs: performance.now() - start };
    let ticks = 0; const timer = setInterval(() => ticks++, 10);
    await invoke('get_capture_element', { point: { x: points[0].x, y: points[0].y }, generation: ++generation });
    clearInterval(timer); records.eventLoopTicksDuringQuery = ticks;
    records.error = null;
  } catch (e) { records.error = String(e); }
  await invoke('native_smoke_report', { records });
})();
