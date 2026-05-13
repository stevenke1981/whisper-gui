use anyhow::Context;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

pub fn download_model(model_name: &str, models_dir: &str) -> anyhow::Result<String> {
    let url = format!(
        "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-{}.bin",
        model_name
    );

    std::fs::create_dir_all(models_dir)
        .with_context(|| format!("Failed to create directory: {}", models_dir))?;

    let dest_path = format!("{}/ggml-{}.bin", models_dir, model_name);

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
    pub fn transcribe_pcm(&self, pcm_data: &[f32], language: &str, n_threads: Option<i32>) -> anyhow::Result<(String, f64)> {
        let ctx = self.ctx.as_ref().context("Model not loaded")?;

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 0 });

        params.set_n_threads(n_threads.unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|n| n.get() as i32)
                .unwrap_or(4)
        }));

        let lang_opt: Option<&str> = if language == "auto" { None } else { Some(language) };
        params.set_language(lang_opt);
        params.set_translate(false);
        params.set_no_context(true);
        params.set_single_segment(true);
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);

        let mut state = ctx.create_state().context("Failed to create state")?;
        let t0 = std::time::Instant::now();
        state
            .full(params, pcm_data)
            .context("Whisper transcription failed")?;
        let elapsed = t0.elapsed().as_secs_f64();

        let segment_count = state.full_n_segments();

        let mut result = String::new();
        for i in 0..segment_count {
            if let Some(segment) = state.get_segment(i) {
                if let Ok(text) = segment.to_str() {
                    if !result.is_empty() {
                        result.push('\n');
                    }
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