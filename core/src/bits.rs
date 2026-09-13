//! One year of progress as 366 bits.
//!
//! This is a derived view of the day log, kept because it is the original web
//! app's on-disk format and therefore the format imports and exports speak.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::date::MAX_DAYS;

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub struct YearBits([u64; 6]);

/// Six bits per character, 61 characters per year — the encoding
/// `everydaycalendar.app` wrote to `localStorage`.
const ENCODED_LEN: usize = 61;
const ENCODE_BASE: u8 = b'*';

impl YearBits {
    pub fn get(&self, day: usize) -> bool {
        day < MAX_DAYS && self.0[day / 64] & (1 << (day % 64)) != 0
    }

    pub fn set(&mut self, day: usize, on: bool) {
        if day >= MAX_DAYS {
            return;
        }
        let mask = 1u64 << (day % 64);
        if on {
            self.0[day / 64] |= mask;
        } else {
            self.0[day / 64] &= !mask;
        }
    }

    pub fn count(&self) -> u32 {
        self.0.iter().map(|word| word.count_ones()).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.0.iter().all(|word| *word == 0)
    }

    pub fn days(&self) -> impl Iterator<Item = usize> + '_ {
        (0..MAX_DAYS).filter(|day| self.get(*day))
    }

    pub fn encode(&self) -> String {
        let mut out = String::with_capacity(ENCODED_LEN);
        for chunk in 0..ENCODED_LEN {
            let mut value = 0u8;
            for bit in 0..6 {
                if self.get(chunk * 6 + bit) {
                    value |= 1 << bit;
                }
            }
            out.push((ENCODE_BASE + value) as char);
        }
        out
    }

    pub fn decode(text: &str) -> Self {
        let mut bits = Self::default();
        for (chunk, byte) in text.bytes().take(ENCODED_LEN).enumerate() {
            if !(ENCODE_BASE..ENCODE_BASE + 64).contains(&byte) {
                continue;
            }
            let value = byte - ENCODE_BASE;
            for bit in 0..6 {
                if value & (1 << bit) != 0 {
                    bits.set(chunk * 6 + bit, true);
                }
            }
        }
        bits
    }
}

impl Serialize for YearBits {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.encode())
    }
}

impl<'de> Deserialize<'de> for YearBits {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(Self::decode(&String::deserialize(d)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrips_the_legacy_encoding() {
        let mut bits = YearBits::default();
        for day in [0usize, 1, 5, 6, 59, 200, 364, 365] {
            bits.set(day, true);
        }
        let encoded = bits.encode();
        assert_eq!(encoded.len(), ENCODED_LEN);
        assert_eq!(YearBits::decode(&encoded), bits);
        assert_eq!(bits.count(), 8);
    }

    #[test]
    fn matches_the_original_bit_layout() {
        // The original packed bit `i` as `state[i / 6] & (1 << (i % 6))` and
        // wrote each six-bit group as `char(42 + value)`.
        let mut bits = YearBits::default();
        bits.set(0, true);
        assert!(bits.encode().starts_with('+'));

        let mut bits = YearBits::default();
        bits.set(5, true);
        assert!(bits.encode().starts_with('J'));
    }
}
