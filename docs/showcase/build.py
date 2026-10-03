#!/usr/bin/env python3
"""Build a standalone showcase gallery from real saccade output.

Runs every command in showcases/<case>/commands.json with the given saccade
binary, then writes:
  media/    optimized copies of the compared images and heatmaps (Pillow)
  reports/  the full HTML reports, with local paths replaced by "$OUT"
  assets/   tokens.css and components.css copied from crates/saccade-core
  index.html

Usage: python3 docs/showcase/build.py --saccade target/release/saccade --out .work/saccade-pages/showcase
Default output: $CARGO_TARGET_DIR/showcase (or target/showcase). Sources are never rewritten.
Requires Python 3, Pillow, and saccade built with --features prechecks. Nothing is downloaded.
"""
import argparse
import hashlib
import html
import json
import os
import re
import shlex
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

from PIL import Image

HERE = Path(__file__).resolve().parent
REPO = HERE.parent.parent
SHOW = REPO / 'showcases'
OUTPUT = Path(os.environ.get('CARGO_TARGET_DIR', REPO / 'target')) / 'showcase'
MEDIA = OUTPUT / 'media'
REPORTS = OUTPUT / 'reports'
GITHUB = 'https://github.com/Tavrin/saccade'
TEXT_EXT = {'.html', '.js', '.json', '.md', '.txt', '.css'}

E = html.escape


# --------------------------------------------------------------------- run
def run_cases(binary, out_root):
    env = dict(os.environ, LC_ALL='C', NO_COLOR='1')
    capabilities = subprocess.run([binary, 'inspect', 'capabilities', '--json'],
                                  check=True, capture_output=True, text=True, encoding="utf-8", env=env)
    features = json.loads(capabilities.stdout)['data']['features']
    results = {}
    for manifest in sorted(SHOW.glob('*/commands.json')):
        case = manifest.parent
        out = out_root / case.name
        out.mkdir(parents=True, exist_ok=True)
        if case.name == 'photosensitivity' and 'prechecks' not in features:
            results[case.name] = {'out': out, 'runs': [], 'unvalidated': True}
            continue
        runs = []
        for c in json.loads(manifest.read_text(encoding="utf-8")):
            args = [binary] + [a.replace('@REPORTS@', str(out)) for a in c['args']]
            if c.get('out'):
                args += ['--out', str(out / c['out'])]
            r = subprocess.run(args, cwd=case, env=env, capture_output=True, text=True, encoding="utf-8")
            if r.returncode != c['exit']:
                sys.exit(f'{case.name}/{c["name"]}: exit {r.returncode}, expected {c["exit"]}\n{r.stderr}')
            runs.append({'cmd': c, 'stdout': r.stdout, 'exit': r.returncode})
        results[case.name] = {'out': out, 'runs': runs}
    return results


def sanitizer(case_out):
    pairs = [(str(case_out), '$OUT'), (str(case_out.parent), '$RUNS'),
             (str(SHOW) + '/', 'showcases/'), (str(REPO) + '/', '')]
    def clean(text):
        for a, b in pairs:
            text = text.replace(a, b)
            text = text.replace(a.replace('/', '\\/'), b.replace('/', '\\/'))
        return text
    return clean


def copy_report(src, dst, clean):
    if dst.exists():
        shutil.rmtree(dst)
    for p in src.rglob('*'):
        if p.is_dir():
            continue
        q = dst / p.relative_to(src)
        q.parent.mkdir(parents=True, exist_ok=True)
        if p.suffix in TEXT_EXT:
            q.write_text(clean(p.read_text(encoding="utf-8")), encoding="utf-8", newline="\n")
        else:
            shutil.copyfile(p, q)
    # Redacting relocatable paths changes report bytes, but not measured semantics.
    # Keep exact artifact references correct without rebinding reviewed content.
    for document in dst.rglob('evidence.json'):
        case = json.loads(document.read_text(encoding="utf-8"))
        if case.get('kind') != 'case':
            continue
        reference = case['measurement']['report']
        report = document.parent / reference['path']
        actual = 'sha256:' + hashlib.sha256(report.read_bytes()).hexdigest()
        previous = reference['sha256']
        if actual == previous:
            continue
        if any(case.get(key) for key in ('requests', 'proposals', 'human_decisions', 'next_actions')):
            raise ValueError('Cannot redact a report with bound workflow records: ' + str(document))
        reference['sha256'] = actual
        for fact in case['facts']:
            artifact = fact['artifact']
            if artifact['path'] == reference['path'] and artifact['sha256'] == previous:
                artifact['sha256'] = actual
        # Case and fact identities deliberately exclude relocatable artifact references.
        document.write_text(json.dumps(case, indent=2, ensure_ascii=False) + '\n', encoding='utf-8', newline='\n')
    # Some report renderers leave this optional sidecar absent until a decision
    # is recorded. Static galleries have no decisions; keep their script links valid.
    for page in dst.rglob('index.html'):
        sidecar = page.parent / 'saccade-decisions.v1.js'
        if 'src="saccade-decisions.v1.js"' in page.read_text(encoding="utf-8") and not sidecar.exists():
            sidecar.write_text('window.__saccadeDecisions=null;\n', encoding='utf-8', newline='\n')


