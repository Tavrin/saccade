#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]
use std::path::Path;
use tiff::{
    encoder::{TiffEncoder, colortype},
    tags::Tag,
};
fn u16be(out: &mut Vec<u8>, v: u16) {
    out.extend(v.to_be_bytes());
}
fn u32be(out: &mut Vec<u8>, v: u32) {
    out.extend(v.to_be_bytes());
}
fn fixed(out: &mut Vec<u8>, v: f64) {
    out.extend(((v * 65536.).round() as i32).to_be_bytes());
}
fn lut(input: u8, output: u8, f: impl Fn(&[f64]) -> Vec<f64>) -> Vec<u8> {
    let mut out = b"mft2\0\0\0\0".to_vec();
    out.extend([input, output, 3, 0]);
    for i in 0..9 {
        fixed(&mut out, if i % 4 == 0 { 1. } else { 0. });
    }
    u16be(&mut out, 2);
    u16be(&mut out, 2);
    for _ in 0..input {
        u16be(&mut out, 0);
        u16be(&mut out, 65535);
    }
    for i in 0..3usize.pow(input.into()) {
        let mut v = vec![0.; input as usize];
        let mut ix = i;
        for j in (0..input as usize).rev() {
            v[j] = (ix % 3) as f64 / 2.;
            ix /= 3;
        }
        for x in f(&v) {
            u16be(&mut out, (x.clamp(0., 1.) * 65535.).round() as u16);
        }
    }
    for _ in 0..output {
        u16be(&mut out, 0);
        u16be(&mut out, 65535);
    }
    out
}
// Original synthetic ICC v2 LUT profile, MIT OR Apache-2.0. It is an analytic
// test device, not a characterization of a commercial press or paper.
pub fn icc() -> Vec<u8> {
    icc_gamut(80.)
}
pub fn icc_gamut(span: f64) -> Vec<u8> {
    let forward = lut(4, 3, |v| {
        vec![
            (1. - v[3]) * 65280. / 65535.,
            (128. + span * (v[1] - v[0])) * 256. / 65535.,
            (128. + span * (v[2] - v[0])) * 256. / 65535.,
        ]
    });
    let reverse = lut(3, 4, |v| {
        let a = (v[1] * 65535. / 256. - 128.) / span;
        let b = (v[2] * 65535. / 256. - 128.) / span;
        let c = 0_f64.max(-a).max(-b);
        vec![c, c + a, c + b, 1. - v[0] * 65535. / 65280.]
    });
    let mut white = b"XYZ \0\0\0\0".to_vec();
    for v in [0.9642, 1., 0.8249] {
        fixed(&mut white, v);
    }
    let tags = [
        (*b"A2B0", forward.clone()),
        (*b"A2B1", forward),
        (*b"B2A0", reverse.clone()),
        (*b"B2A1", reverse),
        (*b"wtpt", white),
    ];
    let mut out = vec![0; 128];
    out[8..12].copy_from_slice(&0x02100000u32.to_be_bytes());
    out[12..16].copy_from_slice(b"prtr");
    out[16..20].copy_from_slice(b"CMYK");
    out[20..24].copy_from_slice(b"Lab ");
    out[36..40].copy_from_slice(b"acsp");
    let mut d50 = Vec::new();
    for v in [0.9642, 1., 0.8249] {
        fixed(&mut d50, v);
    }
    out[68..80].copy_from_slice(&d50);
    u32be(&mut out, tags.len() as u32);
    let mut pos = 128 + 4 + tags.len() * 12;
    for (name, bytes) in &tags {
        out.extend(name);
        u32be(&mut out, pos as u32);
        u32be(&mut out, bytes.len() as u32);
        pos += (bytes.len() + 3) & !3;
    }
    for (_, bytes) in tags {
        out.extend(bytes);
        while !out.len().is_multiple_of(4) {
            out.push(0);
        }
    }
    let len = out.len() as u32;
    out[..4].copy_from_slice(&len.to_be_bytes());
    out
}
pub fn tiff(path: &Path, w: u32, h: u32, pixels: &[u8], profile: Option<&[u8]>) {
    let mut file = std::fs::File::create(path).unwrap();
    let mut enc = TiffEncoder::new(&mut file).unwrap();
    let mut img = enc.new_image::<colortype::CMYK8>(w, h).unwrap();
    if let Some(icc) = profile {
        img.encoder().write_tag(Tag::IccProfile, icc).unwrap();
    }
    img.write_data(pixels).unwrap();
}
pub fn fixture(dir: &Path, name: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    let (w, h) = if name == "packaging-label" {
        (96, 48)
    } else {
        (80, 112)
    };
    let mut pixels = vec![0; w * h * 4];
    for y in 0..h {
        for x in 0..w {
            let p = &mut pixels[(y * w + x) * 4..][..4];
            if x > 6 && x < w - 6 && y > 6 && y < 18 {
                p.copy_from_slice(&[180, 30, 80, 15]);
            }
            if (22..30).contains(&y) && x % 8 < 3 && x > 6 && x < w - 6 {
                p.copy_from_slice(&[30, 30, 30, 200]);
            }
            if (35..40).contains(&y) && (8..18).contains(&x) {
                p.copy_from_slice(&[230, 230, 230, 230]);
            }
            // The page has an illustration panel and two columns of body lines;
            // the compact label has only its title, small marks and ink patch.
            if name == "magazine-page" {
                if (46..76).contains(&y) && (8..72).contains(&x) {
                    p.copy_from_slice(&[40, 100, 20, 10]);
                }
                if (82..104).contains(&y)
                    && y % 6 < 2
                    && ((8..36).contains(&x) || (44..72).contains(&x))
                {
                    p.copy_from_slice(&[0, 0, 0, 180]);
                }
            }
        }
    }
    let profile = icc();
    let a = dir.join(name);
    let a = a.with_extension("tif");
    let b = dir.join(format!("{name}-shift.tif"));
    tiff(&a, w as u32, h as u32, &pixels, Some(&profile));
    for p in pixels.as_chunks_mut::<4>().0.iter_mut() {
        if p[0] == 180 {
            p[0] = 200;
        }
    }
    tiff(&b, w as u32, h as u32, &pixels, Some(&profile));
    (a, b)
}
