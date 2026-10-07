//! Conservative raster-only PDF adapter. Refuse vector content and transformations
//! that would make raw image sample coordinates differ from page coordinates.
use crate::{
    Error, Result,
    input::{self, Raster},
};
use lopdf::{Document, Object, Stream};
fn err(e: impl std::fmt::Display) -> Error {
    Error::Decode(e.to_string())
}
fn unsupported() -> Error {
    Error::Unsupported("PDF requires one page containing only one full-page, unrotated CMYK image; export complex pages as colour-managed CMYK TIFF".into())
}
fn resolve<'a>(doc: &'a Document, o: &'a Object) -> Result<&'a Object> {
    match o {
        Object::Reference(id) => doc.get_object(*id).map_err(err),
        _ => Ok(o),
    }
}
fn decoded(s: &Stream, limit: usize) -> Result<Vec<u8>> {
    if s.dict.has(b"DecodeParms") {
        return Err(unsupported());
    }
    if let Ok(filter) = s.dict.get(b"Filter") {
        if filter.as_name().map_err(err)? != b"FlateDecode" {
            return Err(unsupported());
        }
        s.decompressed_content_with_limit(limit).map_err(err)
    } else if s.content.len() > limit {
        Err(Error::Limit)
    } else {
        Ok(s.content.clone())
    }
}
pub(crate) fn load(data: &[u8], digest: String) -> Result<Raster> {
    let doc = Document::load_mem_with_options(
        data,
        lopdf::LoadOptions {
            strict: true,
            max_decompressed_size: Some(4 * 1024 * 1024),
            ..Default::default()
        },
    )
    .map_err(err)?;
    if doc.is_encrypted() {
        return Err(unsupported());
    }
    let pages = doc.get_pages();
    if pages.len() != 1 {
        return Err(unsupported());
    }
    let id = *pages.values().next().ok_or_else(unsupported)?;
    let page = doc.get_dictionary(id).map_err(err)?;
    if page.has(b"Annots") || page.has(b"Rotate") || page.has(b"CropBox") || page.has(b"UserUnit") {
        return Err(unsupported());
    }
    let contents = resolve(&doc, page.get(b"Contents").map_err(err)?)?
        .as_stream()
        .map_err(err)?;
    let content = lopdf::content::Content::decode(&decoded(contents, 65536)?).map_err(err)?;
    let ops = &content.operations;
    if ops.len() != 4
        || ops[0].operator != "q"
        || ops[1].operator != "cm"
        || ops[2].operator != "Do"
        || ops[3].operator != "Q"
    {
        return Err(unsupported());
    }
    let numbers = |o: &Object| o.as_float().map(|v| v as f64).map_err(err);
    let media = page
        .get(b"MediaBox")
        .map_err(err)?
        .as_array()
        .map_err(err)?;
    let m = media.iter().map(numbers).collect::<Result<Vec<_>>>()?;
    let matrix = ops[1]
        .operands
        .iter()
        .map(numbers)
        .collect::<Result<Vec<_>>>()?;
    if m.len() != 4
        || m[0] != 0.
        || m[1] != 0.
        || m[2] <= 0.
        || m[3] <= 0.
        || matrix != [m[2], 0., 0., m[3], 0., 0.]
    {
        return Err(unsupported());
    }
    let name = ops[2]
        .operands
        .first()
        .ok_or_else(unsupported)?
        .as_name()
        .map_err(err)?;
    let resources = resolve(&doc, page.get(b"Resources").map_err(err)?)?
        .as_dict()
        .map_err(err)?;
    let objects = resolve(&doc, resources.get(b"XObject").map_err(err)?)?
        .as_dict()
        .map_err(err)?;
    let image = resolve(&doc, objects.get(name).map_err(err)?)?
        .as_stream()
        .map_err(err)?;
    if image
        .dict
        .get(b"Subtype")
        .map_err(err)?
        .as_name()
        .map_err(err)?
        != b"Image"
        || image.dict.has(b"Decode")
        || image.dict.has(b"Mask")
        || image.dict.has(b"ImageMask")
    {
        return Err(unsupported());
    }
    if image
        .dict
        .get(b"BitsPerComponent")
        .map_err(err)?
        .as_i64()
        .map_err(err)?
        != 8
    {
        return Err(unsupported());
    }
    let width = u32::try_from(
        image
            .dict
            .get(b"Width")
            .map_err(err)?
            .as_i64()
            .map_err(err)?,
    )
    .map_err(err)?;
    let height = u32::try_from(
        image
            .dict
            .get(b"Height")
            .map_err(err)?
            .as_i64()
            .map_err(err)?,
    )
    .map_err(err)?;
    let n = input::dimensions(width, height)?;
    let space = resolve(&doc, image.dict.get(b"ColorSpace").map_err(err)?)?;
    let embedded = if let Ok(name) = space.as_name() {
        if name != b"DeviceCMYK" {
            return Err(Error::NotPrintInput("PDF raster is not CMYK".into()));
        }
        None
    } else {
        let cs = space.as_array().map_err(err)?;
        if cs.len() != 2 || cs[0].as_name().map_err(err)? != b"ICCBased" {
            return Err(unsupported());
        }
        let icc = resolve(&doc, &cs[1])?.as_stream().map_err(err)?;
        if icc.dict.get(b"N").map_err(err)?.as_i64().map_err(err)? != 4 {
            return Err(Error::NotPrintInput("PDF ICC raster is not CMYK".into()));
        }
        if icc.content.len() > 4 * 1024 * 1024 {
            return Err(Error::Limit);
        }
        Some(decoded(icc, 4 * 1024 * 1024)?)
    };
    let samples = decoded(image, n * 4)?;
    if samples.len() != n * 4 {
        return Err(Error::Decode("PDF sample count mismatch".into()));
    }
    let alpha = if let Ok(mask) = image.dict.get(b"SMask") {
        let mask = resolve(&doc, mask)?.as_stream().map_err(err)?;
        for key in [b"Width".as_slice(), b"Height", b"BitsPerComponent"] {
            if mask.dict.get(key).map_err(err)? != image.dict.get(key).map_err(err)? {
                return Err(unsupported());
            }
        }
        if mask
            .dict
            .get(b"ColorSpace")
            .map_err(err)?
            .as_name()
            .map_err(err)?
            != b"DeviceGray"
            || mask.dict.has(b"Decode")
            || mask.dict.has(b"Matte")
        {
            return Err(unsupported());
        }
        let bytes = decoded(mask, n)?;
        if bytes.len() != n {
            return Err(unsupported());
        }
        bytes.iter().map(|v| *v as f64 / 255.).collect()
    } else {
        vec![1.; n]
    };
    Ok(Raster {
        width,
        height,
        ink: samples
            .as_chunks::<4>()
            .0
            .iter()
            .map(|p| [p[0], p[1], p[2], p[3]].map(|v| v as f64 * 100. / 255.))
            .collect(),
        alpha,
        embedded,
        digest,
    })
}
