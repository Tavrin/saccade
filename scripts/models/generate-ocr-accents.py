#!/usr/bin/env python3
"""Generate frozen OCR contracts before inference; Pillow required, no network."""
import argparse
import hashlib
import json
import pathlib
import unicodedata
from PIL import Image, ImageDraw, ImageFont

REVIEW_STATUS = 'generated, coordinator-reviewed (2026-10-06)'
REVIEW = 'OCR-CONTRACT-REVIEW-2026-10-06.md'


def strip_accents(text):
    # NFD does not decompose Latin ligatures or sharp s.
    text = text.translate(str.maketrans({'œ': 'oe', 'Œ': 'OE', 'æ': 'ae', 'Æ': 'AE', 'ß': 'ss'}))
    return ''.join(c for c in unicodedata.normalize('NFD', text)
                   if not unicodedata.combining(c))


def digest(path):
    return hashlib.sha256(pathlib.Path(path).read_bytes()).hexdigest()


p = argparse.ArgumentParser()
p.add_argument('out', type=pathlib.Path)
a = p.parse_args()
a.out.mkdir(parents=True, exist_ok=True)
fonts = [
    ('/usr/share/fonts/truetype/dejavu/DejaVuSerif.ttf', '/usr/share/doc/fonts-dejavu-core/copyright', 'Bitstream Vera; DejaVu changes public domain'),
    ('/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf', '/usr/share/doc/fonts-dejavu-core/copyright', 'Bitstream Vera; DejaVu changes public domain'),
    ('/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf', '/usr/share/doc/fonts-liberation/copyright', 'SIL Open Font License 1.1'),
]
# Keep all prior text, rendering, thresholds and image identities unchanged.
phrases = {
    'fr': 'café très fête à garçon hôpital où cœur',
    'de': 'Grüße für schöne Straße',
    'es': 'El señor tomó café y está aquí',
    'fr-uppercase': 'ÉTÉ À PARIS : ÇA COÛTE CHER, ÎLE DE NOËL',
    'fr-rare-lowercase': 'maïs, naïve, âme, dîner, piqûre, Noël, «guillemets», l’été, aujourd’hui',
    'fr-ligatures': 'cœur, œuvre, sœur, ex æquo',
    'fr-numbers': '1 234,56 € – 12 h 30 – 3e',
    # The quoted review uses ASCII spaces but explicitly requests thin spaces.
    # Add coverage without replacing that quoted expectation.
    'fr-numbers-thin-space': '1\u202f234,56\u202f€ – 12\u202fh\u202f30 – 3e',
}
contracts = []
for font_path, lic_path, lic in fonts:
    for size in [32, 40, 48]:
        font = ImageFont.truetype(font_path, size)
        for group, text in phrases.items():
            b = font.getbbox(text)
            image = Image.new('RGB', (b[2] - b[0] + 80, b[3] - b[1] + 80), 'white')
            ImageDraw.Draw(image).text((40 - b[0], 40 - b[1]), text, font=font, fill='black')
            name = f'{group}-{pathlib.Path(font_path).stem}-{size}.png'
            image.save(a.out / name)
            # Controls start from truth, so an existing OCR failure cannot make
            # an ineffective/no-op control look rejected.
            controls = [{'kind': 'accent-stripping', 'text': strip_accents(text)}]
            numeric = group.startswith('fr-numbers')
            if numeric:
                controls.append({'kind': 'format-stripping', 'text': text.translate(
                    str.maketrans({'€': 'EUR', '–': '-', ',': '.', '\u202f': ' '}))})
            contracts.append(dict(
                group='fr-numbers' if numeric else group,
                variant='thin-space' if group.endswith('thin-space') else 'quoted',
                image=name, image_sha256=digest(a.out / name), expected_text=text,
                max_cer=0.02, max_wer=0.10, require_exact_accented_words=True,
                required_exact_strings=[text] if numeric else [w for w in text.split() if not w.isascii()],
                negative_controls=controls, review_status=REVIEW_STATUS,
                font={'name': pathlib.Path(font_path).name, 'sha256': digest(font_path),
                      'size': size, 'license': lic, 'license_sha256': digest(lic_path)},
            ))
            (a.out / (pathlib.Path(font_path).stem + '-LICENSE.txt')).write_bytes(pathlib.Path(lic_path).read_bytes())
(a.out / 'contracts.json').write_text(json.dumps({
    'schema': 'saccade-ocr-accent-contracts.v1',
    'thresholds_declared_before_inference': True,
    'review_status': REVIEW_STATUS,
    'coordinator_review_date': '2026-10-06',
    'coordinator_review': REVIEW,
    'negative_control_policy': 'truth-derived; unchanged accent-stripping controls FAIL (numeric text contains no accented letters)',
    'fixtures': contracts,
}, ensure_ascii=False, indent=2) + '\n')
print(f'Generated {len(contracts)} frozen contracts')
