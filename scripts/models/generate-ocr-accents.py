#!/usr/bin/env python3
"""Generate frozen OCR contracts before inference; Pillow required, no network."""
import argparse, hashlib, json, pathlib
from PIL import Image, ImageDraw, ImageFont
p=argparse.ArgumentParser();p.add_argument('out',type=pathlib.Path);a=p.parse_args();a.out.mkdir(parents=True,exist_ok=True)
fonts=[('/usr/share/fonts/truetype/dejavu/DejaVuSerif.ttf','/usr/share/doc/fonts-dejavu-core/copyright','Bitstream Vera; DejaVu changes public domain'),('/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf','/usr/share/doc/fonts-dejavu-core/copyright','Bitstream Vera; DejaVu changes public domain'),('/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf','/usr/share/doc/fonts-liberation/copyright','SIL Open Font License 1.1')]
phrases={'fr':'café très fête à garçon hôpital où cœur','de':'Grüße für schöne Straße','es':'El señor tomó café y está aquí'}
contracts=[]
for font_path,lic_path,lic in fonts:
 for size in [32,40,48]:
  font=ImageFont.truetype(font_path,size)
  for lang,text in phrases.items():
   b=font.getbbox(text);image=Image.new('RGB',(b[2]-b[0]+80,b[3]-b[1]+80),'white');ImageDraw.Draw(image).text((40-b[0],40-b[1]),text,font=font,fill='black')
   name=f'{lang}-{pathlib.Path(font_path).stem}-{size}.png';image.save(a.out/name)
   contracts.append(dict(image=name,image_sha256=hashlib.sha256((a.out/name).read_bytes()).hexdigest(),expected_text=text,max_cer=0.02,max_wer=0.10,require_exact_accented_words=True,review_status='generated, pending coordinator review',font={'name':pathlib.Path(font_path).name,'sha256':hashlib.sha256(pathlib.Path(font_path).read_bytes()).hexdigest(),'size':size,'license':lic,'license_sha256':hashlib.sha256(pathlib.Path(lic_path).read_bytes()).hexdigest()}))
   (a.out/(pathlib.Path(font_path).stem+'-LICENSE.txt')).write_bytes(pathlib.Path(lic_path).read_bytes())
(a.out/'contracts.json').write_text(json.dumps({'schema':'saccade-ocr-accent-contracts.v1','thresholds_declared_before_inference':True,'fixtures':contracts},ensure_ascii=False,indent=2)+'\n')
print(f'Generated {len(contracts)} frozen contracts')
