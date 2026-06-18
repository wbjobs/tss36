pub mod onnx;

use once_cell::sync::Lazy;
use onnx::{EmbeddingProvider, TfIdfFallbackProvider};

static FALLBACK_PROVIDER: Lazy<TfIdfFallbackProvider> = Lazy::new(|| TfIdfFallbackProvider::new());

pub fn generate_embedding(text: &str) -> Vec<f32> {
    FALLBACK_PROVIDER.generate(text).unwrap_or_else(|_| {
        let mut v = vec![0.0f32; 384];
        v[0] = 1.0;
        v
    })
}

pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;
    for i in 0..a.len() {
        let ai = a[i];
        let bi = b[i];
        dot += ai * bi;
        norm_a += ai * ai;
        norm_b += bi * bi;
    }
    let denom = norm_a.sqrt() * norm_b.sqrt();
    if denom < 1e-8 {
        0.0
    } else {
        (dot / denom).clamp(-1.0, 1.0)
    }
}
