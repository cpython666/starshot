"""Run the opt-in native executable against original fixtures, never a desktop capture.
Build: cargo build --locked --manifest-path src-tauri/Cargo.toml --features windows-uia-smoke,tauri/custom-protocol
Then: python tests/windows-uia-acceptance.py src-tauri/target/debug/starshot.exe
"""
import ctypes
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
EXE = Path(sys.argv[1]).resolve()
OUT = ROOT / 'artifacts/windows-uia'
FIXTURE = OUT / 'fixture.exe'
OUT.mkdir(parents=True, exist_ok=True)
NO_WINDOW = 0x08000000


def load(path):
    return json.loads(path.read_text(encoding='utf-8-sig'))


def ready(path, revision=None):
    start = time.monotonic()
    while time.monotonic() - start < 10:
        try:
            data = load(path)
            if revision is None or data['revision'] == revision:
                return data
        except (OSError, ValueError):
            pass
        time.sleep(.05)
    raise RuntimeError(f'Fixture not ready: {path}')


def launch_fixture(directory, monitor):
    directory.mkdir(parents=True, exist_ok=True)
    for name in ['fixture.json', 'block', 'command.txt']:
        (directory / name).unlink(missing_ok=True)
    process = subprocess.Popen([str(FIXTURE), str(directory), str(monitor)], creationflags=NO_WINDOW)
    try:
        data = ready(directory / 'fixture.json')
        return process, data
    except BaseException:
        process.kill(); process.wait(); raise


def run(mode, source, destination):
    start = time.monotonic()
    with (destination.with_suffix('.stderr.txt')).open('w', encoding='utf-8') as error:
        p = subprocess.Popen([str(EXE), '--starshot-uia-' + mode, str(source), str(destination)],
                             stdout=subprocess.DEVNULL, stderr=error, creationflags=NO_WINDOW)
        try:
            code = p.wait(timeout=45)
        except subprocess.TimeoutExpired:
            subprocess.run(['taskkill', '/PID', str(p.pid), '/T', '/F'], capture_output=True)
            raise
    assert code == 0, (mode, code, destination)
    return load(destination), round((time.monotonic() - start) * 1000, 2)


def edge_error(a, b):
    return max(
        abs(a['x']-b['x']), abs(a['y']-b['y']),
        abs(a['x']+a['width']-b['x']-b['width']), abs(a['y']+a['height']-b['y']-b['height']))


