use anyhow::Result;

pub trait EmbeddingProvider: Send + Sync {
    fn generate(&self, text: &str) -> Result<Vec<f32>>;
    fn dimension(&self) -> usize;
}

pub struct OnnxEmbeddingProvider {
    _model_path: String,
    _session: Option<()>,
}

impl OnnxEmbeddingProvider {
    pub fn new(model_path: &str) -> Result<Self> {
        Ok(Self {
            _model_path: model_path.to_string(),
            _session: None,
        })
    }
}

impl EmbeddingProvider for OnnxEmbeddingProvider {
    fn generate(&self, _text: &str) -> Result<Vec<f32>> {
        Err(anyhow::anyhow!(
            "ONNX 提供程序未启用。请参考以下步骤启用:\n\
             1. 在 Cargo.toml 中添加: onnxruntime = \"0.19\"\n\
             2. 下载合适的 sentence-transformers 模型 (如 all-MiniLM-L6-v2)\n\
             3. 在 new() 中初始化 onnxruntime::Session\n\
             4. 在 generate() 中实现 tokenizer + inference\n\
             5. 当前默认使用 TF-IDF fallback 方案"
        ))
    }

    fn dimension(&self) -> usize {
        384
    }
}

const VECTOR_DIM: usize = 384;

pub struct TfIdfFallbackProvider;

impl TfIdfFallbackProvider {
    pub fn new() -> Self {
        Self
    }

    fn char_ngrams(text: &str, n: usize) -> Vec<String> {
        let chars: Vec<char> = text.chars().collect();
        let mut ngrams = Vec::new();
        if chars.len() < n {
            if !chars.is_empty() {
                ngrams.push(chars.iter().collect());
            }
            return ngrams;
        }
        for i in 0..=chars.len() - n {
            ngrams.push(chars[i..i + n].iter().collect());
        }
        ngrams
    }

    fn word_tokens(text: &str) -> Vec<String> {
        text.split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_lowercase())
            .collect()
    }

    fn hash_to_index(s: &str, dim: usize) -> usize {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        s.hash(&mut hasher);
        (hasher.finish() as usize) % dim
    }

    fn hash_sign(s: &str) -> f32 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        ("sign", s).hash(&mut hasher);
        if hasher.finish() % 2 == 0 { 1.0 } else { -1.0 }
    }
}

impl EmbeddingProvider for TfIdfFallbackProvider {
    fn generate(&self, text: &str) -> Result<Vec<f32>> {
        let mut vector = vec![0.0f32; VECTOR_DIM];
        let text_lower = text.to_lowercase();

        let words = Self::word_tokens(&text_lower);
        let word_count = words.len().max(1) as f32;
        for word in &words {
            let idx = Self::hash_to_index(word, VECTOR_DIM);
            let sign = Self::hash_sign(word);
            vector[idx] += sign;
        }
        let tf_scale = 1.0 / word_count;
        for v in vector.iter_mut() {
            *v *= tf_scale;
        }

        for n in 2..=4 {
            let ngrams = Self::char_ngrams(&text_lower, n);
            let ngram_count = ngrams.len().max(1) as f32;
            let ngram_weight = 0.3 / ngram_count;
            for ng in &ngrams {
                let idx = Self::hash_to_index(&format!("ng{}:{}", n, ng), VECTOR_DIM);
                let sign = Self::hash_sign(&format!("ng{}:{}", n, ng));
                vector[idx] += sign * ngram_weight;
            }
        }

        let special_tokens = [
            ("fn", 0.5), ("function", 0.5), ("class", 0.5), ("struct", 0.5),
            ("impl", 0.5), ("let", 0.3), ("mut", 0.3), ("const", 0.3),
            ("async", 0.4), ("await", 0.4), ("return", 0.3), ("if", 0.2),
            ("else", 0.2), ("for", 0.2), ("while", 0.2), ("match", 0.4),
            ("use", 0.3), ("mod", 0.3), ("pub", 0.3), ("self", 0.2),
            ("def", 0.5), ("import", 0.4), ("from", 0.2), ("true", 0.2),
            ("false", 0.2), ("null", 0.2), ("nil", 0.2), ("none", 0.2),
            ("var", 0.3), ("val", 0.3), ("type", 0.3), ("interface", 0.4),
            ("package", 0.4), ("main", 0.3), ("import", 0.4), ("export", 0.3),
            ("test", 0.4), ("todo", 0.5), ("fixme", 0.5), ("hack", 0.5),
            ("error", 0.5), ("exception", 0.5), ("warn", 0.4), ("debug", 0.4),
            ("info", 0.3), ("log", 0.3), ("doc", 0.4), ("comment", 0.3),
        ];
        for (token, weight) in special_tokens.iter() {
            if text_lower.contains(token) {
                let idx = Self::hash_to_index(&format!("tk:{}", token), VECTOR_DIM);
                let sign = Self::hash_sign(token);
                vector[idx] += sign * weight;
            }
        }

        let norm: f32 = vector.iter().map(|v| v * v).sum::<f32>().sqrt();
        if norm > 1e-8 {
            for v in vector.iter_mut() {
                *v /= norm;
            }
        } else {
            vector[0] = 1.0;
        }

        Ok(vector)
    }

    fn dimension(&self) -> usize {
        VECTOR_DIM
    }
}
