/*
 * Adaptation for Quantic Fusion.
 * Original Ladybird tokenizer is BSD-2-Clause.
 *
 * Ladybird normally maps well-known names to AK/C++ fly-string identities.
 * Quantic is a standalone Rust embedder, so literals remain Rust strings.
 */
macro_rules! tag_name {
    ($name:literal) => {
        $crate::token::KnownName::from_literal($name)
    };
}
pub(crate) use tag_name;

macro_rules! attribute_name {
    ($name:literal) => {
        $crate::token::KnownName::from_literal($name)
    };
}
pub(crate) use attribute_name;
