/*
 * Quantic standalone adapter for Ladybird's NamedCharacterReferenceMatcher API.
 * Entity data is generated from Ladybird's Entities.json at build time.
 * SPDX-License-Identifier: BSD-2-Clause
 */

include!(concat!(env!("OUT_DIR"), "/entities_generated.rs"));

pub struct NamedCharacterReferenceMatcher {
    consumed: String,
    last_match: Option<(u32, u32)>,
    last_match_len: usize,
    ends_with_semicolon: bool,
}

impl NamedCharacterReferenceMatcher {
    pub fn new() -> Self {
        Self {
            consumed: String::new(),
            last_match: None,
            last_match_len: 0,
            ends_with_semicolon: false,
        }
    }

    pub fn try_consume_code_point(&mut self, c: u32) -> bool {
        let Some(ch) = char::from_u32(c) else {
            return false;
        };
        if !ch.is_ascii() {
            return false;
        }
        self.consumed.push(ch);

        let mut any_prefix = false;
        for &(name, first, second) in ENTITIES {
            if name.starts_with(&self.consumed) {
                any_prefix = true;
            }
            if name == self.consumed {
                self.last_match = Some((first, second));
                self.last_match_len = self.consumed.len();
                self.ends_with_semicolon = name.ends_with(';');
            }
        }
        any_prefix
    }

    pub fn code_points(&self) -> Option<(u32, u32)> {
        self.last_match
    }

    pub fn overconsumed_code_points(&self) -> u8 {
        self.consumed
            .len()
            .saturating_sub(self.last_match_len)
            .min(u8::MAX as usize) as u8
    }

    pub fn last_match_ends_with_semicolon(&self) -> bool {
        self.ends_with_semicolon
    }
}

impl Default for NamedCharacterReferenceMatcher {
    fn default() -> Self {
        Self::new()
    }
}
