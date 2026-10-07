//! Conservative PDF intake profile: bounds before upstream parsing/decompression.
use super::worker::{CAPS, error};
use std::collections::BTreeMap;
use std::io::Read;

fn value<'a>(tokens: &'a [String], key: &str) -> Option<&'a str> {
    let mut depth = 0usize;
    for (i, token) in tokens.iter().enumerate() {
        match token.as_str() {
            "<<" | "[" => depth += 1,
            ">>" | "]" => depth = depth.saturating_sub(1),
            _ => {}
        }
        if depth == 1 && token == key {
            return tokens.get(i + 1).map(String::as_str);
        }
    }
    None
}
fn integer(tokens: &[String], key: &str) -> crate::Result<Option<u64>> {
    let Some(v) = value(tokens, key) else {
        return Ok(None);
    };
    // Critical sizes must be direct values. Reject references rather than guess their bound.
    let pos = tokens
        .iter()
        .position(|s| std::ptr::eq(s.as_str(), v))
        .ok_or_else(|| error("document_malformed"))?;
    if tokens.get(pos + 2).is_some_and(|s| s == "R") {
        return Err(error("document_unsupported"));
    }
    v.parse().map(Some).map_err(|_| error("document_malformed"))
}
fn dictionary(tokens: &[String]) -> crate::Result<()> {
    let mut depth = 0usize;
    let mut keys = std::collections::BTreeSet::new();
    let mut key_expected = true;
    for token in tokens {
        match token.as_str() {
            "<<" | "[" => {
                if depth == 1 {
                    key_expected = true;
                }
                depth += 1;
            }
            ">>" | "]" => {
                depth = depth.saturating_sub(1);
            }
            _ if depth == 1 && token.starts_with('/') => {
                if key_expected && !keys.insert(token) {
                    return Err(error("document_malformed"));
                }
                key_expected = !key_expected;
            }
            _ if depth == 1 => {
                key_expected = true;
            }
            _ => {}
        }
    }
    if matches!(value(tokens, "/Type"), Some("/ObjStm" | "/XRef")) {
        return Err(error("document_unsupported"));
    }
    let w = integer(tokens, "/Width")?;
    let h = integer(tokens, "/Height")?;
    if w.is_some_and(|w| w > u64::from(CAPS.dimension))
        || h.is_some_and(|h| h > u64::from(CAPS.dimension))
        || w.zip(h)
            .is_some_and(|(w, h)| w.saturating_mul(h) > CAPS.pixels)
    {
        return Err(error("document_dimensions_limit"));
    }
    if integer(tokens, "/Count")?.is_some_and(|n| n > CAPS.pages as u64) {
        return Err(error("document_page_limit"));
    }
    Ok(())
}

// Inline images have an independent filter pipeline inside content streams.
// Refuse their operator before interpretation, including across decoder chunks.
struct InlineGuard {
    token: [u8; 2],
    length: usize,
}
impl InlineGuard {
    fn inspect(&mut self, bytes: &[u8]) -> crate::Result<()> {
        for &byte in bytes {
            if byte.is_ascii_whitespace() || b"()<>[]{}/%".contains(&byte) {
                if self.length == 2 && self.token == *b"BI" {
                    return Err(error("document_unsupported"));
                }
                self.length = 0;
            } else {
                if self.length < 2 {
                    self.token[self.length] = byte;
                }
                self.length = self.length.saturating_add(1);
            }
        }
        Ok(())
    }
}

