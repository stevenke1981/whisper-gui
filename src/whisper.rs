use anyhow::Context;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

pub fn model_path(model_name: &str, models_dir: &str) -> String {
    format!("{}/ggml-{}.bin", models_dir, model_name)
}

pub fn model_exists(model_name: &str, models_dir: &str) -> bool {
    std::path::Path::new(&model_path(model_name, models_dir)).exists()
}

// ── Dictionary helpers ────────────────────────────────────────────────────────

/// Known downloadable dictionaries: (id, filename, url, description)
pub const DICT_ENTRIES: &[(&str, &str, &str, &str)] = &[
    (
        "en",
        "en_frequency.txt",
        "https://raw.githubusercontent.com/wolfgarbe/SymSpell/master/SymSpell/frequency_dictionary_en_82_765.txt",
        "English SymSpell frequency dictionary (~30 MB)",
    ),
];

pub fn dict_path(filename: &str, dicts_dir: &str) -> String {
    format!("{}/{}", dicts_dir, filename)
}

pub fn dict_exists(filename: &str, dicts_dir: &str) -> bool {
    std::path::Path::new(&dict_path(filename, dicts_dir)).exists()
}

/// Download a dictionary by id. Skips if already present.
pub fn download_dict(dict_id: &str, dicts_dir: &str) -> anyhow::Result<String> {
    let entry = DICT_ENTRIES
        .iter()
        .find(|(id, _, _, _)| *id == dict_id)
        .with_context(|| format!("Unknown dict id: {}", dict_id))?;

    let (_id, filename, url, _desc) = entry;
    let dest = dict_path(filename, dicts_dir);

    if std::path::Path::new(&dest).exists() {
        return Ok(dest);
    }

    std::fs::create_dir_all(dicts_dir)
        .with_context(|| format!("Failed to create directory: {}", dicts_dir))?;

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()?;

    let mut resp = client
        .get(*url)
        .send()
        .with_context(|| format!("HTTP request failed: {}", url))?;

    if !resp.status().is_success() {
        anyhow::bail!("HTTP {} downloading {}", resp.status(), url);
    }

    let mut file = std::fs::File::create(&dest)
        .with_context(|| format!("Failed to create file: {}", dest))?;

    resp.copy_to(&mut file)?;
    Ok(dest)
}

pub fn download_model(model_name: &str, models_dir: &str) -> anyhow::Result<String> {
    let dest_path = model_path(model_name, models_dir);

    // Skip download if file already exists
    if std::path::Path::new(&dest_path).exists() {
        return Ok(dest_path);
    }

    let url = format!(
        "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-{}.bin",
        model_name
    );

    std::fs::create_dir_all(models_dir)
        .with_context(|| format!("Failed to create directory: {}", models_dir))?;

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(1800))
        .build()?;

    let mut response = client
        .get(&url)
        .send()
        .with_context(|| format!("HTTP request failed: {}", url))?;

    if !response.status().is_success() {
        anyhow::bail!("HTTP {} downloading {}", response.status(), url);
    }

    let mut file = std::fs::File::create(&dest_path)
        .with_context(|| format!("Failed to create file: {}", dest_path))?;

    response.copy_to(&mut file)?;

    Ok(dest_path)
}

// ── Transcription parameters ──────────────────────────────────────────────────

pub struct TranscribeParams {
    pub language: String,
    pub n_threads: Option<i32>,
    pub temperature: f32,
    pub beam_size: i32,
    pub best_of: i32,
    pub no_context: bool,
    pub single_segment: bool,
    pub word_timestamps: bool,
    pub initial_prompt: String,
    pub max_len: i32,
}

impl Default for TranscribeParams {
    fn default() -> Self {
        Self {
            language: "auto".to_string(),
            n_threads: None,
            temperature: 0.0,
            beam_size: 5,
            best_of: 5,
            no_context: false,
            single_segment: false,
            word_timestamps: false,
            initial_prompt: String::new(),
            max_len: 0,
        }
    }
}

pub struct WhisperEngine {
    ctx: Option<WhisperContext>,
    model_name: String,
    model_path: String,
    use_gpu: bool,
}

