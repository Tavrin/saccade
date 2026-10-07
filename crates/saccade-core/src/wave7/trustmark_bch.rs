//! TrustMark's shortened binary BCH codes over GF(128), primitive polynomial 137.
//! Layout follows Adobe's Python data layer; no schema guessing on ECC failure.

const CAPACITY: [usize; 4] = [40, 61, 68, 75];
const LIMIT: [usize; 4] = [8, 5, 4, 3];
pub(super) const NAMES: [&str; 4] = ["BCH_SUPER", "BCH_5", "BCH_4", "BCH_3"];

struct Field {
    exp: [u8; 127],
    log: [usize; 128],
}
impl Field {
    fn new() -> Self {
        let mut field = Self {
            exp: [0; 127],
            log: [0; 128],
        };
        let mut value = 1u8;
        for i in 0..127 {
            field.exp[i] = value;
            field.log[value as usize] = i;
            let next = u16::from(value) << 1;
            value = (if next & 128 != 0 { next ^ 137 } else { next }) as u8;
        }
        field
    }
    fn mul(&self, a: u8, b: u8) -> u8 {
        if a == 0 || b == 0 {
            0
        } else {
            self.exp[(self.log[a as usize] + self.log[b as usize]) % 127]
        }
    }
    fn div(&self, a: u8, b: u8) -> u8 {
        if a == 0 {
            0
        } else {
            self.exp[(self.log[a as usize] + 127 - self.log[b as usize]) % 127]
        }
    }
    fn syndromes(&self, word: &[u8], t: usize) -> Vec<u8> {
        (1..=2 * t)
            .map(|power| {
                word.iter().enumerate().fold(0, |sum, (i, bit)| {
                    if *bit == 0 {
                        sum
                    } else {
                        sum ^ self.exp[(power * (word.len() - 1 - i)) % 127]
                    }
                })
            })
            .collect()
    }
}

/// Recover only a verified payload; failed or non-transmitted roots are rejected.
pub(super) fn decode(bits: &[u8; 100]) -> Option<(String, u8, u8)> {
    if bits.iter().any(|b| *b > 1) {
        return None;
    }
    let schema = usize::from(bits[98] * 2 + bits[99]);
    let capacity = CAPACITY[schema];
    let t = LIMIT[schema];
    let padded = capacity.div_ceil(8) * 8;
    let mut word = bits[..capacity].to_vec();
    word.resize(padded, 0);
    word.extend_from_slice(&bits[capacity..96]);
    let field = Field::new();
    let syndrome = field.syndromes(&word, t);
    let mut locator = vec![0u8; 2 * t + 1];
    let mut previous = locator.clone();
    locator[0] = 1;
    previous[0] = 1;
    let (mut degree, mut shift, mut last) = (0usize, 1usize, 1u8);
    // Berlekamp–Massey: all divisions have a nonzero previous discrepancy.
    for n in 0..2 * t {
        let discrepancy = (1..=degree).fold(syndrome[n], |d, i| {
            d ^ field.mul(locator[i], syndrome[n - i])
        });
        if discrepancy == 0 {
            shift += 1;
            continue;
        }
        let saved = locator.clone();
        let factor = field.div(discrepancy, last);
        for i in 0..locator.len().saturating_sub(shift) {
            locator[i + shift] ^= field.mul(factor, previous[i]);
        }
        if 2 * degree <= n {
            degree = n + 1 - degree;
            previous = saved;
            last = discrepancy;
            shift = 1;
        } else {
            shift += 1;
        }
    }
    if degree > t {
        return None;
    }
    let mut roots = Vec::new();
    for position in 0..word.len() {
        let x = field.exp[(127 - position % 127) % 127];
        let value = locator[..=degree]
            .iter()
            .rev()
            .fold(0, |v, c| field.mul(v, x) ^ c);
        if value == 0 {
            let index = word.len() - 1 - position;
            if (capacity..padded).contains(&index) {
                return None;
            }
            roots.push(index);
        }
    }
    if roots.len() != degree {
        return None;
    }
    for i in &roots {
        word[*i] ^= 1;
    }
    if field.syndromes(&word, t).iter().any(|s| *s != 0) {
        return None;
    }
    Some((
        word[..capacity]
            .iter()
            .map(|b| char::from(b'0' + b))
            .collect(),
        schema as u8,
        roots.len() as u8,
    ))
}

pub(super) fn limit(schema: u8) -> u8 {
    LIMIT[usize::from(schema)] as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn upstream_reference_vector_corrects_data_and_ecc_errors() {
        let source = "1011011110011000111111000000011111011111011100000110110110111000110010101101111010011011000010000001";
        let mut bits = [0; 100];
        for (b, c) in bits.iter_mut().zip(source.bytes()) {
            *b = c - b'0';
        }
        let expected = &source[..61];
        assert_eq!(decode(&bits), Some((expected.into(), 1, 0)));
        for count in 1..=5 {
            bits[(count - 1) * 19] ^= 1;
            assert_eq!(decode(&bits), Some((expected.into(), 1, count as u8)));
        }
        assert_eq!(decode(&[1; 100]), None);
    }
    #[test]
    fn all_reference_schemas_preserve_exact_non_byte_aligned_payloads() {
        let receipt: serde_json::Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/trustmark/provenance.json"
        ))
        .unwrap_or_default();
        let vectors = receipt["bch_vectors"].as_array();
        assert!(vectors.is_some_and(|v| v.len() == 8));
        if let Some(vectors) = vectors {
            for v in vectors {
                let packet = v["packet"].as_str().unwrap_or_default();
                let payload = v["payload_bits"].as_str().unwrap_or_default();
                let schema = v["schema_value"].as_u64().unwrap_or(255) as u8;
                let mut bits = [0; 100];
                assert_eq!(packet.len(), 100);
                for (b, c) in bits.iter_mut().zip(packet.bytes()) {
                    *b = c - b'0';
                }
                assert_eq!(decode(&bits), Some((payload.into(), schema, 0)));
                // Every transmitted data/ECC position can be corrected independently.
                for position in 0..96 {
                    let mut changed = bits;
                    changed[position] ^= 1;
                    assert_eq!(decode(&changed), Some((payload.into(), schema, 1)));
                }
                for count in 1..=usize::from(limit(schema)) {
                    bits[(count - 1) * 11] ^= 1;
                    assert_eq!(decode(&bits), Some((payload.into(), schema, count as u8)));
                }
            }
        }
    }
}
