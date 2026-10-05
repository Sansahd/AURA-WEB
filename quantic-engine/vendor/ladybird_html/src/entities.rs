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

        // Important: a character that does not extend any named entity must not
        // mutate the matcher. Ladybird's tokenizer will reconsume that character.
        // Mutating here caused the adapter to backtrack one code point too far
        // after entities such as &amp;.
        let mut candidate = self.consumed.clone();
        candidate.push(ch);

        let mut any_prefix = false;
        let mut exact_match = None;
        for &(name, first, second) in ENTITIES {
            if name.starts_with(&candidate) {
                any_prefix = true;
            }
            if name == candidate {
                exact_match = Some((first, second, name.ends_with(';')));
            }
        }

        if !any_prefix {
            return false;
        }

        self.consumed = candidate;
        if let Some((first, second, ends_with_semicolon)) = exact_match {
            self.last_match = Some((first, second));
            self.last_match_len = self.consumed.len();
            self.ends_with_semicolon = ends_with_semicolon;
        }
        true
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
