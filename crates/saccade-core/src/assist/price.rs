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
/// Historical and Gemini-direct image ceiling table revision.
pub const IMAGE_TABLE: &str = "assist-image-ceilings/1";
/// Calibrated OpenRouter/Gemini table; never a documented tokenizer guarantee.
pub const OPENROUTER_IMAGE_TABLE: &str = "assist-image-ceilings/2";
/// Twice the largest prompt count in the supplied ten-call receipt (including text).
pub const CALIBRATED_IMAGE_TOKENS: u64 = 1543 * 2;
/// Area block covering every constructed corpus image (maximum 1038 by 320).
pub const CALIBRATED_IMAGE_PIXELS: u64 = 1024 * 512;
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
/// Read either table revision without reinterpreting historical reservations.
/// Version 2 scales by rounded-up area blocks, with no undocumented low-detail discount.
/// Unknown resolution, zero dimensions or edges above 2048 retain fail-closed admission.
pub fn image_tokens_for_table(table: &str, resolution: &str, dimensions: [u32; 2]) -> Result<u64> {
    match table {
        IMAGE_TABLE => Ok(image_tokens(resolution, dimensions)),
        OPENROUTER_IMAGE_TABLE => {
            if ![
                "MEDIA_RESOLUTION_LOW",
                "MEDIA_RESOLUTION_MEDIUM",
                "MEDIA_RESOLUTION_HIGH",
            ]
            .contains(&resolution)
                || dimensions.contains(&0)
                || dimensions.iter().any(|edge| *edge > 2048)
            {
                return Ok(MAX_IMAGE_TOKENS);
            }
            let pixels = u64::from(dimensions[0]) * u64::from(dimensions[1]);
            Ok(pixels.div_ceil(CALIBRATED_IMAGE_PIXELS) * CALIBRATED_IMAGE_TOKENS)
        }
        // No model-specific image rate or tokenizer evidence was supplied. Reserve
        // the full historical fallback per image, independent of dimensions/detail.
        // This provisional calibrated policy requires a post-call breach stop.
        OPENROUTER_GPT_IMAGE_TABLE => Ok(MAX_IMAGE_TOKENS),
        _ => Err(super::Error::Invalid("unknown image ceiling table")),
    }
}
// Decode only the PNG header; no image-sized allocation or pixel decoding.
pub(crate) fn png_dimensions(data: &str) -> Result<[u32; 2]> {
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
                    "responseJsonSchema",
                    "mediaResolution",
                    "thinkingConfig",
                ]
                .contains(&k.as_str())
            })
        }),
        "unsupported Gemini generation settings",
    )?;
    if let Some(schema) = settings.get("responseJsonSchema") {
        require(
            *schema == super::structured_output::answer_schema()?,
            "Gemini answer schema drift",
        )?;
    }
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

