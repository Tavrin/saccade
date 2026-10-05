#!/usr/bin/env python3
"""Seeded renderer, independent pixel witnesses and immutable constructed epoch freeze."""
import argparse
import hashlib
import io
import json
import math
from pathlib import Path
import random
import sys
from PIL import Image, ImageDraw, ImageFont, __version__ as PIL_VERSION
from policy import SCHEMA, POLICY, WORKLOADS, ORACLE_SCHEMA

ROOT = Path(__file__).resolve().parents[2]
FONT_DIR = Path("/usr/share/fonts/truetype/dejavu")
FONT_FILES = [FONT_DIR / "DejaVuSans.ttf", FONT_DIR / "DejaVuSerif.ttf"]
FONT_LICENSE = Path("/usr/share/doc/fonts-dejavu-core/copyright")

def encoded(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()

def digest(data):
    return "sha256:" + hashlib.sha256(data).hexdigest()

def put(path, value):
    path.write_bytes(encoded(value) + b"\n")

def gate_source_hash():
    paths=set((ROOT/"scripts/assist").glob("*.py"))
    for crate in ("saccade-core","saccade"):
        paths.update((ROOT/"crates"/crate/"src").rglob("*.rs"))
        paths.update((ROOT/"crates"/crate/"tests").rglob("*.rs"))
        paths.update((ROOT/"crates"/crate/"examples").rglob("*.rs"))
        paths.add(ROOT/"crates"/crate/"Cargo.toml")
    paths.update((ROOT/"crates/saccade-core/schemas").glob("*.json"))
    paths.update((ROOT/"vendor/butteraugli").rglob("*.rs"))
    paths.add(ROOT/"vendor/butteraugli/Cargo.toml")
    paths.update(ROOT/path for path in ["scripts/gates-wave4.sh","scripts/qualify-wave4.sh","Cargo.toml","Cargo.lock"])
    return digest(encoded([(str(p.relative_to(ROOT)),digest(p.read_bytes())) for p in sorted(paths)]))

def png(image):
    out = io.BytesIO()
    image.save(out, format="PNG", optimize=False, compress_level=6)
    return out.getvalue()

def runs(bits):
    out = []
    for index, bit in enumerate(bits):
        if bit:
            if out and sum(out[-1]) == index:
                out[-1][1] += 1
            else:
                out.append([index, 1])
    return out

def mask(width, height, rect):
    bits = bytearray(width * height)
    x, y, w, h = rect
    for row in range(y, min(y+h, height)):
        bits[row*width+x:row*width+min(x+w, width)] = bytes([1]) * min(w, width-x)
    return bits

def node(identity, text, box):
    return dict(id=identity, text=text, role="button", bounds=box, reading_order=None,
                keyboard_order=None, disclosure=False, ocr_confidence=None)

def source(image_bytes, dimensions, nodes):
    return dict(schema="saccade-ui-source.v1", capture_sha256=digest(image_bytes)[7:],
                dimensions=dimensions, kind="dom", producer={"renderer":"constructed-ui/1"},
                nodes=nodes, complete=True)

def render(seed, family, kind, workload):
    """Mutation command and oracle independently inspect actual rendered pixels."""
    rng = random.Random(seed)
    dpr = 1 + family % 2
    width = ([240, 320, 480][family % 3] + rng.randrange(40)) * dpr
    height = 160 * dpr
    theme = family % 2
    background = (240+rng.randrange(12),)*3 if not theme else (18+rng.randrange(16),)*3
    button = (25+rng.randrange(30),65+rng.randrange(30),140+rng.randrange(35))
    foreground = (255, 255, 255)
    font = ImageFont.truetype(str(FONT_FILES[family % 2]), (12 + family % 3 + rng.randrange(3)) * dpr)
    label = ["Continuer", "État enregistré", "Vérifier les éléments sélectionnés"][family % 3] + " " + str(seed%100000)
    target = [(16+rng.randrange(16))*dpr,(24+rng.randrange(16))*dpr,min(width-64*dpr,(180+rng.randrange(60))*dpr),54*dpr]
    baseline = Image.new("RGB", (width,height), background)
    draw = ImageDraw.Draw(baseline)
    # Thirty-two actual template layouts: four header row structures by eight
    # column structures. Heldout families are unseen combinations, not relabelled
    # copies of the same modulo-six mutation template.
    header_rows,header_columns=1+family//8,1+family%8
    header_colour=(95,103,115) if theme else (155,163,175)
    cell_width=(width-16*dpr)//header_columns
    for row in range(header_rows):
        for column in range(header_columns):
            left=8*dpr+column*cell_width
            top=(4+row*4)*dpr
            draw.rectangle((left,top,left+cell_width-3*dpr,top+2*dpr-1),fill=header_colour)
    x,y,w,h = target
    draw.rectangle((x,y,x+w-1,y+h-1), fill=button)
    # Deterministic line wrapping with exact glyph patches retained by the oracle.
    lines, line = [], ""
    for word in label.split():
        candidate = (line + " " + word).strip()
        if line and draw.textbbox((0,0), candidate, font=font)[2] > w-12*dpr:
            lines.append(line); line=word
        else:
            line=candidate
    lines.append(line)
    for index, text in enumerate(lines):
        draw.text((x+6*dpr,y+5*dpr+index*16*dpr),text,font=font,fill=foreground)
    draw.rectangle((x,height-24,x+w-1,height-9),fill=button)
    after = baseline.copy()
    mode = family % 6
    important = kind == "challenge"
    if important:
        painter = ImageDraw.Draw(after)
        if mode == 0:  # remove known text and its target
            painter.rectangle((x,y,x+w-1,y+h-1),fill=background)
        elif mode == 1:  # known displacement
            patch = baseline.crop((x,y,x+w,y+h))
            painter.rectangle((x,y,x+w-1,y+h-1),fill=background)
            after.paste(patch,(x+8*dpr,y+8*dpr))
        elif mode == 2:  # appearance change
            # Replace only exact button-colour samples, preserving glyphs.
            after.putdata([(173,45,52) if p==button else p for p in baseline.getdata()])
        elif mode == 3:  # clipping/occlusion of glyph-bearing pixels
            painter.rectangle((x+w//2,y,x+w-1,y+h-1),fill=background)
        elif mode == 4:  # wrapping: move rendered rows inside the known target
            painter.rectangle((x,y,x+w-1,y+h-1),fill=button)
            for index,word in enumerate(label.split()):
                painter.text((x+6*dpr,y+3*dpr+index*15*dpr),word,font=font,fill=foreground)
            # A one-word label has an explicit geometry mutation as well.
            painter.line((x,y,x+w-1,y),fill=foreground,width=dpr)
        else:  # inverse mutation (known original is the changed side)
            painter.rectangle((x,y,x+w-1,y+h-1),fill=background)
            baseline,after=after,baseline
    elif kind == "control" and workload == "audit_mask":
        # Benign dynamic decorative strip; required label pixels are unchanged.
        ImageDraw.Draw(after).rectangle((0,height-12*dpr,width-1,height-1),fill=(rng.randrange(80,180),90,90))
    # Check-ui/routing challenges exercise known label absence, clipping and occlusion.
    if workload in ("check_ui","routing") and important:
        ImageDraw.Draw(after).rectangle((x,y,x+w-1,y+h-1),fill=background)
    braw, araw = png(baseline), png(after)
    # Independent native samples, not mutation flags, determine truth.
    b = list(Image.open(io.BytesIO(braw)).getdata())
    a = list(Image.open(io.BytesIO(araw)).getdata())
    changed = [int(left != right) for left,right in zip(b,a)]
    # Exact patch identity verifies the known text/raster outcome.
    before_patch = baseline.crop((x,y,x+w,y+h)).tobytes()
    after_patch = after.crop((x,y,x+w,y+h)).tobytes()
    # The canonical rendered label template is built separately for presence testing.
    template = Image.new("RGB",(width,height),background)
    td = ImageDraw.Draw(template)
    td.rectangle((x,y,x+w-1,y+h-1),fill=button)
    for index,text in enumerate(lines):
        td.text((x+6*dpr,y+5*dpr+index*16*dpr),text,font=font,fill=foreground)
    template_patch = template.crop((x,y,x+w,y+h)).tobytes()
    def glyphs(text,at):
        bounds=font.getbbox(text)
        glyph=Image.new("L",(bounds[2]-bounds[0],bounds[3]-bounds[1]),0)
        ImageDraw.Draw(glyph).text((-bounds[0],-bounds[1]),text,font=font,fill=255)
        return {(at[0]+bounds[0]+i%glyph.width,at[1]+bounds[1]+i//glyph.width)
                for i,value in enumerate(glyph.getdata()) if value==255}
    base_glyphs=set().union(*(glyphs(text,(x+6*dpr,y+5*dpr+index*16*dpr)) for index,text in enumerate(lines)))
    shifted_glyphs={(gx+8*dpr,gy+8*dpr) for gx,gy in base_glyphs}
    wrapped_glyphs=set().union(*(glyphs(word,(x+6*dpr,y+3*dpr+index*15*dpr)) for index,word in enumerate(label.split())))
    def present(image):
        return any(expected and all(0<=gx<width and 0<=gy<height and image.getpixel((gx,gy))==foreground for gx,gy in expected)
                   for expected in (base_glyphs,shifted_glyphs,wrapped_glyphs))
    label_complete=present(after)
    before_label_complete=present(baseline)
    exclusions=[]
    if workload == "audit_mask":
        rects = [target,[x+w//3,y,2*w//3,h]] if important else [[0,height-12*dpr,width,12*dpr]]
        for index,rect in enumerate(rects):
            bits=mask(width,height,rect)
            exclusions.append(dict(id=f"exclusion-{index}",origin="constructed-capture/1",
                rationale="declared dynamic area",dimensions=[width,height],runs=runs(bits),
                membership_hash=digest(bits),original_pixels=kind != "unavailable"))
    union = bytearray(width*height)
    for exclusion in exclusions:
        for start,length in exclusion["runs"]:
            union[start:start+length]=bytes([1])*length
    label_changed = before_patch != after_patch
    masked_important = any(union[row*width+col] and changed[row*width+col]
                           for row in range(y,y+h) for col in range(x,x+w))
    wanted = "unverifiable" if kind=="unavailable" else (
        ("observed" if label_complete else "not_observed") if workload in ("check_ui","routing") else
        ("observed" if masked_important else "not_observed") if workload=="audit_mask" else
        ("observed" if any(changed) else "not_observed"))
    witnesses=dict(changed_pixels=sum(changed),label_patch_hash=digest(after_patch),
                   label_template_hash=digest(template_patch),label_complete=label_complete,before_label_complete=before_label_complete,
                   target=target,masked_important_change=masked_important,
                   union_pixels=sum(union),dimensions=[width,height],lines=len(lines),
                   text=label,background=list(background))
    # Preregistered independent render admissibility.
    checks=[len(b)==width*height,len(a)==width*height,
            kind!="challenge" or workload in ("check_ui","routing") or any(changed),
            workload not in ("check_ui","routing") or kind!="control" or label_complete,
            workload!="audit_mask" or kind!="challenge" or masked_important]
    if not all(checks):
        return None
    return dict(before=braw,after=araw,dimensions=[width,height],target=target,
                label=label,exclusions=exclusions,wanted=wanted,witnesses=witnesses,
                font_family=family%2,dpr=dpr,theme=theme)

def freeze(out, target, seed, gemini_revision, jev_revision):
    if target <= 0 or target > 1000 or target % 5:
        raise ValueError("target must be a positive multiple of five at most 1000")
    if not gemini_revision or not jev_revision:
        raise ValueError("freeze exact observed model revisions")
    # Distinct families, seeds and roots; descendants/orders/retries retain root/split.
    families = {"development":list(range(0,8)),"calibration":list(range(8,12)),"heldout":list(range(12,32))}
    versions = dict(gate_source_hash=gate_source_hash(),generator_hash=digest(Path(__file__).read_bytes()),policy_hash=digest(encoded(POLICY)),
                    pillow=PIL_VERSION,fonts=[digest(p.read_bytes()) for p in FONT_FILES],
                    font_license_hash=digest(FONT_LICENSE.read_bytes()),
                    workflow_hash=digest((ROOT/"crates/saccade-core/src/assist/workflow.rs").read_bytes()),
                    schema_hash=digest((ROOT/"crates/saccade-core/schemas/saccade-assist.v1.schema.json").read_bytes()))
    metadata = dict(schema=SCHEMA,epoch="wave4-constructed/2",seed=seed,target_per_workload=target,
                    families=families,policy=POLICY,versions=versions,
                    models={"gemini":"gemini-3.8-flash","gemini_revision":gemini_revision,
                            "jev":"jev-1.13.0","jev_revision":jev_revision},
                    licence="Generated pixels: MIT OR Apache-2.0; DejaVu rendered fonts: see font-license.txt")
    if out.exists() and any(out.iterdir()):
        raise ValueError("freeze requires a new empty output directory; never rewrite a frozen epoch")
    out.mkdir(parents=True,exist_ok=True)
    (out/"images").mkdir()
    (out/"font-license.txt").write_bytes(FONT_LICENSE.read_bytes())
    cases, oracle, excluded = [],{},[]
    for split,family_ids in families.items():
        count = target if split=="heldout" else min(target,20)
        for workload in WORKLOADS:
            for index in range(count):
                kind = "challenge" if index < count*3//5 else "control" if index < count*4//5 else "unavailable"
                family=family_ids[index%len(family_ids)]
                case_seed=seed + 100000*WORKLOADS.index(workload) + 10000*list(families).index(split)+index
                root=digest(encoded([seed,workload,split,family,case_seed]))
                case=render(case_seed,family,kind,workload)
                if case is None:
                    excluded.append(dict(root=root,reason="preregistered rendered witness failed",split=split,workload=workload))
                    continue
                prefix=root[7:]
                before=f"images/{prefix}-p1.png";after=f"images/{prefix}-p2.png"
                (out/before).write_bytes(case["before"]);(out/after).write_bytes(case["after"])
                task="check_ui" if workload=="routing" else workload
                case_record=dict(case_id=root,root_id=root,family=f"family-{family:02}",split=split,
                    workload=workload,task=task,generator_seed=case_seed,before=before,after=after,
                    before_hash=digest(case["before"]),after_hash=digest(case["after"]),
                    dimensions=case["dimensions"],target=case["target"],label=case["label"],
                    original_pixels=kind!="unavailable",complete=kind!="unavailable",
                    exclusions=case["exclusions"],source=None,
                    structured_packet={},counterfactual=None)
                if workload=="routing" and kind=="control":
                    # Exact source geometry answers non-overlap without receiving oracle labels.
                    dims=case["dimensions"];x,y,w,h=case["target"]
                    nodes=[node("target-a","",[x,y,w,h]),node("target-b","",[x,dims[1]-24,w,16])]
                    packet=source(case["after"],dims,nodes)
                    path=f"images/{prefix}-source.json";put(out/path,packet)
                    case_record["source"]=path
                    case_record["condition"]={"kind":"non_overlap","first":"target-a","second":"target-b"}
                    wanted="observed"
                else:
                    wanted=case["wanted"]
                    case_record["condition"]={"kind":"label_visible","label":case["label"]} if task=="check_ui" else None
                if workload=="routing" and kind=="challenge":
                    # Same empty structured packet, opposite visible label truth.
                    opposite=render(case_seed,family,"control",workload)
                    counter=f"images/{prefix}-counterfactual.png"
                    (out/counter).write_bytes(opposite["after"])
                    case_record["counterfactual"]={"path":counter,"hash":digest(opposite["after"]),"root_id":root,"split":split,"structured_packet":{}}
                    assert case_record["structured_packet"]==case_record["counterfactual"]["structured_packet"]
                    assert wanted!=opposite["wanted"]
                cases.append(case_record)
                oracle[root]=dict(root_id=root,family=case_record["family"],split=split,workload=workload,
                    category=kind,expected_outcome=wanted,important=kind=="challenge",
                    necessary_vision=workload=="routing" and kind=="challenge",
                    rendered_witness=case["witnesses"],oracle_verified=True,
                    counterfactual_witness=opposite["witnesses"] if case_record["counterfactual"] else None,
                    diagnostic_statements=["presence:"+("present" if case["witnesses"]["label_complete"] else "absent")],
                    assertion_vocabulary={"presence:present":case["witnesses"]["label_complete"],
                                          "presence:absent":not case["witnesses"]["label_complete"],
                                          "appearance:changed":case["witnesses"]["changed_pixels"]>0,
                                          "appearance:unchanged":case["witnesses"]["changed_pixels"]==0,
                                          "text:"+case["label"]:case["witnesses"]["label_complete"]})
    metadata["cases"]=cases;metadata["exclusions"]=excluded
    oracle_document={"schema":ORACLE_SCHEMA,"cases":oracle}
    metadata["oracle_hash"]=digest(encoded(oracle_document));metadata["manifest_hash"]=digest(encoded(metadata))
    put(out/"manifest.json",metadata);put(out/"oracle.json",oracle_document)
    return metadata

def verify(directory):
    raw=(directory/"manifest.json").read_bytes()
    if len(raw)>32*1024*1024: raise ValueError("manifest too large")
    manifest=json.loads(raw); claimed=manifest.pop("manifest_hash")
    if digest(encoded(manifest))!=claimed: raise ValueError("manifest drift")
    if manifest["schema"]!=SCHEMA or manifest["policy"]!=POLICY: raise ValueError("epoch/policy drift")
    if manifest["versions"]["generator_hash"]!=digest(Path(__file__).read_bytes()): raise ValueError("generator drift")
    if manifest["versions"]["workflow_hash"]!=digest((ROOT/"crates/saccade-core/src/assist/workflow.rs").read_bytes()): raise ValueError("prompt/workflow drift")
    if manifest["versions"]["pillow"]!=PIL_VERSION or manifest["versions"]["fonts"]!=[digest(p.read_bytes()) for p in FONT_FILES]: raise ValueError("renderer/font drift")
    if manifest["versions"]["gate_source_hash"]!=gate_source_hash(): raise ValueError("implementation/gate source drift")
    oracle_document=json.loads((directory/"oracle.json").read_bytes())
    if set(oracle_document)!={"schema","cases"} or oracle_document["schema"]!=ORACLE_SCHEMA or digest(encoded(oracle_document))!=manifest["oracle_hash"]: raise ValueError("oracle schema/hash drift")
    oracle=oracle_document["cases"]
    expected_families={"development":list(range(8)),"calibration":list(range(8,12)),"heldout":list(range(12,32))}
    target=manifest["target_per_workload"]
    if manifest["epoch"]!="wave4-constructed/2" or manifest["families"]!=expected_families or not isinstance(target,int) or target<=0 or target>1000 or target%5:
        raise ValueError("preregistered split/epoch drift")
    if manifest["models"]["gemini"]!="gemini-3.8-flash" or manifest["models"]["jev"]!="jev-1.13.0" or not all(manifest["models"][m+"_revision"] for m in ("gemini","jev")):
        raise ValueError("pinned model binding drift")
    families={};roots=set()
    for case in manifest["cases"]:
        if case["root_id"] in roots: raise ValueError("duplicate root")
        roots.add(case["root_id"])
        if case["family"] in families and families[case["family"]]!=case["split"]: raise ValueError("family split leakage")
        families[case["family"]]=case["split"]
        for name in ("before","after"):
            path=directory/case[name]
            if path.is_symlink() or not path.resolve().is_relative_to(directory.resolve()): raise ValueError("unsafe fixture path")
            data=path.read_bytes()
            if digest(data)!=case[name+"_hash"]: raise ValueError("fixture hash drift")
            im=Image.open(io.BytesIO(data));im.load()
            if list(im.size)!=case["dimensions"]: raise ValueError("dimension drift")
        family=int(case["family"].split("-")[1])
        count=manifest["target_per_workload"] if case["split"]=="heldout" else min(manifest["target_per_workload"],20)
        index=case["generator_seed"]-manifest["seed"]-100000*WORKLOADS.index(case["workload"])-10000*["development","calibration","heldout"].index(case["split"])
        expected_category="challenge" if index<count*3//5 else "control" if index<count*4//5 else "unavailable"
        if index<0 or index>=count or family!=manifest["families"][case["split"]][index%len(manifest["families"][case["split"]])] or oracle[case["root_id"]]["category"]!=expected_category:
            raise ValueError("constructed split/seed/category drift")
        if digest(encoded([manifest["seed"],case["workload"],case["split"],family,case["generator_seed"]]))!=case["root_id"]:
            raise ValueError("constructed root identity drift")
        rendered=render(case["generator_seed"],family,expected_category,case["workload"])
        if rendered is None: raise ValueError("rendered witness admission drift")
        truth=oracle[case["root_id"]]
        if any(truth[field]!=case[field] for field in ("root_id","family","split","workload")):
            raise ValueError("oracle root metadata drift")
        task="check_ui" if case["workload"]=="routing" else case["workload"]
        source_control=case["workload"]=="routing" and expected_category=="control"
        expected_condition={"kind":"non_overlap","first":"target-a","second":"target-b"} if source_control else {"kind":"label_visible","label":rendered["label"]} if task=="check_ui" else None
        if case["task"]!=task or case["original_pixels"]!=(expected_category!="unavailable") or case["complete"]!=(expected_category!="unavailable") or case["exclusions"]!=rendered["exclusions"] or case["structured_packet"]!={} or case["condition"]!=expected_condition:
            raise ValueError("manual public input or evidence policy drift")
        if source_control:
            x,y,w,h=rendered["target"];dims=rendered["dimensions"]
            expected_source=source(rendered["after"],dims,[node("target-a","",[x,y,w,h]),node("target-b","",[x,dims[1]-24,w,16])])
            source_path=directory/case["source"]
            if source_path.is_symlink() or not source_path.resolve().is_relative_to(directory.resolve()) or json.loads(source_path.read_bytes())!=expected_source:
                raise ValueError("constructed source witness drift")
        elif case["source"] is not None:
            raise ValueError("unexpected constructed source evidence")
        if bool(case["counterfactual"])!=(case["workload"]=="routing" and expected_category=="challenge"):
            raise ValueError("necessary-pixel counterfactual missing or invented")
        expected_outcome="observed" if case["workload"]=="routing" and expected_category=="control" else rendered["wanted"]
        expected_fields={"root_id","family","split","workload","category","expected_outcome","important","necessary_vision","rendered_witness","oracle_verified","counterfactual_witness","diagnostic_statements","assertion_vocabulary"}
        if set(truth)!=expected_fields or truth["expected_outcome"]!=expected_outcome or truth["important"]!=(expected_category=="challenge") or truth["necessary_vision"]!=(case["workload"]=="routing" and expected_category=="challenge") or truth["diagnostic_statements"]!=["presence:"+("present" if rendered["witnesses"]["label_complete"] else "absent")]:
            raise ValueError("manual or unreproduced oracle truth is forbidden")
        expected_assertions={"presence:present":rendered["witnesses"]["label_complete"],"presence:absent":not rendered["witnesses"]["label_complete"],"appearance:changed":rendered["witnesses"]["changed_pixels"]>0,"appearance:unchanged":rendered["witnesses"]["changed_pixels"]==0,"text:"+rendered["label"]:rendered["witnesses"]["label_complete"]}
        if truth["assertion_vocabulary"]!=expected_assertions or case["label"]!=rendered["label"] or case["target"]!=rendered["target"]:
            raise ValueError("fixture content contract drift")
        if rendered is None or digest(rendered["before"])!=case["before_hash"] or digest(rendered["after"])!=case["after_hash"] or rendered["witnesses"]!=oracle[case["root_id"]]["rendered_witness"]:
            raise ValueError("independent renderer/oracle witnesses no longer reproduce")
        if not oracle[case["root_id"]]["oracle_verified"]: raise ValueError("unverified oracle")
        if case["counterfactual"]:
            child=case["counterfactual"]
            if child["root_id"]!=case["root_id"] or child["split"]!=case["split"] or child["structured_packet"]!=case["structured_packet"]: raise ValueError("counterfactual split/packet leakage")
            opposite=render(case["generator_seed"],int(case["family"].split("-")[1]),"control",case["workload"])
            if digest((directory/child["path"]).read_bytes())!=child["hash"] or digest(opposite["after"])!=child["hash"] or opposite["witnesses"]!=oracle[case["root_id"]]["counterfactual_witness"]: raise ValueError("counterfactual hash/oracle drift")
    manifest["manifest_hash"]=claimed
    return manifest,oracle

if __name__=="__main__":
    parser=argparse.ArgumentParser()
    parser.add_argument("--out",type=Path,required=True)
    parser.add_argument("--target-per-workload",type=int,default=1000)
    parser.add_argument("--seed",type=int,default=4406)
    parser.add_argument("--gemini-revision")
    parser.add_argument("--jev-revision",default="jev-1.13.0")
    parser.add_argument("--verify",action="store_true")
    args=parser.parse_args()
    try:
        data=verify(args.out)[0] if args.verify else freeze(args.out,args.target_per_workload,args.seed,args.gemini_revision,args.jev_revision)
        print(json.dumps({"schema":SCHEMA,"manifest_hash":data["manifest_hash"],"cases":len(data["cases"]),"excluded":len(data["exclusions"])}))
    except (ValueError,OSError) as error:
        print("constructed freeze refused: "+str(error),file=sys.stderr);sys.exit(4)