pub(super) fn pdf(data: &[u8]) -> crate::Result<()> {
    let tail = &data[data.len().saturating_sub(4096)..];
    let marker = tail
        .windows(9)
        .rposition(|w| w == b"startxref")
        .ok_or_else(|| error("document_malformed"))?;
    let offset = std::str::from_utf8(&tail[marker + 9..])
        .map_err(|_| error("document_malformed"))?
        .split_whitespace()
        .next()
        .and_then(|s| s.parse::<usize>().ok())
        .ok_or_else(|| error("document_malformed"))?;
    if data.get(offset..offset.saturating_add(4)) != Some(b"xref") {
        return Err(error("document_malformed"));
    }
    let mut tokens = Vec::<String>::new();
    let mut stack = Vec::<(bool, usize)>::new();
    let mut last_dict = None;
    let mut graph = BTreeMap::<u64, Vec<u64>>::new();
    let mut parents = BTreeMap::<u64, Vec<u64>>::new();
    let mut object = None;
    let mut objects = 0usize;
    let mut decoded = 0u64;
    let mut i = 0;
    while i < data.len() {
        if data[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if data[i] == b'%' {
            while i < data.len() && !matches!(data[i], b'\r' | b'\n') {
                i += 1;
            }
            continue;
        }
        let token = match data[i] {
            b'(' => {
                i += 1;
                let mut depth = 1usize;
                while i < data.len() && depth > 0 {
                    match data[i] {
                        b'\\' => {
                            i += 1;
                        }
                        b'(' => depth += 1,
                        b')' => depth -= 1,
                        _ => {}
                    }
                    if depth > CAPS.object_depth {
                        return Err(error("document_depth_limit"));
                    }
                    i += 1;
                }
                if depth != 0 {
                    return Err(error("document_malformed"));
                }
                "string".into()
            }
            b'<' if data.get(i + 1) != Some(&b'<') => {
                i += 1;
                while i < data.len() && data[i] != b'>' {
                    i += 1;
                }
                if i == data.len() {
                    return Err(error("document_malformed"));
                }
                i += 1;
                "string".into()
            }
            b'<' | b'>' => {
                let b = data[i];
                if data.get(i + 1) != Some(&b) {
                    return Err(error("document_malformed"));
                }
                i += 2;
                if b == b'<' { "<<".into() } else { ">>".into() }
            }
            b'[' | b']' => {
                let b = data[i];
                i += 1;
                char::from(b).to_string()
            }
            _ => {
                let start = i;
                i += 1;
                while i < data.len()
                    && !data[i].is_ascii_whitespace()
                    && !b"()<>[]{}/%".contains(&data[i])
                {
                    i += 1;
                }
                if i - start > 1024 {
                    return Err(error("document_unsupported"));
                }
                let raw = &data[start..i];
                let mut normalized = Vec::new();
                let mut j = 0;
                while j < raw.len() {
                    if raw[0] == b'/' && raw[j] == b'#' {
                        let hex = raw
                            .get(j + 1..j + 3)
                            .and_then(|s| std::str::from_utf8(s).ok())
                            .and_then(|s| u8::from_str_radix(s, 16).ok())
                            .ok_or_else(|| error("document_malformed"))?;
                        normalized.push(hex);
                        j += 3;
                    } else {
                        normalized.push(raw[j]);
                        j += 1;
                    }
                }
                String::from_utf8(normalized).map_err(|_| error("document_unsupported"))?
            }
        };
        if tokens.len() >= 1_000_000 {
            return Err(error("document_objects_limit"));
        }
        if token == "stream" {
            let (start, end) = last_dict
                .take()
                .ok_or_else(|| error("document_malformed"))?;
            let dict = &tokens[start..end];
            let len = integer(dict, "/Length")?.ok_or_else(|| error("document_unsupported"))?;
            if data.get(i) == Some(&b'\r') {
                i += 1;
            }
            if data.get(i) != Some(&b'\n') {
                return Err(error("document_malformed"));
            }
            i += 1;
            let length = usize::try_from(len).map_err(|_| error("document_malformed"))?;
            let bytes = data
                .get(
                    i..i.checked_add(length)
                        .ok_or_else(|| error("document_malformed"))?,
                )
                .ok_or_else(|| error("document_malformed"))?;
            if value(dict, "/DecodeParms").is_some()
                || value(dict, "/DP").is_some()
                || value(dict, "/F").is_some()
            {
                return Err(error("document_unsupported"));
            }
            let mut inline = InlineGuard {
                token: [0; 2],
                length: 0,
            };
            match value(dict, "/Filter") {
                None => {
                    decoded = decoded.saturating_add(len);
                    inline.inspect(bytes)?;
                }
                Some("/FlateDecode") => {
                    let mut reader = flate2::read::ZlibDecoder::new(bytes)
                        .take(CAPS.decompressed_bytes.saturating_sub(decoded) + 1);
                    let mut buffer = [0; 8192];
                    loop {
                        let n = reader
                            .read(&mut buffer)
                            .map_err(|_| error("document_malformed"))?;
                        if n == 0 {
                            break;
                        }
                        inline.inspect(&buffer[..n])?;
                        decoded += n as u64;
                        if decoded > CAPS.decompressed_bytes {
                            return Err(error("document_decompressed_limit"));
                        }
                    }
                }
                _ => return Err(error("document_unsupported")),
            }
            inline.inspect(b" ")?;
            if decoded > CAPS.decompressed_bytes {
                return Err(error("document_decompressed_limit"));
            }
            i += length;
            while data.get(i).is_some_and(u8::is_ascii_whitespace) {
                i += 1;
            }
            if data.get(i..i + 9) != Some(b"endstream") {
                return Err(error("document_malformed"));
            }
            i += 9;
            continue;
        }
        let n = tokens.len();
        match token.as_str() {
            "<<" | "[" => {
                stack.push((token == "<<", n));
                if stack.len() > CAPS.object_depth {
                    return Err(error("document_depth_limit"));
                }
            }
            ">>" | "]" => {
                let (dict, start) = stack.pop().ok_or_else(|| error("document_malformed"))?;
                if dict != (token == ">>") {
                    return Err(error("document_malformed"));
                }
                if dict {
                    tokens.push(token);
                    dictionary(&tokens[start..])?;
                    last_dict = Some((start, tokens.len()));
                    continue;
                }
            }
            "obj" => {
                objects += 1;
                if objects > CAPS.objects {
                    return Err(error("document_objects_limit"));
                }
                let id = tokens
                    .get(n.saturating_sub(2))
                    .and_then(|s| s.parse().ok())
                    .ok_or_else(|| error("document_malformed"))?;
                object = Some(id);
                graph.entry(id).or_default();
            }
            "endobj" => {
                object = None;
            }
            "R" if n >= 2 => {
                if let Some(id) = object {
                    let target = tokens[n - 2]
                        .parse()
                        .map_err(|_| error("document_malformed"))?;
                    // Validate structural /Parent chains separately from forward references.
                    if n >= 3 && tokens[n - 3] == "/Parent" {
                        parents.entry(id).or_default().push(target);
                    } else {
                        graph.entry(id).or_default().push(target);
                    }
                }
            }
            _ => {}
        }
        tokens.push(token);
        if tokens.len() > 1_000_000 {
            return Err(error("document_objects_limit"));
        }
    }
    if !stack.is_empty() {
        return Err(error("document_malformed"));
    }
    // Memoized longest path; cycles and deep reference chains fail before the renderer.
    fn visit(
        id: u64,
        graph: &BTreeMap<u64, Vec<u64>>,
        done: &mut BTreeMap<u64, usize>,
        path: &mut Vec<u64>,
    ) -> crate::Result<usize> {
        if let Some(&depth) = done.get(&id) {
            return Ok(depth);
        }
        if path.len() >= CAPS.object_depth || path.contains(&id) {
            return Err(error("document_depth_limit"));
        }
        path.push(id);
        let mut depth = 1;
        if let Some(edges) = graph.get(&id) {
            for &next in edges {
                depth = depth.max(1 + visit(next, graph, done, path)?);
            }
        }
        path.pop();
        if depth > CAPS.object_depth {
            return Err(error("document_depth_limit"));
        }
        done.insert(id, depth);
        Ok(depth)
    }
    let mut done = BTreeMap::new();
    for &id in graph.keys() {
        visit(id, &graph, &mut done, &mut Vec::new())?;
    }
    let mut done = BTreeMap::new();
    for &id in parents.keys() {
        visit(id, &parents, &mut done, &mut Vec::new())?;
    }
    Ok(())
}
