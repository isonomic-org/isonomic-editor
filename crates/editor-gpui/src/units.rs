// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Offsets are UTF-8 bytes everywhere except at the platform's text
//! input boundary, which counts UTF-16 units. These convert between
//! the two, clamping to the text.

pub fn to_utf16(text: &str, byte: usize) -> usize {
    let mut at = byte.min(text.len());
    while !text.is_char_boundary(at) {
        at -= 1;
    }
    text[..at].encode_utf16().count()
}

pub fn from_utf16(text: &str, units: usize) -> usize {
    let mut seen = 0;
    for (byte, ch) in text.char_indices() {
        if seen >= units {
            return byte;
        }
        seen += ch.len_utf16();
    }
    text.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversions_round_trip_and_clamp() {
        let text = "aé😀z";
        for byte in [0, 1, 3, 7, 8] {
            assert_eq!(from_utf16(text, to_utf16(text, byte)), byte);
        }
        assert_eq!(to_utf16(text, 7), 4);
        assert_eq!(to_utf16(text, 5), 2, "inside a character rounds down");
        assert_eq!(from_utf16(text, 99), text.len());
    }
}
