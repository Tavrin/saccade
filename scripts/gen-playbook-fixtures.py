#!/usr/bin/env python3
"""Generate the acceptance inputs using only Python's standard library.

No network, source imagery, fonts or benchmark acquisition. MIT OR Apache-2.0.
"""
import hashlib
import importlib.util
import json
import pathlib
import struct
import sys

SPEC = importlib.util.spec_from_file_location("guide_fixtures", pathlib.Path(__file__).with_name("gen-guide-fixtures.py"))
GUIDE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(GUIDE)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()


def write_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def pdf(path, changed):
    # Two pages, rectangles only: no host fonts, metadata or timestamps.
    objects = [b"<< /Type /Catalog /Pages 2 0 R >>", b"<< /Type /Pages /Kids [3 0 R 5 0 R] /Count 2 >>"]
    for page in range(2):
        stream = b"0.2 0.3 0.4 rg 12 12 40 40 re f\n"
        if page == 1 and changed:
            stream += b"1 0 0 rg 20 65 30 15 re f\n"
        objects += [f"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 96 96] /Resources << >> /Contents {4 + page * 2} 0 R >>".encode(),
                    f"<< /Length {len(stream)} >>\nstream\n".encode() + stream + b"endstream"]
    data = b"%PDF-1.4\n"
    offsets = [0]
    for n, obj in enumerate(objects, 1):
        offsets.append(len(data))
        data += f"{n} 0 obj\n".encode() + obj + b"\nendobj\n"
    start = len(data)
    data += f"xref\n0 {len(offsets)}\n0000000000 65535 f \n".encode()
    data += b"".join(f"{offset:010d} 00000 n \n".encode() for offset in offsets[1:])
    data += f"trailer\n<< /Size {len(offsets)} /Root 1 0 R >>\nstartxref\n{start}\n%%EOF\n".encode()
    path.write_bytes(data)


def geometry(root):
    vertices = [(-1., -1., -1.), (1., -1., -1.), (1., 1., -1.), (-1., 1., -1.),
                (-1., -1., 1.), (1., -1., 1.), (1., 1., 1.), (-1., 1., 1.)]
    faces = [(0, 3, 2, 1), (4, 5, 6, 7), (0, 1, 5, 4), (3, 7, 6, 2), (0, 4, 7, 3), (1, 2, 6, 5)]
    assets = []
    meshes = []
    for name, detailed in (("source", True), ("lod", False)):
        vv, tt = list(vertices), []
        for face in faces:
            if detailed:
                center = tuple(sum(vertices[i][axis] for i in face) / 4 for axis in range(3))
                index = len(vv)
                vv.append(center)
                tt.extend((face[i], face[(i + 1) % 4], index) for i in range(4))
            else:
                tt.extend(((face[0], face[1], face[2]), (face[0], face[2], face[3])))
        text = "".join("v " + " ".join(map(str, v)) + "\n" for v in vv)
        text += "".join("f " + " ".join(str(i + 1) for i in t) + "\n" for t in tt)
        path = root / f"{name}.obj"
        path.write_text(text)
        # The OBJ loader compacts vertices in first face-reference order.
        order = list(dict.fromkeys(i for triangle in tt for i in triangle))
        remap = {old: new for new, old in enumerate(order)}
        decoded_vertices = [vv[i] for i in order]
        decoded_triangles = [tuple(remap[i] for i in triangle) for triangle in tt]
        body = b"ordered-world-triangle-geometry/1\0" + struct.pack("<Q", 6) + b"metres" + struct.pack("<Q", len(decoded_vertices))
        body += b"".join(struct.pack("<ddd", *v) for v in decoded_vertices) + struct.pack("<Q", len(tt))
        body += b"".join(struct.pack("<III", *t) for t in decoded_triangles)
        assets.append(dict(document_sha256=digest(path.read_bytes()), geometry_sha256=digest(body), lod=name))
        meshes.append((vv, tt))
    identity = [1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.]
    source_hash = digest(pathlib.Path(__file__).read_bytes())
    context = dict(renderer="procedural-triangle-raster/1", renderer_source_sha256=source_hash,
                   renderer_binary_sha256=digest(pathlib.Path(sys.executable).resolve().read_bytes()),
                   settings_sha256=digest(b"orthographic-64x64-center-samples"), lighting_sha256=digest(b"unlit"),
                   background_sha256=digest(b"rgb-20-20-20"), color_pipeline_sha256=digest(b"srgb"),
                   material_mode="override_materials", override_material_sha256=digest(b"rgb-180-120-60"), dimensions=[64, 64])
    views = []
    for name, rotation in (("front", identity),
                           ("rear", [-1.,0.,0.,0., 0.,1.,0.,0., 0.,0.,-1.,0., 0.,0.,0.,1.]),
                           ("underside", [1.,0.,0.,0., 0.,0.,1.,0., 0.,-1.,0.,0., 0.,0.,0.,1.])):
        camera = dict(convention="row_major_column_vector_right_handed_clip_z_zero_to_one",
                      model_to_world=identity, world_to_view=rotation,
                      projection=[0.5,0.,0.,0., 0.,0.5,0.,0., 0.,0.,0.25,0.5, 0.,0.,0.,1.])
        view = dict(id=name, camera=camera)
        for side, asset, (vv, tt) in zip(("reference", "candidate"), assets, meshes):
            projected = [(sum(rotation[a * 4 + k] * v[k] for k in range(3)) for a in range(3)) for v in vv]
            projected = [tuple(v) for v in projected]
            rows = GUIDE.canvas(64, 64, (20, 20, 20))
            # Orthographic triangle silhouette; constant unlit material.
            for y in range(64):
                for x in range(64):
                    p = ((x + 0.5 - 32) / 16, (32 - y - 0.5) / 16)
                    for triangle in tt:
                        abc = [projected[i] for i in triangle]
                        crosses = [(abc[(i + 1) % 3][0] - abc[i][0]) * (p[1] - abc[i][1]) -
                                   (abc[(i + 1) % 3][1] - abc[i][1]) * (p[0] - abc[i][0]) for i in range(3)]
                        area = (abc[1][0]-abc[0][0])*(abc[2][1]-abc[0][1])-(abc[1][1]-abc[0][1])*(abc[2][0]-abc[0][0])
                        if area and (min(crosses) >= 0 or max(crosses) <= 0):
                            rows[y][x] = (180, 120, 60)
                            break
            image = f"{name}-{side}.png"
            GUIDE.write_png(root / image, rows)
            view[side] = dict(image=dict(path=image, sha256=digest((root / image).read_bytes())), silhouette=None,
                              camera_sha256=digest(canonical(camera)), context_sha256=digest(canonical(context)),
                              asset_document_sha256=asset["document_sha256"], asset_geometry_sha256=asset["geometry_sha256"],
                              materials_sha256=context["override_material_sha256"], textures_sha256=[])
        views.append(view)
    write_json(root / "cameras.json", dict(schema="saccade-asset-views.v1", unit="metres", assets=assets, context=context,
                                          views=views, pixels_per_degree=67., maximum_mean_flip=0., maximum_silhouette_change=0.))


