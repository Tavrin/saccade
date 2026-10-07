#!/usr/bin/env python3
"""Generic display-referred PNG quality measurements and configurable offline gates.

Grid metrics follow the MIT inspect-threejs-canvas definitions:
https://github.com/majidmanzarpour/threejs-game-skills
Requires numpy and Pillow. No network, credentials, or baseline approval.
"""
import argparse
import json
from pathlib import Path
import numpy as np
from PIL import Image
from video_scores import read

GATES = {
    "luminance_contrast": (">=", 60.0),
    "colour_entropy_bits": (">=", 3.0),
    "dominant_colour_share": ("<=", 0.6),
    "edge_density": (">=", 0.04),
    "clipped_share_outside_mask": ("<=", 0.02),
    "palette_coverage": (">=", 0.70),
}
GRID_COLS = 160
GRID_ROWS = 90
EDGE_STEP = 12.0
CLIP_LEVEL = 254
BLACK_LUMA = 10.0

REGRESSION_GATES = {
    "edge_density_ratio": (">=", 0.5),
    "luminance_contrast_ratio": (">=", 0.7),
    "median_luma_ratio_min": (">=", 0.5),
    "median_luma_ratio_max": ("<=", 2.0),
    "colour_entropy_delta_abs": ("<=", 1.5),
    "dominant_share_delta": ("<=", 0.10),
    "clipped_share_delta": ("<=", 0.02),
    "mean_colour_delta_e": ("<=", 10.0),
}

def luma(rgb: np.ndarray) -> np.ndarray:
    """Rec.709 luma of 8-bit sRGB values (0-255), as in the reference script."""
    rgb = rgb.astype(np.float64)
    return 0.2126 * rgb[..., 0] + 0.7152 * rgb[..., 1] + 0.0722 * rgb[..., 2]


