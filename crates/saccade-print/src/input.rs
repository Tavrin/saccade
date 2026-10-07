use crate::{Error, Result};
use std::{
    io::{Cursor, Read},
    path::Path,
};
use tiff::{
    ColorType,
    decoder::{Decoder, DecodingResult},
    tags::Tag,
};

pub(crate) const MAX_PIXELS: usize = 16_000_000;
pub(crate) const MAX_BYTES: u64 = 128 * 1024 * 1024;

pub(crate) struct Raster {
    pub width: u32,
    pub height: u32,
    pub ink: Vec<[f64; 4]>,
    pub alpha: Vec<f64>,
    pub embedded: Option<Vec<u8>>,
    pub digest: String,
}

pub(crate) fn bytes(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(limit + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(Error::Limit);
    }
    Ok(bytes)
}

pub(crate) fn dimensions(w: u32, h: u32) -> Result<usize> {
    let n = (w as usize).checked_mul(h as usize).ok_or(Error::Limit)?;
    if n == 0 || n > MAX_PIXELS {
        return Err(Error::Limit);
    }
    Ok(n)
}

pub(crate) fn load(path: &Path) -> Result<Raster> {
    let data = bytes(path, MAX_BYTES)?;
    let digest = saccade_core::localized::digest(&data);
    if data.starts_with(b"%PDF-") {
        return crate::pdf::load(&data, digest);
    }
    if data.starts_with(&[0xff, 0xd8]) {
        return jpeg(&data, digest);
    }
    if !(data.starts_with(b"II") || data.starts_with(b"MM")) {
        return Err(Error::NotPrintInput(
            "expected CMYK TIFF/JPEG (including prepared PDF rasters)".into(),
        ));
    }
    let mut dec = Decoder::new(Cursor::new(&data)).map_err(decode)?;
    let (width, height) = dec.dimensions().map_err(decode)?;
    let n = dimensions(width, height)?;
    let channels = match dec.colortype().map_err(decode)? {
        ColorType::CMYK(8 | 16) => 4,
        ColorType::CMYKA(8 | 16) => 5,
        ColorType::CMYK(_) | ColorType::CMYKA(_) => {
            return Err(Error::Unsupported(
                "TIFF requires 8 or 16 bit integer samples".into(),
            ));
        }
        _ => return Err(Error::NotPrintInput("TIFF is not CMYK".into())),
    };
    if dec
        .find_tag_unsigned::<u16>(Tag::Orientation)
        .map_err(decode)?
        .unwrap_or(1)
        != 1
    {
        return Err(Error::Unsupported(
            "normalize TIFF orientation before comparison".into(),
        ));
    }
    if dec
        .find_tag_unsigned::<u16>(Tag::Unknown(332))
        .map_err(decode)?
        .unwrap_or(1)
        != 1
    {
        return Err(Error::Unsupported(
            "only process CMYK inks are supported".into(),
        ));
    }
    let associated = if channels == 5 {
        let extras = dec.get_tag_u16_vec(Tag::ExtraSamples).map_err(decode)?;
        match extras.as_slice() {
            [1] => true,
            [2] => false,
            _ => {
                return Err(Error::Unsupported(
                    "CMYKA requires declared associated or straight alpha".into(),
                ));
            }
        }
    } else {
        false
    };
    let embedded = dec
        .find_tag(Tag::IccProfile)
        .map_err(decode)?
        .map(|v| v.into_u8_vec().map_err(decode))
        .transpose()?;
    let samples: Vec<f64> = match dec.read_image().map_err(decode)? {
        DecodingResult::U8(v) => v.into_iter().map(|x| x as f64 / 255.).collect(),
        DecodingResult::U16(v) => v.into_iter().map(|x| x as f64 / 65535.).collect(),
        _ => return Err(Error::Unsupported("unsupported TIFF sample format".into())),
    };
    if dec.more_images() {
        return Err(Error::Unsupported(
            "one TIFF page per comparison is required".into(),
        ));
    }
    if samples.len() != n * channels {
        return Err(Error::Decode("inconsistent sample count".into()));
    }
    let mut ink = Vec::with_capacity(n);
    let mut alpha = Vec::with_capacity(n);
    for p in samples.chunks_exact(channels) {
        let a = if channels == 5 { p[4] } else { 1. };
        let mut c = [p[0], p[1], p[2], p[3]];
        if associated && a > 0. {
            for v in &mut c {
                if *v > a + 1. / 255. {
                    return Err(Error::Decode("associated ink exceeds alpha".into()));
                }
                *v = (*v / a).min(1.);
            }
        }
        ink.push(c.map(|x| x * 100.));
        alpha.push(a);
    }
    Ok(Raster {
        width,
        height,
        ink,
        alpha,
        embedded,
        digest,
    })
}

pub(crate) fn jpeg(data: &[u8], digest: String) -> Result<Raster> {
    let mut dec = jpeg_decoder::Decoder::new(Cursor::new(data));
    dec.read_info().map_err(decode)?;
    let info = dec
        .info()
        .ok_or_else(|| Error::Decode("missing JPEG header".into()))?;
    let n = dimensions(info.width.into(), info.height.into())?;
    if info.pixel_format != jpeg_decoder::PixelFormat::CMYK32 {
        return Err(Error::NotPrintInput("JPEG is not CMYK".into()));
    }
    if !adobe_marker(data) {
        return Err(Error::Unsupported(
            "CMYK JPEG requires an Adobe APP14 marker to establish ink polarity".into(),
        ));
    }
    dec.set_max_decoding_buffer_size(MAX_PIXELS * 4);
    let pixels = dec.decode().map_err(decode)?;
    if pixels.len() != n * 4 {
        return Err(Error::Decode("inconsistent JPEG samples".into()));
    }
    // jpeg-decoder normalizes Adobe CMYK/YCCK to ink amounts (0 = no ink).
    let ink = pixels
        .as_chunks::<4>()
        .0
        .iter()
        .map(|p| [p[0], p[1], p[2], p[3]].map(|x| x as f64 * 100. / 255.))
        .collect();
    Ok(Raster {
        width: info.width.into(),
        height: info.height.into(),
        ink,
        alpha: vec![1.; n],
        embedded: dec.icc_profile(),
        digest,
    })
}

fn decode(e: impl std::fmt::Display) -> Error {
    Error::Decode(e.to_string())
}

fn adobe_marker(data: &[u8]) -> bool {
    let mut pos = 2;
    while pos + 4 <= data.len() && data[pos] == 0xff {
        pos += 1;
        while pos < data.len() && data[pos] == 0xff {
            pos += 1;
        }
        if pos + 3 > data.len() {
            return false;
        }
        let marker = data[pos];
        pos += 1;
        if marker == 0xda || marker == 0xd9 {
            return false;
        }
        let len = u16::from_be_bytes([data[pos], data[pos + 1]]) as usize;
        if len < 2 || pos + len > data.len() {
            return false;
        }
        if marker == 0xee && len >= 14 && &data[pos + 2..pos + 7] == b"Adobe" {
            return true;
        }
        pos += len;
    }
    false
}
