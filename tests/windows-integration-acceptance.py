"""Production begin_capture with generated pixels; one persistent Tauri parent.
Build the existing fixture and windows-uia-smoke/custom-protocol executable first.
Run: python tests/windows-integration-acceptance.py src-tauri/target/debug/starshot.exe
All pointer positions come from original fixtures; no screenshots or clipboard.
"""
import ctypes
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import time
import traceback

ROOT = Path(__file__).resolve().parents[1]
EXE = Path(sys.argv[1]).resolve()
OUT = ROOT / 'artifacts' / ('windows-integration-' + time.strftime('%Y%m%d-%H%M%S'))
OUT.mkdir(parents=True)
NO_WINDOW = 0x08000000
FIXTURE = ROOT / 'artifacts/windows-uia/fixture.exe'


def read(path):
    return json.loads(path.read_text(encoding='utf-8-sig'))


def wait_file(path, predicate=lambda v: True, timeout=15):
    start = time.monotonic()
    while time.monotonic() - start < timeout:
        try:
            data = read(path)
            if predicate(data):
                return data
        except (OSError, ValueError):
            pass
        time.sleep(.02)
    raise RuntimeError('file deadline: ' + str(path))


def error(a, b):
    if a is None or b is None:
        return 99999
    return max(abs(a['x'] - b['x']), abs(a['y'] - b['y']),
               abs(a['x'] + a['width'] - b['x'] - b['width']),
               abs(a['y'] + a['height'] - b['y'] - b['height']))


processes, fixtures, rows = [], [], []
summary = {'binarySha256': hashlib.sha256(EXE.read_bytes()).hexdigest(), 'checks': {}, 'failures': []}
seq = 0
parent = None


def memory(pid):
    from ctypes import wintypes as w
    class Counters(ctypes.Structure):
        _fields_ = [('cb', w.DWORD), ('pageFaults', w.DWORD)] + [(n, ctypes.c_size_t) for n in
            ['peakWorkingSet', 'workingSet', 'peakPaged', 'paged', 'peakNonPaged', 'nonPaged', 'pagefile', 'peakPagefile']]
    k = ctypes.windll.kernel32
    k.OpenProcess.restype = w.HANDLE
    k.CloseHandle.argtypes = [w.HANDLE]
    p = ctypes.windll.psapi.GetProcessMemoryInfo
    p.argtypes = [w.HANDLE, ctypes.POINTER(Counters), w.DWORD]
    handle = k.OpenProcess(0x410, False, pid)
    try:
        c = Counters(); c.cb = ctypes.sizeof(c)
        if not handle or not p(handle, ctypes.byref(c), c.cb):
            raise ctypes.WinError()
        return {'workingSetBytes': c.workingSet, 'privateCommitBytes': c.pagefile}
    finally:
        if handle: k.CloseHandle(handle)


def replace_retry(temp, path):
    for attempt in range(50):
        try:
            temp.replace(path)
            return
        except PermissionError:
            if attempt == 49: raise
            time.sleep(.01)  # Short Windows file-sharing race with the local reader.


def send(action, **args):
    global seq
    seq += 1
    command = dict(action=action, sequence=seq, **args)
    path = OUT/'command.json'
    temp = path.with_suffix('.tmp')
    temp.write_text(json.dumps(command), encoding='utf-8')
    replace_retry(temp, path)
    result = wait_file(OUT/f'response-{seq}.json', timeout=35)
    if action != 'finish': result['memory'] = memory(parent.pid)
    result['command'] = command
    rows.append(result)
    if result['error'] is not None:
        raise RuntimeError(result)
    assert result['parentPid'] == parent.pid
    return result


def control(index, command):
    directory = fixtures[index].parent
    temp=directory/'command.tmp'
    temp.write_text(command, encoding='ascii')
    replace_retry(temp, directory/'command.txt')
    return wait_file(fixtures[index], lambda v: v.get('command') == command)


