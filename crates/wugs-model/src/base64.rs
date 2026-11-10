//! Standard base64 (RFC 4648, with padding), for `text-edit` loro payloads.
//! Zero-dependency on purpose, like the rest of the model crate.

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = u32::from_be_bytes([0, b[0], b[1], b[2]]);
        let chars = [
            ALPHABET[(n >> 18) as usize & 63],
            ALPHABET[(n >> 12) as usize & 63],
            ALPHABET[(n >> 6) as usize & 63],
            ALPHABET[n as usize & 63],
        ];
        let keep = chunk.len() + 1;
        for (i, c) in chars.iter().enumerate() {
            out.push(if i < keep { *c as char } else { '=' });
        }
    }
    out
}

fn value_of(c: u8) -> Option<u32> {
    match c {
        b'A'..=b'Z' => Some((c - b'A') as u32),
        b'a'..=b'z' => Some((c - b'a') as u32 + 26),
        b'0'..=b'9' => Some((c - b'0') as u32 + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

pub fn decode(s: &str) -> Result<Vec<u8>, String> {
    let bytes: Vec<u8> = s.bytes().filter(|b| !b" \t\r\n".contains(b)).collect();
    let trimmed = match bytes.iter().position(|&b| b == b'=') {
        Some(i) if bytes[i..].iter().all(|&b| b == b'=') && bytes.len() - i <= 2 => &bytes[..i],
        Some(i) => return Err(format!("stray '=' at {i}")),
        None => &bytes[..],
    };
    if trimmed.len() % 4 == 1 {
        return Err("truncated base64".to_string());
    }
    let mut out = Vec::with_capacity(trimmed.len() * 3 / 4);
    for chunk in trimmed.chunks(4) {
        let mut n: u32 = 0;
        for (i, &c) in chunk.iter().enumerate() {
            let v = value_of(c).ok_or_else(|| format!("invalid base64 byte {:?}", c as char))?;
            n |= v << (18 - 6 * i);
        }
        let b = n.to_be_bytes();
        out.extend_from_slice(&b[1..chunk.len()]);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        for len in 0..40usize {
            let data: Vec<u8> = (0..len as u8).map(|i| i.wrapping_mul(37).wrapping_add(len as u8)).collect();
            let enc = encode(&data);
            assert_eq!(enc.len() % 4, 0, "padded length for {len}");
            assert_eq!(decode(&enc).unwrap(), data, "round trip at {len}");
        }
    }

    #[test]
    fn known_vectors() {
        assert_eq!(encode(b""), "");
        assert_eq!(encode(b"f"), "Zg==");
        assert_eq!(encode(b"fo"), "Zm8=");
        assert_eq!(encode(b"foo"), "Zm9v");
        assert_eq!(encode(b"foobar"), "Zm9vYmFy");
        assert_eq!(decode("Zm9vYmFy").unwrap(), b"foobar");
    }

    #[test]
    fn lenient_on_missing_padding_only() {
        assert_eq!(decode("Zm8").unwrap(), b"fo", "unpadded input accepted");
        assert!(decode("A").is_err(), "length 1 mod 4");
        assert!(decode("Z!==").is_err(), "invalid byte");
        assert!(decode("Zg=X").is_err(), "non-trailing padding");
        assert!(decode("Z===").is_err(), "too much padding");
    }
}