def grid_sample(rgb: np.ndarray) -> np.ndarray:
    h, w = rgb.shape[:2]
    step_x = max(1, w // GRID_COLS)
    step_y = max(1, h // GRID_ROWS)
    cols = w // step_x
    rows = h // step_y
    return rgb[0 : rows * step_y : step_y, 0 : cols * step_x : step_x]


def srgb_to_lab(rgb: np.ndarray) -> np.ndarray:
    """8-bit sRGB -> CIE L*a*b* (D65)."""
    c = rgb.astype(np.float64) / 255.0
    lin = np.where(c <= 0.04045, c / 12.92, ((c + 0.055) / 1.055) ** 2.4)
    m = np.array(
        [
            [0.4124564, 0.3575761, 0.1804375],
            [0.2126729, 0.7151522, 0.0721750],
            [0.0193339, 0.1191920, 0.9503041],
        ]
    )
    xyz = lin @ m.T
    xyz = xyz / np.array([0.95047, 1.0, 1.08883])
    eps = 216.0 / 24389.0
    kappa = 24389.0 / 27.0
    f = np.where(xyz > eps, np.cbrt(xyz), (kappa * xyz + 16.0) / 116.0)
    l = 116.0 * f[..., 1] - 16.0
    a = 500.0 * (f[..., 0] - f[..., 1])
    b = 200.0 * (f[..., 1] - f[..., 2])
    return np.stack([l, a, b], axis=-1)


def hex_to_rgb(value: str) -> list[int]:
    s = value.strip().lstrip("#")
    if len(s) != 6:
        raise ValueError(f"palette hex must be #rrggbb, got {value!r}")
    return [int(s[i : i + 2], 16) for i in (0, 2, 4)]


def compute(rgb: np.ndarray, mask: np.ndarray | None = None, palette: list[dict] | None = None,
            delta_e: float = 20.0) -> dict:
    """All metrics for one HxWx3 uint8 image. `mask` is HxW bool (True = emissive)."""
    sample = grid_sample(rgb)
    y = luma(sample)
    flat = np.sort(y.ravel())
    n = flat.size
    p5 = float(flat[int(n * 0.05)])
    p95 = float(flat[min(n - 1, int(n * 0.95))])

    q = (sample >> 4).astype(np.int32)
    keys = (q[..., 0] << 8) | (q[..., 1] << 4) | q[..., 2]
    _, counts = np.unique(keys.ravel(), return_counts=True)
    p = counts / counts.sum()
    entropy = float(-(p * np.log2(p)).sum())
    dominant = float(counts.max() / counts.sum())

    dx = np.abs(y[:-1, :-1] - y[:-1, 1:])
    dy = np.abs(y[:-1, :-1] - y[1:, :-1])
    edges = float((np.maximum(dx, dy) > EDGE_STEP).mean()) if dx.size else 0.0

    clipped = rgb.max(axis=-1) >= CLIP_LEVEL
    if mask is not None:
        outside = ~mask
        clipped_share = float((clipped & outside).sum() / max(1, outside.sum()))
    else:
        clipped_share = float(clipped.mean())

    result = {
        "width": int(rgb.shape[1]),
        "height": int(rgb.shape[0]),
        "grid_samples": int(n),
        "luminance_p5": round(p5, 1),
        "luminance_p50": round(float(flat[n // 2]), 1),
        "luminance_p95": round(p95, 1),
        "luminance_contrast": round(p95 - p5, 1),
        "colour_entropy_bits": round(entropy, 2),
        "colour_buckets": int(counts.size),
        "dominant_colour_share": round(dominant, 3),
        "edge_density": round(edges, 3),
        "clipped_share_outside_mask": round(clipped_share, 4),
        "emissive_mask": mask is not None,
    }

    if palette:
        full_luma = luma(rgb)
        lit = full_luma > BLACK_LUMA
        pixels = rgb[lit]
        if pixels.size == 0:
            result["palette_coverage"] = 0.0
        else:
            lab = srgb_to_lab(pixels)
            pal_lab = srgb_to_lab(np.array([e["rgb"] for e in palette], dtype=np.uint8))
            best = np.full(lab.shape[0], np.inf)
            nearest = np.zeros(lab.shape[0], dtype=np.int64)
            for i, ref in enumerate(pal_lab):
                d = np.sqrt(((lab - ref) ** 2).sum(axis=-1))
                closer = d < best
                best = np.where(closer, d, best)
                nearest = np.where(closer, i, nearest)
            within = best <= delta_e
            result["palette_coverage"] = round(float(within.mean()), 3)
            result["palette_delta_e"] = delta_e
            result["palette_share_by_role"] = {
                f"{e['role'] or i}:{e['hex']}": round(float((within & (nearest == i)).mean()), 3)
                for i, e in enumerate(palette)
            }
    return result



def rules(values, config):
    result = {}
    for key, rule in config.items():
        if key not in values:
            result[key] = dict(value=None, rule=rule, pass_=False)
            continue
        op, threshold = rule
        if op not in (">=", "<=") or not np.isfinite(threshold):
            raise ValueError('invalid quality threshold')
        value = values[key]
        result[key] = dict(value=value, rule=rule, pass_=bool(np.isfinite(value) and (value >= threshold if op == ">=" else value <= threshold)))
    return {k: dict(value=v['value'], rule=v['rule'], **{'pass':v['pass_']}) for k,v in result.items()}


def measure(path, mask=None, palette=None, delta_e=20):
    with Image.open(path) as image:
        rgb = np.asarray(image.convert('RGB'), dtype=np.uint8)
    excluded = None
    if mask:
        with Image.open(mask) as image:
            excluded = np.asarray(image.convert('L')) > 0
        if excluded.shape != rgb.shape[:2]:
            raise ValueError('mask shape mismatch')
    if not np.isfinite(delta_e) or delta_e < 0:
        raise ValueError('invalid palette distance')
    entries = [dict(role=str(i), hex=color, rgb=hex_to_rgb(color)) for i,color in enumerate(palette)] if palette else None
    metrics = compute(rgb, excluded, entries, delta_e)
    # Lab of the mean sRGB triplet, rather than mean Lab.
    mean_lab = srgb_to_lab(rgb.reshape(-1,3).mean(axis=0,keepdims=True))[0]
    return metrics, mean_lab


def absolute(metrics, config):
    thresholds = {k:v for k,v in GATES.items() if k != 'palette_coverage' or config.get('palette')}
    thresholds.update(config.get('absolute', {}))
    if not set(thresholds) <= set(GATES):
        raise ValueError('unknown absolute metric')
    gates = rules(metrics, thresholds)
    band = config.get('exposure_band')
    if band is not None:
        if len(band) != 2 or not all(np.isfinite(band)) or band[0] > band[1]:
            raise ValueError('invalid exposure band')
        value=metrics['luminance_p50']
        gates['luminance_p50_in_band'] = dict(value=value,rule=band,**{'pass':band[0] <= value <= band[1]})
    return gates


def evaluate(image, config, baseline=None, mask=None, baseline_mask=None):
    if set(config) - {'absolute','relative','palette','delta_e','exposure_band'}:
        raise ValueError('unknown image quality configuration')
    if config.get('relative') and baseline is None:
        raise ValueError('configured relative gates require a baseline')
    v, lab = measure(image,mask,config.get('palette'),config.get('delta_e',20))
    gates = absolute(v,config)
    failed=[k for k,g in gates.items() if not g['pass']]
    report=dict(schema='saccade-image-quality.v1',metrics=v,gates=gates,absolute_pass=not failed,failed_gates=failed,
                authority='deterministic image measurements; no baseline approval',trusted_for=[])
    report['pass'] = not failed
    if baseline:
        b, base_lab=measure(baseline,baseline_mask,config.get('palette'),config.get('delta_e',20))
        values={
            'edge_density_ratio':v['edge_density']/max(b['edge_density'],1e-9),
            'luminance_contrast_ratio':v['luminance_contrast']/max(b['luminance_contrast'],1e-9),
            'median_luma_ratio_min':v['luminance_p50']/max(b['luminance_p50'],1.0),
            'median_luma_ratio_max':v['luminance_p50']/max(b['luminance_p50'],1.0),
            'colour_entropy_delta_abs':abs(v['colour_entropy_bits']-b['colour_entropy_bits']),
            'dominant_share_delta':v['dominant_colour_share']-b['dominant_colour_share'],
            'clipped_share_delta':v['clipped_share_outside_mask']-b['clipped_share_outside_mask'],
            'mean_colour_delta_e':float(np.linalg.norm(lab-base_lab)),
        }
        thresholds=dict(REGRESSION_GATES,**config.get('relative',{}))
        if set(thresholds) - set(REGRESSION_GATES):
            raise ValueError('unknown relative metric')
        regression=rules(values,thresholds)
        # Predicates use unrounded ratios; output rounds only after comparison.
        for gate in regression.values(): gate['value']=round(gate['value'],4)
        base_gates=absolute(b,config)
        new=[k for k in failed if base_gates.get(k,{}).get('pass',True)]
        failed_relative=[k for k,g in regression.items() if not g['pass']]
        report.update(baseline_metrics=b,regression=regression,new_absolute_failures=new,
                      failed_regression_gates=failed_relative,flagged=bool(new or failed_relative))
        report['pass'] = report['absolute_pass'] and not failed_relative
    return report


def selftest():
    """Independent constructed-positive/negative truth through configured gates."""
    import tempfile
    with tempfile.TemporaryDirectory() as temp:
        root=Path(temp)
        noise=np.random.default_rng(7).integers(0,254,size=(90,160,3),dtype=np.uint8)
        Image.fromarray(noise).save(root/'positive.png')
        Image.new('RGB',(160,90),(128,128,128)).save(root/'negative.png')
        perfect=evaluate(root/'positive.png',{},root/'positive.png')
        wrong=evaluate(root/'negative.png',{},root/'positive.png')
        cases={'oracle-perfect':perfect['pass'] and not perfect['flagged'],
               'oracle-wrong':not wrong['pass'] and wrong['flagged'] and bool(wrong['new_absolute_failures'])}
    return dict(status='PASS' if all(cases.values()) else 'FAIL',provider_calls=0,cases=cases)


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('image',type=Path)
    parser.add_argument('--config',type=Path,required=True)
    parser.add_argument('--baseline',type=Path)
    parser.add_argument('--mask',type=Path)
    parser.add_argument('--baseline-mask',type=Path)
    parser.add_argument('--out',type=Path,required=True)
    parser.add_argument('--strict',action='store_true')
    args=parser.parse_args()
    report=evaluate(args.image,read(args.config),args.baseline,args.mask,args.baseline_mask)
    with args.out.open('x') as output: json.dump(report,output,indent=2,allow_nan=False);output.write('\n')
    return 1 if args.strict and not report['pass'] else 0

if __name__=='__main__': raise SystemExit(main())