impl WhisperEngine {
    pub fn new() -> Self {
        Self {
            ctx: None,
            model_name: String::new(),
            model_path: String::new(),
            use_gpu: false,
        }
    }

    pub fn backend_label(&self) -> &str {
        if !self.use_gpu {
            return "CPU";
        }
        // Only report GPU if the crate was actually compiled with a GPU backend.
        // Without the feature flag whisper-rs silently falls back to CPU regardless
        // of what use_gpu(true) was called with.
        #[cfg(feature = "cuda")]   { return "CUDA"; }
        #[cfg(feature = "vulkan")] { return "Vulkan"; }
        #[allow(unreachable_code)]
        "CPU"
    }

    pub fn is_loaded(&self) -> bool {
        self.ctx.is_some()
    }

    pub fn model_name(&self) -> &str {
        &self.model_name
    }

    pub fn load_model(&mut self, model_path: &str, use_gpu: bool) -> anyhow::Result<()> {
        let path = std::path::Path::new(model_path);
        if !path.exists() {
            anyhow::bail!("Model file not found: {}", model_path);
        }

        let mut ctx_params = WhisperContextParameters::default();
        ctx_params.use_gpu(use_gpu);
        let ctx = WhisperContext::new_with_params(model_path, ctx_params)
            .with_context(|| format!("Failed to load model: {}", model_path))?;

        self.model_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();
        self.model_path = model_path.to_string();
        self.use_gpu = use_gpu;
        self.ctx = Some(ctx);

        Ok(())
    }

    /// Returns `(transcribed_text, elapsed_secs)`
    pub fn transcribe_pcm(&self, pcm_data: &[f32], params: &TranscribeParams) -> anyhow::Result<(String, f64)> {
        let ctx = self.ctx.as_ref().context("Model not loaded")?;

        let strategy = if params.beam_size >= 2 {
            SamplingStrategy::BeamSearch {
                beam_size: params.beam_size,
                patience: -1.0,
            }
        } else {
            SamplingStrategy::Greedy { best_of: params.best_of }
        };

        let mut fp = FullParams::new(strategy);

        fp.set_n_threads(params.n_threads.unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|n| n.get() as i32)
                .unwrap_or(4)
        }));

        let lang_opt: Option<&str> = if params.language == "auto" { None } else { Some(&params.language) };
        fp.set_language(lang_opt);
        fp.set_translate(false);
        fp.set_no_context(params.no_context);
        fp.set_single_segment(params.single_segment);
        fp.set_print_special(false);
        fp.set_print_progress(false);
        fp.set_print_realtime(false);
        fp.set_print_timestamps(false);
        fp.set_temperature(params.temperature);
        fp.set_token_timestamps(params.word_timestamps);
        if !params.initial_prompt.is_empty() {
            fp.set_initial_prompt(&params.initial_prompt);
        }
        if params.max_len > 0 {
            fp.set_max_len(params.max_len);
        }

        let mut state = ctx.create_state().context("Failed to create state")?;
        let t0 = std::time::Instant::now();
        state.full(fp, pcm_data).context("Whisper transcription failed")?;
        let elapsed = t0.elapsed().as_secs_f64();

        let n = state.full_n_segments();
        let mut result = String::new();
        for i in 0..n {
            if let Some(segment) = state.get_segment(i) {
                if let Ok(text) = segment.to_str() {
                    if !result.is_empty() { result.push('\n'); }
                    result.push_str(text);
                }
            }
        }

        Ok((result, elapsed))
    }

    pub fn estimate_memory(&self) -> String {
        match std::fs::metadata(&self.model_path) {
            Ok(meta) => {
                let mb = meta.len() as f64 / (1024.0 * 1024.0);
                format!("Model: {:.0} MB", mb)
            }
            Err(_) => String::new(),
        }
    }
}

// SAFETY: WhisperEngine is always accessed through Arc<Mutex<>>, serializing
// all access. whisper.cpp is safe to move between threads when access is serialized.
unsafe impl Send for WhisperEngine {}