def expected(index, point):
    f = read(fixtures[index])
    p = next(p for p in f['points'] if p['id'] == point)
    return f, dict(p['expected'], x=p['expected']['x']-f['screen']['x'], y=p['expected']['y']-f['screen']['y'])


def sample(index, point, observed):
    f, rect = expected(index, point)
    data = observed['result']
    frames = data['frames']
    frame = next(r for r in frames if r['monitor']['x'] == f['screen']['x'])
    view = next(r['view'] for r in data['views'] if r['label'] == frame['label'])
    calls = [r for r in view['calls'] if r['command'] == 'get_capture_element']
    point_case = next(p for p in f['points'] if p['id'] == point)
    local = {'x': point_case['x']-f['screen']['x'], 'y': point_case['y']-f['screen']['y']}
    semantic = [r for r in calls[-1:] if max(abs(r['args']['point'][axis]-local[axis]) for axis in ['x','y'])<=1 and r['args']['generation']==frame['generation'] and r.get('result') and error(r['result'][0], rect) <= 1]
    return {'fixture': index, 'point': point, 'expected': rect, 'actual': view.get('box'),
            'errorPx': error(view.get('box'), rect), 'semantic': bool(semantic),
            'requestCount': len(calls), 'latencyMs': max((r['end']-r['start'] for r in semantic), default=None),
            'generation': frame['generation'], 'sequence': observed['sequence'], 'parentPid': observed['parentPid'],
            'expectedPoint':local, 'initialPointer':view.get('frame',{}).get('pointer'),
            'lastRequestedPoint':calls[-1]['args']['point'] if calls else None}


def record_check(name, passed):
    summary['checks'][name] = bool(passed)
    if not passed:
        summary['failures'].append(name)