def generate(out):
    out.mkdir(parents=True, exist_ok=False)
    for name in ("local", "text", "arms", "perf", "documents", "geometry", "web", "tuning", "intake", "frames-a", "frames-b", "reuse"):
        (out / name).mkdir()
    a = GUIDE.ui(120, (30, 100, 220))
    b = GUIDE.ui(120, (220, 60, 30))
    for name, rows in (("reference", a), ("candidate", b)):
        GUIDE.write_png(out / f"local/{name}.png", rows)
    bad = GUIDE.ui(120, (220, 60, 30))
    GUIDE.rect(bad, 4, 30, 8, 8, (220, 30, 30))
    GUIDE.write_png(out / "local/collateral.png", bad)
    for name, rows, text in (("before", a, "-1.0 EUR"), ("after", b, "1.0 EUR")):
        path = out / f"text/{name}.png"
        GUIDE.write_png(path, rows)
        write_json(out / f"text/{name}-source.json", dict(schema="saccade-ui-source.v1", capture_sha256=digest(path.read_bytes()),
                  dimensions=[200,140], kind="dom", producer={"name":"procedural-source", "version":"1"}, complete=True,
                  nodes=[dict(id="value",text=text,role="text",bounds=[120.,108.,64.,20.],reading_order=0,keyboard_order=None,disclosure=False,ocr_confidence=None)]))
    stale = json.loads((out / "text/after-source.json").read_text())
    stale["capture_sha256"] = "0" * 64
    write_json(out / "text/stale-source.json", stale)
    for side, mode in (("baseline", "fixed"), ("candidate", "varied")):
        write_json(out / f"arms/{side}/capture.json", dict(producer={"binary":digest(b"synthetic-program"),"source":digest(b"synthetic-source")},run={"mode":mode}))
        GUIDE.write_png(out / f"arms/{side}/image.png", a)
    (out / "arms/fingerprint-map.toml").write_text('compare = "mapped_only"\n[fields."producer.binary"]\npath = "producer.binary"\n[fields."producer.build.commit"]\npath = "producer.source"\n[fields."run.mode"]\npath = "run.mode"\n')
    (out / "arms/strict.toml").write_text('require_valid_arms = true\nintended_variables = ["run.mode"]\nfingerprint_map = "fingerprint-map.toml"\nmeta_name = "capture.json"\n')
    for side, value in (("base", 8.), ("candidate", 4.)):
        for repeat in range(3):
            run = out / f"perf/{side}_r{repeat}"
            run.mkdir()
            GUIDE.write_png(run / "scene.png", GUIDE.sphere(1.))
            write_json(run / "capture.json", {"run":f"{side}-{repeat}","image_sha256":digest((run / "scene.png").read_bytes())})
            write_json(run / "saccade-meta.json", {"binary_sha256":digest(b"synthetic-program"),"source_head":digest(b"synthetic-source")})
            measurements = {"frame_ms":value+2., "pass_ms":value}
            write_json(run / "measurements.json", measurements)
            write_json(run / "raw.json", {"samples_ms":[value+2.] * 5})
            write_json(run / "window.json", {"frames":5,"warmup":5,"complete":True})
            pin = lambda name: dict(source=name, hash="sha256:" + digest((run / name).read_bytes()))
            context = dict(measurement=pin("measurements.json"), timer="synthetic-cpu-timer", quantum_ms=0.0001,
                sample_window=pin("window.json"), raw_samples=[pin("raw.json")], hardware={"cpu":"synthetic-cpu"},
                aggregation="joint_partition",attribution_coverage=1.,unresolved_remainder_ms=0.,
                capture_hash="sha256:"+digest((run / "capture.json").read_bytes()),configuration_hash="sha256:"+digest(b"synthetic-config"),
                producer="procedural-timing",producer_version="1", qualification=dict(status="qualified",rule="synthetic-test-only",version="1",
                checks=dict(window_complete=True,clock_qualified=True,warmup_complete=True),reasons=[]))
            write_json(run / "saccade-perf.json", dict(schema="saccade-perf.v2",kind="measurement",context=context,unit="ms",
                frame=dict(value=value+2.,samples=5,stat="p50"),terms=[dict(id="render",kind="pass",value=value),dict(id="gap",kind="gap",value=2.)],counters={}))
    for name, changed in (("before",False),("after",True)):
        pdf(out / f"documents/{name}.pdf", changed)
    geometry(out / "geometry")
    for n in range(3):
        for side in ("a", "b"):
            GUIDE.write_png(out / f"frames-{side}/frame{n:03d}.png", GUIDE.sphere(1. if side == "a" or n != 1 else .6))
    for name in ("source", "current"):
        GUIDE.write_png(out / f"tuning/{name}.png", GUIDE.photo())
    write_json(out / "tuning/tuning.json", dict(schema="saccade-imgtune.v1",images=[dict(id="image-1",source="source.png",current="current.png")],
        widths=[64,128],formats=["jpeg","webp"],qualities=[60,95],target_score=90,butteraugli_ceiling=None,adapter={"kind":"local"}))
    GUIDE.write_png(out / "intake/photo.png", GUIDE.photo())
    GUIDE.write_png(out / "intake/nested/photo.png", GUIDE.sphere(1.))
    (out / "intake/broken.png").write_bytes(b"deliberately not an image")
    GUIDE.write_png(out / "reuse/match.png", GUIDE.photo())
    GUIDE.write_png(out / "reuse/other.png", GUIDE.sphere(1.))
    (out / "web/urls.txt").write_text("/same\n/change\n/failure\n")
    write_json(out / "web/capture-options.json", dict(concurrency=1,perHostDelayMs=0,timeout=5000,randomSeed=42,fullPage=False))
    files = {str(p.relative_to(out)):digest(p.read_bytes()) for p in sorted(out.rglob("*")) if p.is_file()}
    write_json(out / "provenance.json", dict(schema="saccade-playbook-inputs.v1",licence="MIT OR Apache-2.0",
        generator_sha256=digest(pathlib.Path(__file__).read_bytes()), files=files,
        limitations=["Generated images and timings are acceptance controls, not measured application performance.",
                    "Source observations are synthetic declarations, not OCR or glyph-rendering proof.",
                    "Mesh images are software-rasterized unlit orthographic silhouettes of the exact OBJ vertices."]))


if __name__ == "__main__":
    generate(pathlib.Path(sys.argv[1]))
