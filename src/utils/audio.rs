pub const SAMPLE_RATE: usize = 16_000;
pub const MAX_AUDIO_SECONDS: usize = 120;

/// Декодирует little-endian PCM16 в нормализованные f32-сэмплы, добавляя
/// в `samples` не больше `remaining` штук (ограничивает буфер снаружи).
pub fn decode_pcm16(bytes: &[u8], samples: &mut Vec<f32>, remaining: usize) {
    for pair in bytes.chunks_exact(2).take(remaining) {
        let value = i16::from_le_bytes([pair[0], pair[1]]);
        samples.push(value as f32 / i16::MAX as f32);
    }
}

pub fn duration_ms(sample_count: usize) -> u64 {
    (sample_count as u64 * 1_000) / SAMPLE_RATE as u64
}

/// Грубая проверка «в буфере вообще есть голос», чтобы не гонять модель
/// на тишине — это дорого и даёт мусорные партиалы.
pub fn has_audible_signal(samples: &[f32]) -> bool {
    let (sum_squares, peak) = samples.iter().fold((0.0f64, 0.0f32), |acc, sample| {
        (
            acc.0 + f64::from(*sample) * f64::from(*sample),
            acc.1.max(sample.abs()),
        )
    });
    let rms = (sum_squares / samples.len().max(1) as f64).sqrt() as f32;
    peak >= 0.01 && rms >= 0.002
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_silence_before_inference() {
        assert!(!has_audible_signal(&vec![0.0; SAMPLE_RATE]));
    }

    #[test]
    fn accepts_audible_pcm() {
        let samples = (0..SAMPLE_RATE)
            .map(|index| if index % 2 == 0 { 0.05 } else { -0.05 })
            .collect::<Vec<_>>();
        assert!(has_audible_signal(&samples));
    }
}