try:
    assert ctypes.windll.user32.GetSystemMetrics(80) == 2, 'this matrix requires two monitors'
    for index, monitor in enumerate([0, 1, 0]):
        directory = OUT / f'fixture-{index}'
        directory.mkdir()
        proc = subprocess.Popen([str(FIXTURE), str(directory), str(monitor)], creationflags=NO_WINDOW)
        processes.append(proc)
        path = directory/'fixture.json'
        wait_file(path)
        fixtures.append(path)
    control(2, 'hide:initial')
    (OUT/'config.json').write_text(json.dumps({'fixtures': [str(p) for p in fixtures]}), encoding='utf-8')
    parent = subprocess.Popen([str(EXE), '--starshot-uia-integration', str(OUT/'config.json')],
                              stdout=subprocess.DEVNULL, stderr=(OUT/'parent.stderr.txt').open('w'), creationflags=NO_WINDOW)
    ready = wait_file(OUT/'ready.json')
    assert ready['pid'] == parent.pid
    summary['parentPid'] = parent.pid
    summary['fixtures'] = [read(p) for p in fixtures]
    # Twenty independent capture sessions, the same still-live application parent.
    startup = []
    for i in range(20):
        target = i % 2
        start = send('start', fixture=target, point='native-button')
        time.sleep(1.2)
        observed = send('observe')
        item = sample(target, 'native-button', observed)
        other_views = [v['view'] for v in observed['result']['views'] if not v['view'].get('frame', {}).get('pointer')]
        item['inactiveQueries'] = sum(len([c for c in v['calls'] if c['command']=='get_capture_element']) for v in other_views)
        item['inactiveViewCount'] = len(other_views)
        item['frameCount'] = len(start['result'])
        item['boundsCorrect'] = all(f['actualPosition']==dict(x=f['monitor']['x'], y=f['monitor']['y']) and
                                   f['actualSize']==dict(width=f['monitor']['width'], height=f['monitor']['height']) for f in start['result'])
        item['ownHosts'] = sum(f['ownHostCount'] for f in start['result'])
        startup.append(item)
        send('clear')
        time.sleep(.1)
    summary['startup'] = startup
    record_check('20_stationary_semantic', len(startup)==20 and all(r['semantic'] and r['errorPx']<=1 for r in startup))
    record_check('20_inactive_monitors_no_query', len(startup)==20 and all(r['inactiveViewCount']==1 and r['inactiveQueries']==0 for r in startup))
    record_check('20_production_two_overlays', len(startup)==20 and all(r['frameCount']==2 and r['boundsCorrect'] and r['ownHosts']==0 for r in startup))
    # Actual OS pointer; 110 ms crosses debounce but is shorter than helper startup.
    send('start', fixture=0, point='native-button')
    time.sleep(1.2)
    cross = []
    summary['crossScreen'] = cross
    for phase, delay in [('ordinary',0), ('delayed-reap',150)]:
        send('reap-delay',ms=delay)
        cross_start=seq
        phase_samples=[]
        for i in range(20):
            final=i%2
            send('sweep',fixture=final)
            time.sleep(1.0)
            observed=send('observe')
            item=sample(final,'edit',observed)
            item['phase']=phase
            cross.append(item);phase_samples.append(item)
        cross_trace=[t for row in rows if cross_start<row['sequence']<=seq for t in row['trace']]
        active_cross, overlaps=set(),0
        for t in cross_trace:
            if t['event']=='request' and active_cross: overlaps+=1
            if t['event']=='spawn': active_cross.add(t['helperPid'])
            if t['event']=='reap': active_cross.discard(t['helperPid'])
        summary[phase+'Contention']={'requestsBeforePriorReap':overlaps,'cancelled':sum(t['event']=='reap' and t['status']=='cancelled' for t in cross_trace)}
        record_check(phase+'_20_roundtrips_current_semantic',len(phase_samples)==20 and all(r['semantic'] and r['errorPx']<=1 for r in phase_samples))
        if delay:
            record_check('delayed_reap_slot_contention',overlaps>=20)
            record_check('delayed_reap_cancellation',summary[phase+'Contention']['cancelled']>=20)
    send('reap-delay',ms=0)
    send('clear')
    # Two original top-level windows at the same coordinates; real snapshot Z order.
    control(2, 'front:occlusion')
    front_start = send('start', fixture=2, point='native-button')
    time.sleep(1.2)
    front_observed = send('observe')
    front_sample = sample(2, 'native-button', front_observed)
    f, _ = expected(2, 'native-button')
    frame = next(r for r in front_start['result'] if r['monitor']['x']==f['screen']['x'])
    record_check('front_fixture_selected_by_z_order', front_sample['semantic'] and front_sample['errorPx']<=1 and len(frame['hosts'])>=2 and frame['hosts'][0]['window']['pid']==f['pid'])
    send('clear')
    control(2, 'hide:occlusion')
    send('start', fixture=0, point='native-button')
    time.sleep(1.2)
    back_sample = sample(0, 'native-button', send('observe'))
    record_check('underlying_fixture_after_hide', back_sample['semantic'] and back_sample['errorPx']<=1)
    # Destroy/recreate HWND while the old capture session remains alive.
    old = read(fixtures[0])
    rebuilt = control(0, 'rebuild:stale')
    send('raise')  # Recreating our topmost fixture can put it above the overlays.
    send('move', fixture=1, point='native-button')
    time.sleep(.1)
    send('move', fixture=0, point='edit')
    time.sleep(1.1)
    stale = send('observe')
    stale_rows = [t for row in rows[-3:] for t in row['trace'] if t['event']=='reply']
    record_check('destroyed_handle_refused', rebuilt['hwnd']!=old['hwnd'] and any(t['status']=='stale-target' for t in stale_rows))
    send('clear')
    send('start', fixture=0, point='edit')
    time.sleep(1.2)
    rebuilt_sample = sample(0, 'edit', send('observe'))
    record_check('rebuilt_handle_next_capture', rebuilt_sample['semantic'] and rebuilt_sample['errorPx']<=1)
    summary['occlusionRebuild'] = dict(front=front_sample, back=back_sample, rebuilt=rebuilt_sample, oldHwnd=old['hwnd'], newHwnd=rebuilt['hwnd'], staleSequence=stale['sequence'])
    send('clear')
    # One persistent parent, deliberately hung original provider, separate healthy original target.
    control(2, 'front:block')
    (fixtures[2].parent/'block').write_text('block original provider', encoding='ascii')
    recovery = []
    summary['recovery']=recovery
    for kind in ['timeout', 'cancelled']:
        for i in range(10):
            options = {'cancelMs': 60} if kind=='cancelled' else {}
            fault = send('query', fixture=2, point='blocker', **options)
            healthy = send('query', fixture=1, point='native-button')
            r = healthy['result']
            recovery.append({'kind':kind,'iteration':i,'parentPid':parent.pid,'faultSequence':fault['sequence'],
                             'healthySequence':healthy['sequence'],'status':fault['result']['reply']['status'],
                             'faultMs':fault['result']['wallMs'],'healthyMs':r['wallMs'], 'expected':r['expected'],
                             'actual':r['reply']['candidates'][0] if r['reply']['candidates'] else None,
                             'healthyStatus':r['reply']['status'],'afterFault':fault['census'],'afterHealthy':healthy['census']})
            recovery[-1]['memory'] = healthy['memory']
    summary['recovery'] = recovery
    for kind in ['timeout', 'cancelled']:
        samples = [r for r in recovery if r['kind']==kind]
        record_check('10_'+kind+'_then_healthy_same_parent', len(samples)==10 and all(r['status']==kind and r['healthyStatus']=='ok' and error(r['actual'],r['expected'])==0 for r in samples))
    record_check('no_helpers_after_recovery', len(recovery)==20 and all(r['afterFault']['helperChildren']==0 and r['afterHealthy']['helperChildren']==0 for r in recovery))
    # Runtime thread pools may initialize once. Compare the warmed samples, not pre-WebView startup.
    record_check('bounded_recovery_threads_handles', len(recovery)==20 and all(r['afterHealthy']['threads']<=recovery[1]['afterHealthy']['threads']+2 and r['afterHealthy']['handles']<=recovery[1]['afterHealthy']['handles']+8 for r in recovery[2:]))
    record_check('bounded_recovery_memory', len(recovery)==20 and max(r['memory']['privateCommitBytes'] for r in recovery[2:])-recovery[1]['memory']['privateCommitBytes'] < 16*1024*1024)
    finish = send('finish')
    parent.wait(timeout=10)
    record_check('parent_exited_cleanly', parent.returncode==0)
    trace = [t for r in rows for t in r['trace']]
    active, peak = set(), 0
    for t in trace:
        if t['event']=='spawn': active.add(t['helperPid']); peak=max(peak,len(active))
        if t['event']=='reap': active.discard(t['helperPid'])
    record_check('single_helper_reaped', peak==1 and not active)
    record_check('one_parent_all_requests', bool(trace) and all(t['parentPid']==parent.pid for t in trace))
    summary['helperPeak']=peak
    summary['generatedPixelCalls']=finish['generatedPixelCalls']
except BaseException as e:
    summary['exception']=repr(e)
    (OUT/'exception.txt').write_text(traceback.format_exc(),encoding='utf-8')
    summary['failures'].append('exception')
finally:
    if parent is not None and parent.poll() is None:
        subprocess.run(['taskkill','/PID',str(parent.pid),'/T','/F'],capture_output=True)
        parent.wait(timeout=10)
    for proc in processes:
        if proc.poll() is None: proc.kill()
        proc.wait(timeout=5)
    (OUT/'native-results.json').write_text(json.dumps(rows,ensure_ascii=False,separators=(',',':')),encoding='utf-8')
    (OUT/'native-summary.json').write_text(json.dumps(summary,ensure_ascii=False,indent=2),encoding='utf-8')
    print(json.dumps({'directory':str(OUT),'checks':summary['checks'],'failures':summary['failures'],'exception':summary.get('exception')},ensure_ascii=False))
assert not summary['failures'],summary['failures']