/// Live admission allowlist, distinct from historical recorded response prices.
pub const OPENROUTER_PRICE_ID: &str = "openrouter-price-allowlist/2026-10-07-v2";
/// Recorded source supplied by the admission brief; no runtime price discovery.
pub const OPENROUTER_PRICE_SOURCE: &str = "https://openrouter.ai/api/v1/models";
/// Date of the supplied models API price record.
pub const OPENROUTER_PRICE_DATE: &str = "2026-10-07";
/// Gemini arm with supplied price evidence for live admission.
pub const OPENROUTER_MODEL: &str = "google/gemini-3.8-flash";
/// Independently priced second arm; no implicit fallback or revision substitution.
pub const OPENROUTER_GPT_MODEL: &str = "openai/gpt-5.4-mini";
/// Provisional calibrated policy bound, not measured tokenizer qualification.
pub const OPENROUTER_GPT_IMAGE_TABLE: &str = "assist-openai-image-ceilings/1";
/// Bind each model to its own image policy; Gemini calibration never transfers.
pub fn openrouter_image_table(model: &str) -> &'static str {
    if model == OPENROUTER_GPT_MODEL {
        OPENROUTER_GPT_IMAGE_TABLE
    } else {
        OPENROUTER_IMAGE_TABLE
    }
}
/// Exact pinned nanodollars per token, including image input tokens.
#[derive(Debug, Clone, Copy)]
pub struct OpenRouterPrice {
    /// Text prompt price.
    pub input: u64,
    /// Completion price, including billed reasoning.
    pub output: u64,
    /// Image prompt price.
    pub image: u64,
}
impl OpenRouterPrice {
    /// Provider route ceilings in USD per million tokens (OpenRouter max_price units).
    pub fn max_price(self) -> Value {
        serde_json::json!({"prompt":self.input.max(self.image) as f64 / 1000.0,
                          "completion":self.output as f64 / 1000.0})
    }
}
/// Fail closed for every model without a supplied, versioned price pin.
pub fn openrouter_price(model: &str) -> Result<OpenRouterPrice> {
    match model {
        OPENROUTER_MODEL => Ok(OpenRouterPrice {
            input: 750,
            output: 3750,
            image: 750,
        }),
        OPENROUTER_GPT_MODEL => Ok(OpenRouterPrice {
            input: 750,
            output: 4500,
            image: 750,
        }),
        _ => Err(super::Error::Invalid("openrouter_model_not_allowlisted")),
    }
}
/// Payload-derived token ceilings and exact pre-dispatch monetary reservation.
#[derive(Debug, Clone, Copy)]
pub struct OpenRouterAdmission {
    /// Aggregate prompt and explicit completion ceilings.
    pub bounds: Bounds,
    /// Reasoning subset of aggregate completion; reserved once inside bounds.output.
    pub reasoning: u64,
    /// Integer nanodollars; supplied rates are integral, so no rounding is lost.
    pub reservation: u64,
}
/// Bound closed text/PNG messages without treating base64 bytes as text tokens.
/// Unsupported content is refused, including remote images and tool messages.
pub(crate) fn openrouter_bounds(
    payload: &[u8],
    price: OpenRouterPrice,
) -> Result<OpenRouterAdmission> {
    let request: Value = decode(payload)?;
    openrouter_bounds_for_table(
        payload,
        price,
        openrouter_image_table(request["model"].as_str().unwrap_or("")),
    )
}
// Historical verification only; live admission always selects version 2 above.
pub(crate) fn openrouter_bounds_for_table(
    payload: &[u8],
    price: OpenRouterPrice,
    table: &str,
) -> Result<OpenRouterAdmission> {
    let mut request: Value = decode(payload)?;
    let output = request["max_tokens"]
        .as_u64()
        .filter(|n| *n > 0 && *n <= OUTPUT_LIMIT)
        .ok_or(super::Error::Invalid("explicit output ceiling required"))?;
    let messages = request["messages"]
        .as_array_mut()
        .ok_or(super::Error::Invalid("OpenRouter messages"))?;
    let mut images = 0u64;
    let mut image_count = 0;
    for message in messages {
        require(
            message.as_object().is_some_and(|o| o.len() == 2)
                && message["role"]
                    .as_str()
                    .is_some_and(|r| ["system", "user", "assistant"].contains(&r)),
            "unsupported OpenRouter message",
        )?;
        if message["content"].is_string() {
            continue;
        }
        let parts = message["content"]
            .as_array_mut()
            .ok_or(super::Error::Invalid("OpenRouter content"))?;
        require(!parts.is_empty(), "empty OpenRouter content")?;
        for part in parts {
            require(
                part.as_object().is_some_and(|o| o.len() == 2),
                "unsupported OpenRouter part",
            )?;
            if part["type"] == "text" {
                require(part["text"].is_string(), "OpenRouter text")?;
            } else {
                require(part["type"] == "image_url", "unsupported OpenRouter media")?;
                let image = &mut part["image_url"];
                require(
                    image
                        .as_object()
                        .is_some_and(|o| o.keys().all(|k| ["url", "detail"].contains(&k.as_str())))
                        && image
                            .get("detail")
                            .is_none_or(|d| ["auto", "high", "low"].iter().any(|v| d == v)),
                    "unsupported OpenRouter image settings",
                )?;
                let data = image["url"]
                    .as_str()
                    .and_then(|s| s.strip_prefix("data:image/png;base64,"))
                    .ok_or(super::Error::Invalid("OpenRouter inline PNG required"))?;
                // Auto/absent detail gets the high-resolution bound. Version 2 gives
                // low no discount: there is no documented provider rule in the repo.
                let resolution = if table != IMAGE_TABLE && image["detail"] == "low" {
                    "MEDIA_RESOLUTION_LOW"
                } else {
                    "MEDIA_RESOLUTION_HIGH"
                };
                images = images
                    .checked_add(image_tokens_for_table(
                        table,
                        resolution,
                        png_dimensions(data)?,
                    )?)
                    .ok_or(super::Error::Invalid("image ceiling overflow"))?;
                image_count += 1;
                image["url"] = Value::Null;
            }
        }
    }
    require(
        image_count
            <= if super::video::is_request(&request) {
                super::video::MAX_FRAMES
            } else {
                2
            },
        "image count ceiling",
    )?;
    let text = serde_json::to_vec(&request)
        .map_err(|_| super::Error::Invalid("request bytes"))?
        .len() as u64
        * TOKENS_PER_TEXT_BYTE
        + FRAMING_TOKENS;
    let input = text
        .checked_add(images)
        .ok_or(super::Error::Invalid("input ceiling overflow"))?;
    require(
        input
            <= if super::video::is_request(&request) {
                super::video::INPUT_LIMIT
            } else {
                INPUT_LIMIT
            },
        "local input ceiling exceeded",
    )?;
    let reservation = text
        .checked_mul(price.input)
        .and_then(|n| {
            images
                .checked_mul(price.image)
                .and_then(|i| n.checked_add(i))
        })
        .and_then(|n| {
            output
                .checked_mul(price.output)
                .and_then(|o| n.checked_add(o))
        })
        .ok_or(super::Error::Invalid("reservation price overflow"))?;
    Ok(OpenRouterAdmission {
        bounds: Bounds { input, output },
        reasoning: request["reasoning"]["max_tokens"].as_u64().unwrap_or(0),
        reservation,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod independent_arm_tests {
    use super::*;
    #[test]
    fn second_arm_prices_and_image_policy_are_independent() {
        let p = openrouter_price(OPENROUTER_GPT_MODEL).unwrap();
        assert_eq!((p.input, p.output, p.image), (750, 4500, 750));
        assert_eq!(
            p.max_price(),
            serde_json::json!({"prompt":0.75,"completion":4.5})
        );
        assert_ne!(
            openrouter_image_table(OPENROUTER_MODEL),
            openrouter_image_table(OPENROUTER_GPT_MODEL)
        );
        for dimensions in [[1, 1], [256, 256], [2048, 2048]] {
            assert_eq!(
                image_tokens_for_table(
                    OPENROUTER_GPT_IMAGE_TABLE,
                    "MEDIA_RESOLUTION_HIGH",
                    dimensions
                )
                .unwrap(),
                16384
            );
        }
        assert!(openrouter_price("unknown/model").is_err());
    }
}