summary = {'binarySha256': hashlib.sha256(EXE.read_bytes()).hexdigest(), 'screens': [], 'failures': []}
count = ctypes.windll.user32.GetSystemMetrics(80)  # SM_CMONITORS
for monitor in range(count):
    directory = OUT / f'monitor-{monitor}'
    process, data = launch_fixture(directory, monitor)
    try:
        baseline, _ = run('probe', directory/'fixture.json', directory/'baseline.json')
        overlay, wall = run('overlay', directory/'fixture.json', directory/'overlay.json')
        (directory/'command.txt').write_text('scroll', encoding='ascii')
        scrolled = ready(directory/'fixture.json', 1)
        scroll_result, _ = run('probe', directory/'fixture.json', directory/'scroll.json')
        (directory/'command.txt').write_text('rebuild', encoding='ascii')
        rebuilt = ready(directory/'fixture.json', 2)
        old = dict(data, savedTarget=baseline['target'])
        (directory/'old-target-input.json').write_text(json.dumps(old), encoding='utf-8')
        stale, _ = run('probe', directory/'old-target-input.json', directory/'stale.json')
        fresh, _ = run('probe', directory/'fixture.json', directory/'rebuilt.json')
        # Block ONLY our original provider after all ordinary fixture checks.
        blocked_input = dict(rebuilt, points=[p for p in rebuilt['points'] if p['id'] == 'blocker']*3)
        (directory/'blocked-input.json').write_text(json.dumps(blocked_input), encoding='utf-8')
        (directory/'block').write_text('block original provider', encoding='ascii')
        blocked, _ = run('probe', directory/'blocked-input.json', directory/'blocked.json')
        blocked_input['cancelAfterMs'] = 60
        (directory/'cancel-input.json').write_text(json.dumps(blocked_input), encoding='utf-8')
        cancelled, _ = run('probe', directory/'cancel-input.json', directory/'cancelled.json')
        # Actual native WebView remains responsive while all geometry calls time out.
        blocked_overlay, _ = run('overlay', directory/'fixture.json', directory/'blocked-overlay.json')
        # Explicit cardinalities: no empty all() may pass an acceptance group.
        for result in [baseline, scroll_result, stale, fresh]:
            assert len(result['rows']) == 14
            assert sorted((r['id'], r['repeat']) for r in result['rows']) == sorted((p['id'], repeat) for p in data['points'] for repeat in range(2))
        for result in [blocked, cancelled]:
            assert len(result['rows']) == 6
            assert all(r['id']=='blocker' for r in result['rows'])
        assert len(overlay.get('rows', [])) == 7
        assert sorted(r['id'] for r in overlay['rows']) == sorted(p['id'] for p in data['points'])
        screen = {'monitor': monitor, 'fixtureDpi': data['dpi'], 'fixtureDpiSet': data['dpiSet'], 'fixtureThreadDpiSet':data['threadDpiSet'], 'fixturePmv2':data['pmv2'], 'bounds':data['screen'],
                  'overlayWallMs':wall,'baseline':[], 'blockedStatuses':[r['reply']['status'] for r in blocked['rows']],
                  'cancelledStatuses':[r['reply']['status'] for r in cancelled['rows']],
                  'staleStatuses':[r['reply']['status'] for r in stale['rows']], 'checks':{}}
        for row in baseline['rows']:
            candidates = row['reply']['candidates']
            screen['baseline'].append({'id':row['id'],'repeat':row['repeat'],'status':row['reply']['status'],
                                      'errorPx':edge_error(candidates[0],row['expected']) if candidates else None,
                                      'pmv2':row['reply']['pmv2'],'wallMs':row['wallMs']})
        semantic_ids = ['native-button', 'edit', 'labeled-button', 'scroll-button', 'blocker']
        screen['checks']['semantic'] = all(r['errorPx'] == 0 for r in screen['baseline'] if r['id'] in semantic_ids)
        screen['checks']['overlap_refused'] = all(r['reply']['status']=='ambiguous-overlap' for r in baseline['rows'] if r['id']=='overlap')
        screen['checks']['stale_refused'] = all(s=='stale-target' for s in screen['staleStatuses'])
        screen['checks']['timeout'] = all(s=='timeout' for s in screen['blockedStatuses'])
        screen['checks']['cancel'] = all(s=='cancelled' for s in screen['cancelledStatuses'])
        screen['checks']['no_helper_accumulation'] = all(r['after']['helperChildren']==0 for result in [baseline,blocked,cancelled] for r in result['rows'])
        screen['checks']['no_thread_accumulation'] = all(r['after']['threads']<=result['rows'][0]['before']['threads'] for result in [blocked,cancelled] for r in result['rows'])
        screen['checks']['native_overlay_ipc'] = overlay.get('error') is None and len(overlay.get('rows',[]))==7
        def local(rect):
            return dict(rect,x=rect['x']-data['screen']['x'],y=rect['y']-data['screen']['y'])
        expected = {p['id']:local(p['expected']) for p in data['points']}
        screen['checks']['overlay_geometry'] = all(r['candidates'] and edge_error(r['candidates'][0],expected[r['id']])<=1 for r in overlay.get('rows',[]) if r['id'] in semantic_ids)
        screen['checks']['stationary_pointer'] = overlay.get('stationary') is not None and edge_error(overlay['stationary'],expected['native-button'])<=1
        screen['checks']['rapid_final_point'] = overlay.get('rapidFinal') is not None and edge_error(overlay['rapidFinal'],expected['edit'])<=1
        screen['checks']['scroll_fresh_geometry'] = all(r['reply']['candidates'] and edge_error(r['reply']['candidates'][0],r['expected'])<=1 for r in scroll_result['rows'] if r['id']=='scroll-button')
        screen['checks']['rebuilt_fresh_geometry'] = all(r['reply']['candidates'] and edge_error(r['reply']['candidates'][0],r['expected'])<=1 for r in fresh['rows'] if r['id']=='labeled-button')
        screen['checks']['dpi'] = data['pmv2'] and all(r['pmv2'] for r in screen['baseline'])
        screen['checks']['overlay_dpi'] = overlay.get('dpi',{}).get('processPmv2',False) and overlay.get('dpi',{}).get('reportThreadPmv2',False)
        screen['checks']['tab_stable'] = overlay.get('tabBefore') is not None and overlay.get('tabBefore')==overlay.get('tabAfter')
        parents = overlay.get('parentPath', [])
        children = overlay.get('childPath', [])
        def encloses(a, b):
            return a is not None and b is not None and a['x'] <= b['x']+.6 and a['y'] <= b['y']+.6 and a['x']+a['width'] >= b['x']+b['width']-.6 and a['y']+a['height'] >= b['y']+b['height']-.6
        def unique_path(path):
            return [r for i,r in enumerate(path) if i==0 or r!=path[i-1]]
        screen['checks']['parent_path'] = len(parents)==11 and all(encloses(a,b) for a,b in zip(parents[1:],parents)) and edge_error(parents[0],expected['labeled-button'])<=1
        screen['checks']['parent_stops_at_screen'] = len(parents)==11 and parents[-1]==parents[-2] and edge_error(parents[-1],dict(x=0,y=0,width=data['screen']['width'],height=data['screen']['height']))<=1
        screen['checks']['child_retraces_parent_path'] = len(children)==11 and unique_path(children)==list(reversed(unique_path(parents))) and children[-1]==children[-2]
        screen['checks']['late_refused'] = overlay.get('invalidated',{}).get('candidates')==[]
        screen['checks']['blocked_overlay_responsive'] = blocked_overlay.get('error') is None and blocked_overlay.get('eventLoopTicksDuringQuery',0)>10
        screen['checks']['cancel_and_exit_while_blocked'] = blocked_overlay.get('cancelDuringQueryMs',9999)<250 and blocked_overlay.get('cancelledQueryEmpty',False) and blocked_overlay.get('exitViaProductionGuard',False)
        summary['screens'].append(screen)
    finally:
        process.kill();process.wait(timeout=5)
    # A new original provider answers after the blocked one/helper were terminated.
    process, recovered = launch_fixture(directory/'recovery', monitor)
    try:
        recovery,_ = run('probe',directory/'recovery/fixture.json',directory/'recovery.json')
        assert len(recovery['rows']) == 14
        summary['screens'][-1]['checks']['recovery'] = any(r['reply']['status']=='ok' for r in recovery['rows'])
    finally:
        process.kill();process.wait(timeout=5)
for screen in summary['screens']:
    for check, passed in screen['checks'].items():
        if not passed: summary['failures'].append({'monitor':screen['monitor'],'check':check})
(OUT/'summary.json').write_text(json.dumps(summary,ensure_ascii=False,indent=2),encoding='utf-8')
print(json.dumps(summary,ensure_ascii=False,indent=2))
assert not summary['failures'], summary['failures']
