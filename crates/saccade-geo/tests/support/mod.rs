#![allow(dead_code, missing_docs, clippy::unwrap_used, clippy::expect_used)]
use std::path::Path;
use tiff::{
    encoder::{TiffEncoder, colortype::ColorType},
    tags::{PhotometricInterpretation, SampleFormat, Tag},
};
pub struct Float<const N: usize>;
impl<const N: usize> ColorType for Float<N> {
    type Inner = f32;
    const TIFF_VALUE: PhotometricInterpretation = PhotometricInterpretation::BlackIsZero;
    const BITS_PER_SAMPLE: &'static [u16] = &[32; N];
    const SAMPLE_FORMAT: &'static [SampleFormat] = &[SampleFormat::IEEEFP; N];
    fn horizontal_predict(row: &[f32], out: &mut Vec<f32>) {
        out.extend_from_slice(row);
    }
}
pub struct Word<const N: usize>;
impl<const N: usize> ColorType for Word<N> {
    type Inner = u16;
    const TIFF_VALUE: PhotometricInterpretation = PhotometricInterpretation::BlackIsZero;
    const BITS_PER_SAMPLE: &'static [u16] = &[16; N];
    const SAMPLE_FORMAT: &'static [SampleFormat] = &[SampleFormat::Uint; N];
    fn horizontal_predict(row: &[u16], out: &mut Vec<u16>) {
        out.extend_from_slice(row);
    }
}
pub fn raster<C: ColorType>(
    path: &Path,
    w: u32,
    h: u32,
    values: &[C::Inner],
    grid: Option<(u16, f64)>,
    nodata: Option<&str>,
) where
    [C::Inner]: tiff::encoder::TiffValue,
{
    let mut enc = TiffEncoder::new(std::fs::File::create(path).unwrap()).unwrap();
    let mut image = enc.new_image::<C>(w, h).unwrap();
    if let Some((crs, offset)) = grid {
        image
            .encoder()
            .write_tag(
                Tag::GeoKeyDirectoryTag,
                &[1u16, 1, 0, 3, 1024, 0, 1, 1, 1025, 0, 1, 1, 3072, 0, 1, crs][..],
            )
            .unwrap();
        image
            .encoder()
            .write_tag(Tag::ModelPixelScaleTag, &[2., 2., 0.][..])
            .unwrap();
        image
            .encoder()
            .write_tag(
                Tag::ModelTiepointTag,
                &[0., 0., 0., 100. + offset, 200., 0.][..],
            )
            .unwrap();
    }
    if let Some(n) = nodata {
        image.encoder().write_tag(Tag::GdalNodata, n).unwrap();
    }
    image.write_data(values).unwrap();
}
pub fn fixtures(root: &Path) {
    std::fs::create_dir_all(root).unwrap();
    let mut a = vec![0f32; 16 * 16 * 3];
    for y in 0..16 {
        for x in 0..16 {
            for b in 0..3 {
                a[(y * 16 + x) * 3 + b] = if x == 0 || y == 0 || x == 15 || y == 15 {
                    -9999.
                } else {
                    (x + y + b * 10) as f32
                };
            }
        }
    }
    let mut b = a.clone();
    for y in 7..9 {
        for x in 7..9 {
            b[(y * 16 + x) * 3 + 1] += 2.;
        }
    }
    raster::<Float<3>>(
        &root.join("float-a.tiff"),
        16,
        16,
        &a,
        Some((32631, 0.)),
        Some("-9999"),
    );
    raster::<Float<3>>(
        &root.join("float-b.tiff"),
        16,
        16,
        &b,
        Some((32631, 0.)),
        Some("-9999"),
    );
    raster::<Float<3>>(
        &root.join("other-grid.tiff"),
        16,
        16,
        &b,
        Some((32631, 1.)),
        Some("-9999"),
    );
    raster::<Float<3>>(
        &root.join("other-crs.tiff"),
        16,
        16,
        &b,
        Some((32632, 0.)),
        Some("-9999"),
    );
    let a: Vec<u16> = (0..16 * 16 * 4)
        .map(|i| {
            let band = (i % 4) as f64;
            let x = ((i / 4) % 16) as f64;
            let y = (i / 64) as f64;
            let dx = x - (4. + band * 2.);
            let dy = y - (5. + band);
            (1000. + (45000. - band * 5000.) * (-(dx * dx + dy * dy) / 8.).exp()).round() as u16
        })
        .collect();
    let mut b = a.clone();
    b[42 * 4 + 3] += 1024;
    raster::<Word<4>>(&root.join("word-a.tiff"), 16, 16, &a, None, None);
    raster::<Word<4>>(&root.join("word-b.tiff"), 16, 16, &b, None, None);
    let mut a = vec![0u16; 256];
    for y in 4..12 {
        for x in 4..12 {
            a[y * 16 + x] = 1000;
        }
    }
    a[0] = 65535;
    let mut b = a.clone();
    b[5 * 16 + 5] = 0;
    b[1] = 65535;
    raster::<Word<1>>(&root.join("class-a.tiff"), 16, 16, &a, None, Some("65535"));
    raster::<Word<1>>(&root.join("class-b.tiff"), 16, 16, &b, None, Some("65535"));
    for side in ["tiles-a", "tiles-b"] {
        std::fs::create_dir_all(root.join(side).join("2/1")).unwrap();
    }
    for y in 0..3 {
        let a = image::RgbaImage::from_pixel(16, 16, image::Rgba([80, 100, 120, 255]));
        a.save(root.join(format!("tiles-a/2/1/{y}.png"))).unwrap();
        if y == 1 {
            continue;
        }
        let mut b = a;
        if y == 2 {
            *b.get_pixel_mut(8, 8) = image::Rgba([255, 0, 0, 255]);
        }
        b.save(root.join(format!("tiles-b/2/1/{y}.png"))).unwrap();
    }
    image::RgbImage::from_pixel(16, 16, image::Rgb([1, 2, 3]))
        .save(root.join("tiles-b/2/1/3.webp"))
        .unwrap();
}

