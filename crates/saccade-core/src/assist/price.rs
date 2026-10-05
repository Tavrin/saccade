//! Versioned conservative admission policy; these ceilings are local policy,
//! not a claim about a provider tokenizer or the price of countTokens.
use super::{
    Result, decode,
    execution::{INPUT_LIMIT, OUTPUT_LIMIT},
    require,
    schema::Usage,
};
use serde_json::Value;

/// Changed reservation semantics require a fresh qualification epoch.
pub const PRICE_ID: &str = "assist-prices/2026-10-05-local-v2";
/// Pinned image ceiling table revision.
pub const IMAGE_TABLE: &str = "assist-image-ceilings/1";
/// Upper bound for an image outside the pinned dimension/resolution table.
pub const MAX_IMAGE_TOKENS: u64 = 16_384;
/// At most one token per encoded UTF-8 text byte, plus protocol framing.
pub const TOKENS_PER_TEXT_BYTE: u64 = 1;
/// Fixed protocol framing allowance, in addition to serialized non-image bytes.
pub const FRAMING_TOKENS: u64 = 1024;

/// Auxiliary counting is disabled unless a versioned policy states its price.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Counting {
    /// No counting HTTP; use the local upper bound.
    Off,
    /// Explicit human policy establishes free counting.
    Free,
    /// Confirmed counting price in nanodollars per counted input token.
    Priced(u64),
}
/// Price binding for one execution epoch. New facts need a distinct policy ID.
#[derive(Clone, Copy, Debug)]
pub struct Policy {
    /// Versioned binding, included in receipts.
    pub id: &'static str,
    /// Auxiliary countTokens price; default off.
    pub counting: Counting,
}
/// Default policy needs no unresolved external counting-price fact.
pub const DEFAULT: Policy = Policy {
    id: PRICE_ID,
    counting: Counting::Off,
};
impl Policy {
    /// Refuse an unversioned change to optional counting billing.
    pub fn validate(&self) -> Result<()> {
        require(
            !self.id.is_empty()
                && self.id.len() <= 128
                && (self.counting == Counting::Off || self.id != PRICE_ID)
                && !matches!(self.counting, Counting::Priced(0)),
            "counting price policy version",
        )
    }
}
/// Reservation bounds derived from actual request settings and PNG headers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bounds {
    /// Conservative prompt token ceiling.
    pub input: u64,
    /// Candidate and thinking ceilings added once.
    pub output: u64,
}
impl Bounds {
    /// Required counters must be consistent and within the reserved ceilings.
    pub fn contains(&self, usage: &Usage) -> bool {
        match (
            usage.input_tokens,
            usage.candidate_tokens,
            usage.thinking_tokens,
            usage.total_tokens,
        ) {
            (Some(input), Some(candidate), Some(thinking), Some(total)) => {
                input <= self.input
                    && candidate.checked_add(thinking).is_some_and(|output| {
                        output <= self.output && input.checked_add(output) == Some(total)
                    })
                    && usage.cached_input_tokens.is_none_or(|n| n <= input)
            }
            _ => false,
        }
    }
}
/// Conservative pinned table; unknown resolution/dimensions use the maximum.
pub fn image_tokens(resolution: &str, dimensions: [u32; 2]) -> u64 {
    let edge = dimensions[0].max(dimensions[1]);
    match (resolution, edge) {
        ("MEDIA_RESOLUTION_LOW", 1..=512) => 1024,
        ("MEDIA_RESOLUTION_MEDIUM", 1..=1024) => 4096,
        ("MEDIA_RESOLUTION_MEDIUM", 1025..=2048) => 8192,
        ("MEDIA_RESOLUTION_HIGH", 1..=2048) => 8192,
        _ => MAX_IMAGE_TOKENS,
    }
}
// Decode only the PNG header; no image-sized allocation or pixel decoding.
fn png_dimensions(data: &str) -> Result<[u32; 2]> {
    require(
        data.len() >= 32 && data.len() <= 24 * 1024 * 1024,
        "encoded image size",
    )?;
    let mut header = Vec::with_capacity(24);
    let mut accumulator = 0u32;
    let mut bits = 0;
    for byte in data.bytes().take(32) {
        let digit = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return Err(super::Error::Invalid("PNG base64 header")),
        };
        accumulator = (accumulator << 6) | u32::from(digit);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            header.push((accumulator >> bits) as u8);
        }
    }
    require(
        header.get(..8) == Some(b"\x89PNG\r\n\x1a\n") && header.get(12..16) == Some(b"IHDR"),
        "PNG header",
    )?;
    let number = |start: usize| -> Result<u32> {
        let bytes: [u8; 4] = header[start..start + 4]
            .try_into()
            .map_err(|_| super::Error::Invalid("PNG dimensions"))?;
        Ok(u32::from_be_bytes(bytes))
    };
    let dimensions = [number(16)?, number(20)?];
    require(
        dimensions[0] > 0
            && dimensions[1] > 0
            && u64::from(dimensions[0]) * u64::from(dimensions[1]) <= 4_194_304,
        "image dimension ceiling",
    )?;
    Ok(dimensions)
}
/// Local Gemini upper bound: serialized non-image bytes plus pinned image ceilings.
/// Unsupported media/functions/cached inputs are refused rather than undercounted.
pub fn gemini_bounds(payload: &[u8]) -> Result<Bounds> {
    let mut request: Value = decode(payload)?;
    let object = request
        .as_object()
        .ok_or(super::Error::Invalid("Gemini request object"))?;
    require(
        object
            .keys()
            .all(|k| ["systemInstruction", "contents", "generationConfig"].contains(&k.as_str())),
        "unsupported Gemini request fields",
    )?;
    let settings = &request["generationConfig"];
    require(
        settings.as_object().is_some_and(|o| {
            o.keys().all(|k| {
                [
                    "temperature",
                    "maxOutputTokens",
                    "responseMimeType",
                    "mediaResolution",
                    "thinkingConfig",
                ]
                .contains(&k.as_str())
            })
        }),
        "unsupported Gemini generation settings",
    )?;
    require(
        settings["thinkingConfig"]
            .as_object()
            .is_some_and(|o| o.len() == 1 && o.contains_key("thinkingBudget")),
        "unsupported thinking configuration",
    )?;
    let candidates = settings["maxOutputTokens"]
        .as_u64()
        .ok_or(super::Error::Invalid("explicit output ceiling required"))?;
    let thinking = settings["thinkingConfig"]["thinkingBudget"]
        .as_u64()
        .ok_or(super::Error::Invalid("explicit thinking ceiling required"))?;
    let output = candidates
        .checked_add(thinking)
        .ok_or(super::Error::Invalid("output ceiling overflow"))?;
    require(
        candidates > 0 && output <= OUTPUT_LIMIT,
        "billed output ceiling",
    )?;
    let resolution = settings["mediaResolution"]
        .as_str()
        .ok_or(super::Error::Invalid("explicit media resolution required"))?
        .to_owned();
    let mut image_total = 0;
    let mut image_count = 0;
    for field in ["systemInstruction", "contents"] {
        let contents: Vec<&mut Value> = if field == "contents" {
            request[field]
                .as_array_mut()
                .ok_or(super::Error::Invalid("Gemini contents"))?
                .iter_mut()
                .collect()
        } else {
            vec![&mut request[field]]
        };
        for content in contents {
            let parts = content["parts"]
                .as_array_mut()
                .ok_or(super::Error::Invalid("Gemini parts"))?;
            for part in parts {
                let object = part
                    .as_object_mut()
                    .ok_or(super::Error::Invalid("Gemini part"))?;
                require(object.len() == 1, "unsupported mixed Gemini part")?;
                if let Some(text) = object.get("text") {
                    require(text.is_string(), "Gemini text")?;
                } else {
                    let name = if object.contains_key("inline_data") {
                        "inline_data"
                    } else {
                        "inlineData"
                    };
                    let media = object
                        .get_mut(name)
                        .ok_or(super::Error::Invalid("unsupported Gemini media"))?;
                    let mime = media.get("mime_type").or_else(|| media.get("mimeType"));
                    require(
                        mime.and_then(Value::as_str) == Some("image/png")
                            && media.as_object().is_some_and(|o| o.len() == 2),
                        "unsupported image encoding",
                    )?;
                    let dimensions = png_dimensions(
                        media["data"]
                            .as_str()
                            .ok_or(super::Error::Invalid("missing inline image"))?,
                    )?;
                    image_total += image_tokens(&resolution, dimensions);
                    image_count += 1;
                    media["data"] = Value::Null;
                }
            }
        }
    }
    require(image_count <= 2, "image count ceiling")?;
    let text_bytes = serde_json::to_vec(&request)
        .map_err(|_| super::Error::Invalid("request bytes"))?
        .len() as u64;
    let input = text_bytes * TOKENS_PER_TEXT_BYTE + FRAMING_TOKENS + image_total;
    require(input <= INPUT_LIMIT, "local input ceiling exceeded")?;
    Ok(Bounds { input, output })
}
