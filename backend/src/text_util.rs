use once_cell::sync::Lazy;
use regex::Regex;

static MEANINGLESS_SYMBOLS: Lazy<Regex> = Lazy::new(|| Regex::new("(?i)[^a-zа-я0-9]+").unwrap());

pub fn answer_to_standard(text: &str) -> String {
    MEANINGLESS_SYMBOLS.replace(text, "").to_lowercase()
}