/// Original minimal planar TIFF, with one uncompressed strip per band.
pub fn planar(path: &Path, w: u32, h: u32, planes: &[Vec<u16>]) {
    let n = planes.len();
    assert!(n >= 3);
    let shorts = |v: u16| v.to_le_bytes().to_vec();
    let long = |v: u32| v.to_le_bytes().to_vec();
    let tags = vec![
        (256u16, 4u16, 1u32, long(w)),
        (257, 4, 1, long(h)),
        (258, 3, n as u32, (0..n).flat_map(|_| shorts(16)).collect()),
        (259, 3, 1, shorts(1)),
        (262, 3, 1, shorts(1)),
        (273, 4, n as u32, vec![0; n * 4]),
        (277, 3, 1, shorts(n as u16)),
        (278, 4, 1, long(h)),
        (
            279,
            4,
            n as u32,
            (0..n).flat_map(|_| long(w * h * 2)).collect(),
        ),
        (284, 3, 1, shorts(2)),
        (339, 3, n as u32, (0..n).flat_map(|_| shorts(1)).collect()),
    ];
    let start = 8 + 2 + tags.len() * 12 + 4;
    let mut entries = Vec::new();
    let mut extra = Vec::new();
    let mut offsets = 0;
    for (tag, ty, count, data) in tags {
        entries.extend(tag.to_le_bytes());
        entries.extend(ty.to_le_bytes());
        entries.extend(count.to_le_bytes());
        if data.len() <= 4 {
            entries.extend(&data);
            entries.resize(entries.len() + 4 - data.len(), 0);
        } else {
            let offset = start + extra.len();
            if tag == 273 {
                offsets = extra.len();
            }
            entries.extend((offset as u32).to_le_bytes());
            extra.extend(data);
        }
    }
    let pixels_start = start + extra.len();
    for b in 0..n {
        extra[offsets + b * 4..offsets + b * 4 + 4].copy_from_slice(
            &((pixels_start + b * w as usize * h as usize * 2) as u32).to_le_bytes(),
        );
    }
    let mut out = b"II\x2a\0\x08\0\0\0".to_vec();
    out.extend((11u16).to_le_bytes());
    out.extend(entries);
    out.extend(0u32.to_le_bytes());
    out.extend(extra);
    for plane in planes {
        assert_eq!(plane.len(), w as usize * h as usize);
        for v in plane {
            out.extend(v.to_le_bytes());
        }
    }
    std::fs::write(path, out).unwrap();
}