# ------------------------------------------------------------------- media
def save_media(src, rel):
    """Copy one image into media/, 8-bit, optimized. Returns the page-relative path."""
    dst = MEDIA / rel
    dst.parent.mkdir(parents=True, exist_ok=True)
    im = Image.open(src)
    if im.mode in ('I', 'I;16', 'I;16B'):
        im = im.point(lambda v: v / 257).convert('L')  # 16-bit depth -> 8-bit for display
    elif im.mode not in ('RGB', 'RGBA', 'L'):
        im = im.convert('RGB')
    im.save(dst, optimize=True)
    return 'media/' + rel


def thumb(case, b, cap, h, size=(480, 300), focus=None):
    """A split thumbnail: baseline on the left; on the right, the capture dimmed under its FLIP heatmap.
    `focus` is a hotspot [x, y, w, h] (fractions) to crop around when the change is too small to see."""
    a = Image.open(b).convert('RGB')
    m = Image.open(h).convert('RGB').resize(a.size)
    c0 = Image.open(cap).convert('RGB')
    if focus:
        cw, ch = max(1, a.width // 4), max(1, round(a.width // 4 * size[1] / size[0]))
        cx, cy = (focus[0] + focus[2] / 2) * a.width, (focus[1] + focus[3] / 2) * a.height
        x0 = int(min(max(cx - cw * 0.72, 0), a.width - cw))  # land the change in the right half
        y0 = int(min(max(cy - ch / 2, 0), a.height - ch))
        box = (x0, y0, x0 + cw, y0 + ch)
        a, m, c0 = a.crop(box), m.crop(box), c0.crop(box)
    s = min(size[0] / a.width, size[1] / a.height)
    scale = int(s) if s >= 2 else s  # integer upscaling keeps pixels square
    w, hh = int(a.width * scale), int(a.height * scale)
    a = a.resize((w, hh), Image.NEAREST)
    m = m.resize((w, hh), Image.NEAREST)
    c = c0.resize((w, hh), Image.NEAREST)
    dim = Image.eval(c, lambda v: int(v * 0.45))
    over = Image.composite(m, dim, m.convert('L').point(lambda v: min(255, v * 3)))
    out = a.copy()
    out.paste(over.crop((w // 2, 0, w, hh)), (w // 2, 0))
    for y in range(hh):
        out.putpixel((w // 2, y), (255, 255, 255))
        if w // 2 + 1 < w:
            out.putpixel((w // 2 - 1, y), (255, 255, 255))
    canvas = Image.new('RGB', size, (20, 23, 29))  # --stage-bg
    canvas.paste(out, ((size[0] - w) // 2, (size[1] - hh) // 2))
    rel = f'{case}/thumb.png'
    dst = MEDIA / rel
    dst.parent.mkdir(parents=True, exist_ok=True)
    canvas.save(dst, optimize=True)
    return 'media/' + rel


# ------------------------------------------------------------------ entries
def hs_list(e, limit=5):
    return [[round(v, 5) for v in h['rect_frac']] + [round(h['share_of_total_error'], 4)]
            for h in (e.get('hotspots') or [])[:limit]]


def entry_from_report(case, rep_dir, e, key, left=None, right=None):
    p = e['paths']
    stem = re.sub(r'[^A-Za-z0-9_.-]', '_', key)
    b = save_media(rep_dir / p['baseline'], f'{case}/{stem}-b.png')
    c = save_media(rep_dir / p['capture'], f'{case}/{stem}-c.png')
    h = save_media(rep_dir / p['heatmap'], f'{case}/{stem}-h.png') if p.get('heatmap') else None
    w, hh = Image.open(rep_dir / p['baseline']).size  # buffer entries carry no FLIP metrics
    buf = e.get('buffer')
    metric = e['metric_used'] + (f" ({buf['unit']})" if buf else '')
    d = {
        'name': e['name'], 'b': b, 'c': c, 'h': h, 'ar': round(w / hh, 5), 'w': w, 'hgt': hh,
        'status': e['status'], 'metric': metric, 'value': e['value'], 'thr': e['threshold'],
        'hs': hs_list(e), 'diag': (e.get('diagnostics') or {}).get('description'),
        'cls': (e.get('diagnostics') or {}).get('class'), 'bit': e.get('bit_identical'),
        'meta': e.get('meta_ignored_diff') or [], 'buffer': buf,
    }
    if buf:
        d['hlabel'] = f"error map ({buf['unit']})"
    if left:
        d['left'] = left
    if right:
        d['right'] = right
    d['foot'] = foot_html(d)
    return d


def foot_html(d):
    tone = {'fail': 's-fail', 'pass': 's-pass'}.get(d['status'], 's-info')
    parts = []
    if d.get('diag'):
        cls = f'<span class="badge">{E(d["cls"])}</span>' if d.get('cls') else ''
        parts.append(f'<div class="diag {tone}"><div class="dh">{cls}</div><p class="dcap">{E(d["diag"])}</p></div>')
    elif d.get('buffer'):
        b = d['buffer']
        st = b.get('stats') or {}
        parts.append(f'<div class="diag {tone}"><p class="dcap">{E(b["kind"])} buffer, {E(b["encoding"])}: '
                     f'mean {st.get("mean", 0):.5f}, max {st.get("max", 0):.5f} {E(b["unit"])} (limit {b["threshold"]}). '
                     'Compared numerically, not with FLIP.</p></div>')
    elif d.get('bit'):
        parts.append(f'<div class="diag {tone}"><p class="dcap">Bit-identical: every decoded sample matches.</p></div>')
    elif d['status'] == 'pass':
        parts.append(f'<div class="diag {tone}"><p class="dcap">Under the threshold.</p></div>')
    row = []
    if d.get('h') and d.get('buffer'):
        b = d['buffer']
        row.append(f'<span class="legend flip">0 <i></i> {b.get("heatmap_max", 1):g} {E(b["unit"])}</span>')
    elif d.get('h'):
        row.append('<span class="legend flip">0 <i></i> 1 FLIP</span>')
    for m in d.get('meta') or []:
        row.append(f'<span class="badge">{E(m["key"])} {E(m["baseline"])} → {E(m["capture"])}</span>')
    if d.get('bit'):
        row.append('<span class="badge pass">bit-identical</span>')
    if row:
        parts.append('<div class="row">' + ''.join(row) + '</div>')
    return ''.join(parts)


def order(entries):
    rank = {'fail': 0, 'error': 1, 'missing': 2, 'new': 3, 'pass': 4}
    return sorted(entries, key=lambda e: (rank.get(e['status'], 5), e['name']))


# ------------------------------------------------------------------ html bits
def term(title, body_html, copy=None, cls=''):
    data = f' data-copy="{E(copy)}"' if copy else ''
    btn = '<button class="copy" type="button">Copy</button>' if copy else ''
    return (f'<div class="term"><div class="term-h">{E(title)}{btn}</div>'
            f'<pre class="{cls}"{data}>{body_html}</pre></div>')


def color_output(text):
    out = []
    for line in E(text).splitlines():
        line = re.sub(r'^(FAIL)\b', r'<span class="k-fail">\1</span>', line)
        line = re.sub(r'^(pass)\b', r'<span class="k-pass">\1</span>', line)
        line = re.sub(r'^(\s+↳)', r'<span class="k-note">\1</span>', line)
        out.append(line)
    return '\n'.join(out)


def command_text(case, runs):
    lines = [f'cd showcases/{case}', 'OUT=$(mktemp -d)']
    for r in runs:
        c = r['cmd']
        args = []
        for a in c['args']:
            if '@REPORTS@' in a:
                args.append('"' + a.replace('@REPORTS@', '$OUT') + '"')
            else:
                args.append(shlex.quote(a))
        if c.get('out'):
            args += ['--out', f'"$OUT/{c["out"]}"']
        lines.append('saccade ' + ' '.join(args))
    return '\n'.join(lines)


def command_html(text):
    out = []
    for line in text.splitlines():
        if line.startswith('saccade '):
            out.append('<span class="p">$ </span><span class="c">saccade</span>' + E(line[8:]))
        else:
            out.append('<span class="p">$ </span>' + E(line))
    return '\n'.join(out)


def widget(wid, data, split=50, intro=False, maxh=None, picker=None, tools=True, px=False, zoom=False):
    e = data['entries'][data.get('default', 0)]
    script = f'<script type="application/json" id="{wid}-data">{json.dumps(data, separators=(",", ":")).replace("</", "<\\/")}</script>'
    pick = ''
    if picker:
        pick = f'<div class="picker" data-for="{wid}" role="group" aria-label="Image">' + ''.join(
            f'<button type="button" class="s-{x["status"]}" data-pick="{i}" aria-pressed="{str(i == data.get("default", 0)).lower()}">{E(picker(x))}</button>'
            for i, x in enumerate(data['entries'])) + '</div>'
    tl = ''
    if tools:
        tl = ('<div class="tools"><div class="seg small" data-role="layer" role="group" aria-label="Right-hand layer">'
              f'<button type="button" data-layer="cap" aria-pressed="true">{E(data["right"].capitalize())}</button>'
              '<button type="button" data-layer="heat" aria-pressed="false">Heatmap</button></div>'
              '<button type="button" class="btn small" data-role="boxes" aria-pressed="true">Hotspots</button>'
              '<button type="button" class="btn small" data-role="zoom" aria-pressed="false" title="Zoom to the largest hotspot">Zoom</button></div>')
    st = 'FAIL' if e['status'] == 'fail' else e['status']
    style = f'--ar:{e["ar"]};' + (f'--maxh:{maxh}px;' if maxh else '')
    boxes = ''.join(f'<div class="hb" style="left:{h[0]*100}%;top:{h[1]*100}%;width:{h[2]*100}%;height:{h[3]*100}%"></div>' for h in e['hs'])
    intro_attr = (' data-intro' if intro else '') + (' data-zoom' if zoom else '')
    return f'''{script}<figure class="cmp" id="{wid}" data-src="{wid}-data" data-split="{split}"{intro_attr}>
<div class="cmp-bar"><span class="chip s-{e["status"]}">{st}</span><span class="name">{E(e["name"])}</span><span class="val">{E(e["metric"])} {e["value"]:.5f} (limit {e["thr"]})</span>{tl}</div>
{pick}<div class="stagebox"><div class="stg" style="{style}">
<div class="stg-l" aria-hidden="true"><span class="tag l">{E(e.get("left", data["left"]))}</span><span class="tag r">{E(e.get("right", data["right"]))}</span></div>
<div class="stage{' px' if px else ''}">
<div class="ly base"><img src="{e["b"]}" alt="{E(data["left"])} {E(e["name"])}" width="{e["w"]}" height="{e["hgt"]}" draggable="false"></div>
<div class="ly top"><img src="{e["c"]}" alt="{E(data["right"])} {E(e["name"])}" width="{e["w"]}" height="{e["hgt"]}" draggable="false"></div>
<div class="boxes">{boxes}</div>
<div class="div" role="slider" tabindex="0" aria-label="Swipe divider" aria-valuemin="0" aria-valuemax="100" aria-valuenow="{split}"><span class="grip"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M9 6l-6 6 6 6M15 6l6 6-6 6"/></svg></span></div>
</div></div></div>
<figcaption class="cmp-foot">{e["foot"]}</figcaption></figure>'''


def rank_table(wid, overall, metric):
    mx = max(o['mean_metric'] for o in overall)
    rows = ''.join(
        f'<tr data-pick="{i}" tabindex="0" aria-selected="{str(i == 0).lower()}"><td>{o["rank"]}</td><td>{E(o["label"])}</td>'
        f'<td class="num">{o["mean_metric"]:.5f}</td><td class="bar"><span class="b" style="width:{o["mean_metric"] / mx * 100:.1f}%"></span></td></tr>'
        for i, o in enumerate(overall))
    return (f'<table class="rank" data-for="{wid}"><thead><tr><th>#</th><th>Candidate</th><th>{E(metric)} FLIP</th><th></th></tr></thead>'
            f'<tbody>{rows}</tbody></table>')


def seq_chart(wid, seq, threshold, clickable):
    curve = seq['mean_flip_curve']
    n = len(curve)
    W, H, pad = 600, 120, 34
    mx = max(max(curve), threshold) * 1.15
    bw = (W - pad) / n
    bars = []
    for i, v in enumerate(curve):
        h = (H - 20) * v / mx
        cls = 'bar' + (' over' if v > threshold else '') + ('' if clickable else ' static')
        attrs = f' data-pick="{i}" tabindex="0" role="button" aria-label="Frame {i}: mean FLIP {v:.4f}"' if clickable else ''
        bars.append(f'<rect class="{cls}"{attrs} x="{pad + i * bw + 2:.1f}" y="{H - 20 - h:.1f}" width="{bw - 4:.1f}" height="{max(h, 1):.1f}" rx="2"><title>frame {i}: {v:.4f}</title></rect>')
        if n <= 16:
            bars.append(f'<text x="{pad + i * bw + bw / 2:.1f}" y="{H - 5}" text-anchor="middle">{i}</text>')
    ty = H - 20 - (H - 20) * threshold / mx
    thr = f'<line class="thr" x1="{pad}" x2="{W}" y1="{ty:.1f}" y2="{ty:.1f}"/><text x="0" y="{ty + 4:.1f}">{threshold:g}</text>'
    axis = f'<line class="axis" x1="{pad}" x2="{W}" y1="{H - 20}" y2="{H - 20}"/>'
    data_for = f' data-for="{wid}"' if clickable else ''
    return (f'<div class="seq"{data_for}><svg viewBox="0 0 {W} {H}" role="img" aria-label="Mean FLIP per frame">{axis}{thr}{"".join(bars)}</svg>'
            f'<dl class="kv"><dt>temporal instability</dt><dd>{seq["temporal_instability"]:.6f} (capture {seq["capture_temporal_mean"]:.6f} − baseline {seq["baseline_temporal_mean"]:.6f})</dd>'
            f'<dt>frames over threshold</dt><dd>{seq["frames_over_threshold"]} of {n}</dd>'
            f'<dt>worst frame</dt><dd>{seq["worst_frame"].get("index", "-")}</dd></dl></div>')


# ------------------------------------------------------------------ cases
CASES = [
    {
        'id': 'webapp-ui', 'title': 'UI screenshots',
        'problem': 'A dashboard screenshot where a button label changed and the clock changes on every run.',
        'about': 'Three regressions, one pixel-identical page and one page where only the masked timestamp changed. '
                 'The header is a region with its own p95 gate; the timestamp mask has a margin because FLIP spreads error past glyph edges.',
        'pick': ['token.png', 'button-shift.png', 'label.png', 'timestamp-only.png', 'identical.png'],
    },
    {
        'id': 'cover-art', 'title': 'Cover art and image assets',
        'problem': 'A warm tint, an 8% crop and JPEG quality 40 all fail. The diagnostics say which is which.',
        'about': 'The tint is classed as a global tone shift (measured: tone explains 94% of the difference). The crop and the recompression are structural changes.',
        'pick': ['warmer.png', 'crop.png', 'jpeg-q40.png'],
    },
    {
        'id': 'texture-compression', 'title': 'Texture compression',
        'problem': 'Five lossy encodings of one texture, ranked by how visible their error is.',
        'about': 'The block candidates are simulations of two block-compression qualities, not real BC or ASTC encoders. '
                 'All five exceed mean FLIP 0.001. Ranking completes with exit 0; inspect its findings.',
    },
    {
        'id': 'upscaler', 'title': 'Upscalers and temporal shimmer',
        'problem': 'Rank reconstruction filters against the native image, then measure flicker over a camera pan.',
        'about': 'On this scene nearest-neighbour has the lowest mean FLIP. A ranking reports which output differs least visibly from the reference, not which one people prefer. '
                 'Temporal instability is informational and not motion-compensated.',
    },
    {
        'id': 'lod-transition', 'title': 'LOD pops in an animation',
        'problem': 'A rotating gear loses its fine spokes for one frame. Which frame?',
        'about': 'Twelve frames compared by sorted index. Only frame 6 crosses the threshold; the pop and the recovery also add temporal instability.',
    },
    {
        'id': 'ml-image-model', 'title': 'Image-model checkpoints',
        'problem': 'Checkpoint B must keep the colour and structure of checkpoint A for the same seeds.',
        'about': 'Six fixed procedural seeds stand in for shared prompts. B adds a colour cast to two seeds and moves one shape. '
                 'The case also prepares evidence strips and a Markdown export; no model is called.',
        'pick': ['seed_101.png', 'seed_105.png', 'seed_104.png', 'seed_100.png'],
    },
    {
        'id': 'render-gbuffer', 'title': 'G-buffers: depth, normals, motion',
        'problem': 'Depth, normals and motion vectors are data, not pictures. Compare them in their own units.',
        'about': 'The lit image goes through FLIP. Buffers declared in saccade.toml bypass FLIP and are measured in depth units, degrees and pixels. '
                 'The capture quantizes depth to 32 levels; normals and motion are unchanged.',
        'pick': ['lit.png', 'depth.png', 'normal.png', 'motion.png'],
    },
    {
        'id': 'perf-identity', 'title': 'Optimization identity proof',
        'problem': 'A faster build must produce the same image. Two views are bit-identical; one pixel moved in the third.',
        'about': '`identity` proves exact native decoded-sample equality for selected captures. Timings come from metadata sidecars and are paired, not qualified. '
                 'The numbers are illustrative, not benchmark measurements.',
        'pick': ['view_2.png', 'view_0.png', 'view_1.png'], 'zoom': True, 'split': 30,
    },
]


def build_case(spec, res, n):
    case = spec['id']
    out = res['out']
    clean = sanitizer(out)
    runs = res['runs']
    links = []
    for r in runs:
        c = r['cmd']
        if c.get('out') and (out / c['out'] / 'index.html').exists():
            copy_report(out / c['out'], REPORTS / case / c['out'], clean)
            links.append((f'reports/{case}/{c["out"]}/index.html', f'{c["name"]} report'))
    first = runs[0]['cmd']['name']
    rep_dir = out / runs[0]['cmd']['out']
    extra = ''
    picker_fn = lambda x: x['name']
    px = False
    wid = f'w-{case}'
    if first in ('compare', 'identity'):
        rep = json.loads((rep_dir / 'saccade-report.v1.json').read_text(encoding="utf-8"))
        labels = (rep.get('config') or {}).get('labels') or {}
        left, right = labels.get('baseline', 'baseline'), labels.get('capture', 'capture')
        by = {e['name']: e for e in rep['entries']}
        names = spec.get('pick') or [e['name'] for e in order(rep['entries'])]
        ents = [entry_from_report(case, rep_dir, by[nm], nm) for nm in names]
        data = {'left': left, 'right': right, 'entries': ents}
        px = ents[0]['w'] <= 256
    elif first == 'rank':
        rk = json.loads((rep_dir / 'saccade-rank.v1.json').read_text(encoding="utf-8"))
        ents = []
        for o in rk['overall']:
            sub = rep_dir / o['label']
            rep = json.loads((sub / 'saccade-report.v1.json').read_text(encoding="utf-8"))
            e = rep['entries'][0]
            ents.append(entry_from_report(case, sub, e, f'{o["label"]}', left='reference', right=o['label']))
        data = {'left': 'reference', 'right': 'candidate', 'entries': ents}
        picker_fn = None
        extra = '<h3>Ranking</h3>' + rank_table(wid, rk['overall'], rk['metric'])
        px = ents[0]['w'] <= 256
    elif first == 'sequence':
        seq = json.loads((rep_dir / 'saccade-sequence.v1.json').read_text(encoding="utf-8"))
        thr = seq['frames'][0]['entry']['threshold']
        ents = []
        for f in seq['frames']:
            e = dict(f['entry'])
            e['name'] = f'frame {f["index"]}'
            ents.append(entry_from_report(case, rep_dir, e, f'frame_{f["index"]:02d}'))
        worst = seq['worst_frame'].get('index', 0)
        data = {'left': 'baseline', 'right': 'capture', 'entries': ents, 'default': worst}
        picker_fn = None
        extra = '<h3>Mean FLIP per frame</h3>' + seq_chart(wid, seq, thr, True)
        px = True
    if case == 'upscaler':
        sq_dir = out / runs[1]['cmd']['out']
        seq = json.loads((sq_dir / 'saccade-sequence.v1.json').read_text(encoding="utf-8"))
        extra += '<h3>Camera pan: mean FLIP per frame</h3>' + seq_chart(wid + '-seq', seq, seq['frames'][0]['entry']['threshold'], False)
    tn = data['entries'][data.get('default', 0)]
    th = thumb(case, OUTPUT / tn['b'], OUTPUT / tn['c'], OUTPUT / (tn['h'] or tn['c']),
               focus=(tn['hs'][0][:4] if spec.get('zoom') and tn['hs'] else None))
    wdg = widget(wid, data, split=spec.get('split', 50), picker=picker_fn, px=px, zoom=spec.get('zoom', False),
                 maxh=(520 if data['entries'][0]['ar'] < 1 else None))
    cmd = command_text(case, runs)
    outputs = ''.join(clean(f'=== {r["cmd"]["name"]} (exit {r["exit"]}) ===\n' + r['stdout']) for r in runs)
    if case == 'ml-image-model':  # the decision request is long JSON; keep its head
        cut = outputs.find('=== decision-request')
        head = outputs[cut:].splitlines()[:16]
        outputs = outputs[:cut] + '\n'.join(head) + '\n  … (3 items; full JSON in the README of this case)\n'
        strip = save_media(out / 'explain/hotspots/seed_105.png.d/h1.png', 'ml-image-model/explain-seed_105-h1.png')
        extra += (f'<h3>Explain strip (what a vision model is given)</h3><figure class="shot">'
                  f'<img class="px" src="{strip}" alt="Explain strip for seed_105: baseline, capture and heatmap crops of the moved shape">'
                  '<figcaption><code>hotspots/seed_105.png.d/h1.png</code>: [baseline | capture | heatmap], one per hotspot.</figcaption></figure>')
    if case == 'render-gbuffer':
        rep = json.loads((rep_dir / 'saccade-report.v1.json').read_text(encoding="utf-8"))
        rows = ''.join(
            f'<tr><td>{E(e["name"])}</td><td>{E((e.get("buffer") or {}).get("kind", "colour"))}</td>'
            f'<td>{E((e.get("buffer") or {}).get("unit", "FLIP"))}</td><td class="num">{e["value"]:.5f}</td><td class="num">{e["threshold"]:g}</td>'
            f'<td><span class="chip s-{e["status"]}">{"FAIL" if e["status"] == "fail" else e["status"]}</span></td></tr>'
            for e in order(rep['entries']))
        extra += ('<h3>Per-buffer measurement</h3><table class="rank"><thead><tr><th>Image</th><th>Kind</th><th>Unit</th><th>Mean</th><th>Limit</th><th></th></tr></thead>'
                  f'<tbody>{rows}</tbody></table>')
    link_html = ''.join(f'<a class="btn" href="{h}">Open the {E(t)}</a>' for h, t in links)
    link_html += f'<a class="btn ghost" href="{GITHUB}/tree/main/showcases/{case}">Dataset</a>'
    verdict = sum(1 for r in runs if r['exit'] == 1)
    section = f'''<section class="case" id="{case}" aria-labelledby="{case}-h">
<div class="wrap">
<header class="case-h"><span class="num">{n:02d}</span><h2 id="{case}-h"><a href="#{case}">{E(spec["title"])}</a></h2><p class="problem">{E(spec["problem"])}</p></header>
<div class="case-b">
<div class="case-main">{wdg}{('<div class="extra">' + extra + '</div>') if extra else ''}</div>
<div class="case-side">
<div><h3>What this shows</h3><p>{inline_code(spec["about"])}</p></div>
{term('Command', command_html(cmd), copy=cmd)}
{term('Output', color_output(outputs), cls='out')}
<div class="links">{link_html}</div>
</div></div></div></section>'''
    totals = summary_chip(runs)
    card = (f'<a class="uc" href="#{case}"><div class="th"><img src="{th}" alt=""></div>'
            f'<div class="bd"><span class="n">{n:02d} · {E(case)}</span><h3>{E(spec["title"])}</h3><p>{E(spec["problem"])}</p>{totals}</div></a>')
    return section, card, {'thumb': th, 'title': spec['title'], 'problem': spec['problem']}


def build_precheck(res, n):
    case = 'photosensitivity'
    title = 'Photosensitivity precheck'
    description = 'Procedural 4 Hz fixture; static previews only. No certification claim.'
    image = save_media(SHOW / case / 'frames/frame_000.png', case + '/thumb.png')
    if res.get('unvalidated'):
        body = 'Unvalidated in this build. Showcase validation requires cargo build --release -p saccade --features prechecks.'
        link = ''
    else:
        body = res['runs'][0]['stdout']
        destination = REPORTS / case / 'safety'
        copy_report(res['out'] / 'safety', destination, sanitizer(res['out']))
        link = '<a class="btn" href="reports/photosensitivity/safety/index.html">Open static report</a>'
    commands = json.loads((SHOW / case / 'commands.json').read_text(encoding="utf-8"))
    command = command_text(case, [{'cmd': c} for c in commands])
    section = (f'<section class="case" id="{case}" aria-labelledby="{case}-h"><div class="wrap">'
               f'<h2 id="{case}-h">{n:02d} · {title}</h2><p>{description}</p>'
               f'{term("Command (requires prechecks)", command_html(command), copy=command)}'
               f'{term("Validation", color_output(body), cls="out")}{link}</div></section>')
    card = (f'<a class="uc" href="#{case}"><div class="th"><img src="{image}" alt="Static black frame"></div>'
            f'<div class="bd"><span class="n">{n:02d} · {case}</span><h3>{title}</h3><p>{description}</p></div></a>')
    return section, card


def summary_chip(runs):
    s = runs[0]['stdout']
    m = re.search(r'(\d+) fail, \d+ error, \d+ missing, \d+ new, (\d+) pass', s)
    if m:
        return f'<span class="chip s-fail">{m.group(1)} fail · {m.group(2)} pass</span>'
    m = re.search(r'frames over threshold: (\d+)', s)
    if m:
        return f'<span class="chip s-fail">{m.group(1)} frame over threshold</span>' if m.group(1) == '1' else f'<span class="chip s-fail">{m.group(1)} frames over threshold</span>'
    n = len(re.findall(r'^\d+  ', s, re.M))
    return f'<span class="chip s-info">{n} candidates ranked</span>'


def inline_code(text):
    return re.sub(r'`([^`]+)`', r'<code>\1</code>', E(text))


# ------------------------------------------------------------------ page
def main():
    global OUTPUT, MEDIA, REPORTS
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('--saccade', default='saccade', help='saccade binary (default: on PATH)')
    ap.add_argument('--out', type=Path, default=OUTPUT, help='standalone gallery output directory')
    ap.add_argument('--keep', help='also keep raw run output in this directory')
    args = ap.parse_args()
    OUTPUT = args.out.resolve()
    if (OUTPUT == REPO or OUTPUT in REPO.parents
            or any(source == OUTPUT or source in OUTPUT.parents for source in (HERE, SHOW, REPO / 'crates'))):
        ap.error('--out must be a build directory separate from the source inputs')
    MEDIA, REPORTS = OUTPUT / 'media', OUTPUT / 'reports'
    binary = shutil.which(args.saccade) or str(Path(args.saccade).resolve())
    for d in (MEDIA, REPORTS):
        if d.exists():
            shutil.rmtree(d)
    (OUTPUT / 'assets').mkdir(parents=True, exist_ok=True)
    for f in ('showcase.css', 'showcase.js'):
        shutil.copyfile(HERE / 'assets' / f, OUTPUT / 'assets' / f)
    for f in ('tokens.css', 'components.css'):
        shutil.copyfile(REPO / 'crates/saccade-core/assets' / f, OUTPUT / 'assets' / f)
    tmp = Path(args.keep) if args.keep else Path(tempfile.mkdtemp(prefix='saccade-showcase-'))
    results = run_cases(binary, tmp)

    sections, cards = [], []
    for n, spec in enumerate(CASES, 1):
        s, c, _ = build_case(spec, results[spec['id']], n)
        sections.append(s)
        cards.append(c)
    extra = set(results) - {spec['id'] for spec in CASES}
    if extra != {'photosensitivity'}:
        ap.error('Gallery cases do not match showcase manifests: ' + ', '.join(sorted(extra)))
    section, card = build_precheck(results['photosensitivity'], len(CASES) + 1)
    sections.append(section)
    cards.append(card)

    # hero: the label regression from webapp-ui, at higher scale
    out = results['webapp-ui']['out']
    rep_dir = out / 'compare'
    rep = json.loads((rep_dir / 'saccade-report.v1.json').read_text(encoding="utf-8"))
    lab = entry_from_report('hero', rep_dir, next(e for e in rep['entries'] if e['name'] == 'label.png'), 'label.png')
    hero = widget('w-hero', {'left': 'baseline', 'right': 'capture', 'entries': [lab]}, split=50, intro=True)
    hero_lines = [l for l in sanitizer(out)(results['webapp-ui']['runs'][0]['stdout']).splitlines()]
    i = next(k for k, l in enumerate(hero_lines) if l.startswith('FAIL    label.png'))
    hero_out = '\n'.join([hero_lines[0], hero_lines[i], hero_lines[i + 1], hero_lines[i + 2], '…', hero_lines[-1]])

    # agent evidence: a real CLI snapshot and the lean JSON result
    snap = MEDIA / 'agents/snapshot-label.png'
    snap.parent.mkdir(parents=True, exist_ok=True)
    subprocess.run([binary, 'inspect', 'export', str(rep_dir / 'saccade-report.v1.json'), '--format', 'png', '--entry', 'label.png',
                    '--state', 'layout=swipe&split=0.5&heat=0.6&hotspot=1&zoom=3', '--out', str(snap), '--width', '1200'],
                   check=True, capture_output=True)
    Image.open(snap).save(snap, optimize=True)
    lean = subprocess.run([binary, 'compare', 'baseline', 'capture', '--config', 'saccade.toml', '--out', str(tmp / 'lean'), '--json'],
                          cwd=SHOW / 'webapp-ui', capture_output=True, text=True, encoding="utf-8", env=dict(os.environ, LC_ALL='C', NO_COLOR='1'))
    if lean.returncode != 1:
        sys.exit(f'UI JSON comparison: expected exit 1, got {lean.returncode}\n{lean.stderr}')
    lj = json.loads(lean.stdout)
    lean_view = {k: lj[k] for k in ('schema', 'execution', 'measurement', 'validity', 'review', 'counts', 'page')}
    lean_view['next_actions'] = lj['next_actions'][:1]
    lean_view = json.loads(sanitizer(tmp / 'lean')(json.dumps(lean_view)))
    lean_txt = json.dumps(lean_view, indent=2, ensure_ascii=False)
    # keep short objects on one line so the excerpt reads like the real output, only shorter
    lean_txt = re.sub(r'\{\n\s+([^{}\[\]]{0,160}?)\n\s*\}', lambda m: '{' + re.sub(r'\n\s+', ' ', m.group(1)) + '}', lean_txt)
    lean_txt = re.sub(r'\[\n\s+([^{}\[\]]{0,200}?)\n\s*\]', lambda m: '[' + re.sub(r'\n\s+', ' ', m.group(1)) + ']', lean_txt)

    page = (HERE / 'template.html').read_text(encoding="utf-8")
    page = (page.replace('{{HERO}}', hero)
                .replace('{{HERO_OUT}}', color_output(hero_out))
                .replace('{{CARDS}}', '\n'.join(cards))
                .replace('{{SECTIONS}}', '\n'.join(sections))
                .replace('{{CASE_COUNT}}', str(len(results)))
                .replace('{{LEAN_JSON}}', E(lean_txt)))
    (OUTPUT / 'index.html').write_text(page, encoding='utf-8', newline='\n')
    (OUTPUT / 'cases.json').write_text(json.dumps({
        'count': len(results),
        'cases': [{'id': name, 'validated': not res.get('unvalidated', False)}
                  for name, res in sorted(results.items())]}, indent=2) + '\n', encoding='utf-8', newline='\n')
    leftovers = [str(p) for p in OUTPUT.rglob('*') if p.suffix in TEXT_EXT and p.is_file()
                 and (str(REPO) in p.read_text(encoding='utf-8', errors='ignore') or str(tmp) in p.read_text(encoding='utf-8', errors='ignore'))]
    if leftovers:
        sys.exit('local paths left in: ' + ', '.join(leftovers))
    size = sum(p.stat().st_size for p in MEDIA.rglob('*') if p.is_file())
    rsize = sum(p.stat().st_size for p in REPORTS.rglob('*') if p.is_file())
    print(f'{OUTPUT / "index.html"} written; media {size / 1e6:.2f} MB, reports {rsize / 1e6:.2f} MB')
    if not args.keep:
        shutil.rmtree(tmp)


if __name__ == '__main__':
    main()
