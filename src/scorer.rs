use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Embedding(pub Vec<f32>);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scorer {
    pub history: Vec<Embedding>,
    pub threshold: f32,
    pub consecutive_turns: usize,
}

impl Scorer {
    pub fn new(threshold: f32, consecutive_turns: usize) -> Self {
        Self {
            history: Vec::with_capacity(5),
            threshold,
            consecutive_turns,
        }
    }

    pub fn add_and_check(&mut self, embedding: Embedding) -> bool {
        self.history.push(embedding.clone());
        if self.history.len() > 5 {
            self.history.remove(0);
        }

        if self.history.len() < self.consecutive_turns {
            return false;
        }

        // Check last N turns for semantic "stalling"
        let last_n = &self.history[self.history.len() - self.consecutive_turns..];
        let mut loop_detected = true;

        for i in 0..last_n.len() - 1 {
            let similarity = cosine_similarity(&last_n[i].0, &last_n[i+1].0);
            // Distance = 1 - Similarity. 
            // Distance < 0.02 means Similarity > 0.98
            if similarity < (1.0 - self.threshold) {
                loop_detected = false;
                break;
            }
        }

        loop_detected
    }
}

/// Calculates cosine similarity between two vectors.
/// Assumes vectors are already normalized (standard for OpenAI embeddings).
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }
    
    let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    
    // If they aren't normalized, we'd need: dot / (norm_a * norm_b)
    // But OpenAI text-embedding-3-small/large are pre-normalized to 1.0.
    dot_product
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_similarity_identical() {
        let v1 = vec![1.0, 0.0, 0.0];
        let v2 = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&v1, &v2) - 1.0).abs() < 0.0001);
    }

    #[test]
    fn test_similarity_orthogonal() {
        let v1 = vec![1.0, 0.0, 0.0];
        let v2 = vec![0.0, 1.0, 0.0];
        assert!((cosine_similarity(&v1, &v2) - 0.0).abs() < 0.0001);
    }

    #[test]
    fn test_loop_detection() {
        let mut scorer = Scorer::new(0.02, 3);
        let v1 = Embedding(vec![1.0, 0.0]);
        let v2 = Embedding(vec![0.99, 0.01]); // very close
        let v3 = Embedding(vec![0.985, 0.015]); // very close
        
        assert_eq!(scorer.add_and_check(v1), false);
        assert_eq!(scorer.add_and_check(v2), false);
        assert_eq!(scorer.add_and_check(v3), true); // Loop detected!
    }
}
