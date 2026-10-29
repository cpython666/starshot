// Geometry only, loaded solely by the explicit native acceptance feature.
(() => {
  const invoke = window.__TAURI_INTERNALS__.invoke.bind(window.__TAURI_INTERNALS__);
  window.__starshotSample = (sequence, width, height) => {
    const node = document.querySelector('.capture-selection');
    const r = node?.getBoundingClientRect();
    const box = r ? { x: r.x / innerWidth * width, y: r.y / innerHeight * height,
      width: r.width / innerWidth * width, height: r.height / innerHeight * height } : null;
    return invoke('native_integration_record', { sequence, records: { time: performance.now(),
      viewport: [innerWidth, innerHeight], devicePixelRatio, box, loading: !!document.querySelector('.capture-loading') } });
  };
})();
