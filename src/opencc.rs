use ferrous_opencc::OpenCC;

/// Simplified Chinese → Traditional Chinese (Taiwan, s2twp).
pub fn s2twp(text: &str) -> String {
    convert(text, "s2twp")
}

/// Traditional Chinese (Taiwan) → Simplified Chinese (t2sp).
pub fn t2sp(text: &str) -> String {
    convert(text, "t2sp")
}

/// Apply conversion by mode string ("s2twp" | "t2sp" | anything else = passthrough).
pub fn apply_mode(text: &str, mode: &str) -> String {
    match mode {
        "s2twp" => s2twp(text),
        "t2sp"  => t2sp(text),
        _       => text.to_string(),
    }
}

fn convert(text: &str, config: &str) -> String {
    match OpenCC::new(config) {
        Ok(cc) => cc.convert(text),
        Err(_) => text.to_string(),
    }
}
