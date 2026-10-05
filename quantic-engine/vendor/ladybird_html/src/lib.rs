/*
 * Ladybird HTML tokenizer adapted for standalone Quantic embedding.
 * Upstream: LadybirdBrowser/ladybird @ a4140db626af72d0a676075a1ba26be728f4ea39
 * License: BSD-2-Clause
 */
pub mod entities;
pub(crate) mod known_names;
pub mod token;
pub mod tokenizer;

pub use token::{Attribute, HtmlName, Token, TokenPayload, TokenType};
pub use tokenizer::{HtmlTokenizer, State};
