#!/usr/bin/env python3
"""Generated fixture artwork, MIT; no downloaded photographs or model-generated pixels."""
from pathlib import Path
from PIL import Image, ImageDraw, ImageFilter

def face():
    im=Image.new('RGB',(256,256),(218,229,238));d=ImageDraw.Draw(im)
    d.ellipse((28,211,228,330),fill=(44,78,118));d.rectangle((107,193,149,235),fill=(204,150,113))
    for box in [(57,92,79,142),(177,92,199,142)]:d.ellipse(box,fill=(205,147,110))
    d.ellipse((63,26,192,212),fill=(228,178,139));d.pieslice((60,16,194,161),180,350,fill=(57,35,22))
    for box in [(76,80,115,89),(139,80,178,89)]:d.ellipse(box,fill=(74,43,25))
    for box in [(78,96,115,110),(139,96,176,110)]:d.ellipse(box,fill=(247,236,212))
    for box in [(91,95,104,111),(150,95,163,111)]:d.ellipse(box,fill=(46,57,33))
    for box in [(95,97,101,109),(154,97,160,109)]:d.ellipse(box,fill=(13,12,10))
    d.polygon([(122,111),(111,146),(136,146),(128,115)],fill=(201,139,102));d.ellipse((108,140,140,151),fill=(207,145,103))
    d.ellipse((91,163,160,183),fill=(160,72,64));d.ellipse((97,162,154,173),fill=(241,218,195))
    d.arc((78,103,176,210),15,165,fill=(194,126,94),width=2)
    return im.filter(ImageFilter.GaussianBlur(.65))
def bottle():
    im=Image.new('RGB',(256,256),'white');d=ImageDraw.Draw(im)
    d.rounded_rectangle((89,64,167,229),radius=20,fill=(72,140,213),outline=(24,72,126),width=3)
    d.rectangle((108,29,148,78),fill=(74,140,211),outline=(24,72,126),width=3)
    d.rounded_rectangle((105,23,151,41),radius=4,fill=(20,45,79))
    d.rectangle((92,128,164,179),fill=(238,237,214))
    d.ellipse((115,138,141,164),fill=(40,111,66))
    d.line((101,83,101,116),fill=(146,194,234),width=5)
    return im
if __name__=='__main__':
    import argparse
    p=argparse.ArgumentParser();p.add_argument('directory',type=Path);a=p.parse_args();a.directory.mkdir(parents=True,exist_ok=True)
    face().save(a.directory/'face.png');bottle().save(a.directory/'bottle.png')
    Image.new('RGB',(257,193),'white').save(a.directory/'blank.png')
