use std::sync::Arc;

// ── Public types ─────────────────────────────────────────────────────────────

pub struct CorrectionResult {
    pub text: String,
    pub spell_fixes: usize,
    pub grammar_hints: usize,
    pub lt_fixes: usize,
}

impl CorrectionResult {
    pub fn log_line(&self, is_zh: bool) -> String {
        let total = self.spell_fixes + self.lt_fixes;
        if is_zh {
            if total == 0 && self.grammar_hints == 0 {
                "校正完成：無需修改".to_string()
            } else {
                format!(
                    "校正完成 — SymSpell:{} LT:{} Harper提示:{}",
                    self.spell_fixes, self.lt_fixes, self.grammar_hints
                )
            }
        } else if total == 0 && self.grammar_hints == 0 {
            "Correction done: nothing to fix".to_string()
        } else {
            format!(
                "Corrected — SymSpell:{} LT:{} Harper hints:{}",
                self.spell_fixes, self.lt_fixes, self.grammar_hints
            )
        }
    }
}

/// Entry point — runs all three engines in sequence.
/// Individual engine failures are logged to stderr and skipped gracefully.
pub fn correct_text(text: &str, language: &str) -> CorrectionResult {
    let is_english = matches!(language, "en" | "auto" | "");
    let mut current = text.to_string();

    // 1. SymSpell — fast local spell correction (English, optional dict file)
    let spell_fixes = if is_english {
        match run_symspell(&current) {
            Ok((fixed, n)) => { current = fixed; n }
            Err(e) => { eprintln!("[SymSpell] {e}"); 0 }
        }
    } else {
        0
    };

    // 2. Harper — lightweight English grammar (count only; user edits manually)
    let grammar_hints = if is_english {
        run_harper(&current).unwrap_or_else(|e| { eprintln!("[Harper] {e}"); 0 })
    } else {
        0
    };

    // 3. LanguageTool — multilingual grammar + spelling over HTTP
    let lt_fixes = match run_languagetool(&current, language) {
        Ok((fixed, n)) => { current = fixed; n }
        Err(e) => { eprintln!("[LanguageTool] {e}"); 0 }
    };

    CorrectionResult { text: current, spell_fixes, grammar_hints, lt_fixes }
}

// ── SymSpell ─────────────────────────────────────────────────────────────────

const DICT_CANDIDATES: &[&str] = &[
    "dicts/en_frequency.txt",
    "frequency_dictionary_en_82_765.txt",
];

fn run_symspell(text: &str) -> anyhow::Result<(String, usize)> {
    use symspell::{AsciiStringStrategy, SymSpell, Verbosity};

    let dict_path = DICT_CANDIDATES
        .iter()
        .find(|p| std::path::Path::new(p).exists())
        .ok_or_else(|| anyhow::anyhow!(
            "SymSpell: no dictionary found. Add en_frequency.txt to dicts/ to enable spell check."
        ))?;

    let mut ss: SymSpell<AsciiStringStrategy> = SymSpell::default();
    ss.load_dictionary(dict_path, 0, 1, " ");

    let mut fixes = 0usize;
    let result = text
        .split_whitespace()
        .map(|word| {
            let alpha = word.trim_matches(|c: char| !c.is_ascii_alphabetic());
            if alpha.len() < 3 || !alpha.chars().all(|c| c.is_ascii_alphabetic()) {
                return word.to_string();
            }
            let suggestions = ss.lookup(alpha, Verbosity::Closest, 1);
            if let Some(best) = suggestions.first() {
                if best.distance == 1 && best.term != alpha.to_ascii_lowercase() {
                    fixes += 1;
                    return word.replacen(alpha, &best.term, 1);
                }
            }
            word.to_string()
        })
        .collect::<Vec<_>>()
        .join(" ");

    Ok((result, fixes))
}

// ── Harper ───────────────────────────────────────────────────────────────────

fn run_harper(text: &str) -> anyhow::Result<usize> {
    use harper_core::linting::{LintGroup, Linter};
    use harper_core::spell::FstDictionary;
    use harper_core::{Dialect, Document};

    let dict: Arc<FstDictionary> = FstDictionary::curated();
    let doc = Document::new_plain_english(text, &dict);
    let mut linter = LintGroup::new_curated(dict.clone(), Dialect::American);
    let lints = linter.lint(&doc);
    Ok(lints.len())
}

// ── LanguageTool ─────────────────────────────────────────────────────────────

fn map_lt_language(lang: &str) -> &'static str {
    match lang {
        "zh"  => "zh-TW",
        "en"  => "en-US",
        "ja"  => "ja-JP",
        "de"  => "de-DE",
        "fr"  => "fr-FR",
        "es"  => "es-ES",
        "pt"  => "pt-BR",
        "ru"  => "ru-RU",
        "it"  => "it-IT",
        "nl"  => "nl-NL",
        "pl"  => "pl-PL",
        "ar"  => "ar",
        "ko"  => "ko",
        "tr"  => "tr",
        "vi"  => "vi",
        _     => "auto",
    }
}

fn run_languagetool(text: &str, language: &str) -> anyhow::Result<(String, usize)> {
    use languagetool_rust::api::check::Request;
    use languagetool_rust::api::server::ServerClient;

    let lt_lang = map_lt_language(language);

    // Create a single-thread tokio runtime — background thread has no runtime.
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    let response = rt.block_on(async {
        let client = ServerClient::from_env_or_default();
        let mut req = Request::default();
        req.language = lt_lang.to_string();
        req.text = Some(text.to_string().into());
        client.check(&req).await
    });

    let response = match response {
        Ok(r) => r,
        Err(e) => {
            // Network failure is non-fatal; return original text unchanged.
            eprintln!("[LanguageTool] network error: {e}");
            return Ok((text.to_string(), 0));
        }
    };

    // Apply matches end-to-start so earlier char offsets remain valid after splice.
    let mut result: Vec<char> = text.chars().collect();
    let total_chars = result.len();

    let mut matches = response.matches;
    matches.sort_by(|a, b| b.offset.cmp(&a.offset));

    let mut fixes = 0usize;
    let mut applied_start = total_chars; // track to skip overlapping patches

    for m in &matches {
        let start = m.offset;
        let end = start + m.length;
        if end > applied_start || end > result.len() {
            continue; // out of bounds or overlapping
        }
        if let Some(rep) = m.replacements.first() {
            let replacement: Vec<char> = rep.value.chars().collect();
            result.splice(start..end, replacement);
            applied_start = start;
            fixes += 1;
        }
    }

    Ok((result.into_iter().collect(), fixes))
}
