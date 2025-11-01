/// Generate deterministic coil data for snapshot-friendly tests.
///
/// Boolean placeholders rely on scanning `OFF` entries sequentially, so
/// providing stable values keeps reference screenshots deterministic.
/// Runtime tests may still toggle individual coils as needed.
pub fn generate_random_coils(length: usize) -> Vec<u16> {
    vec![0; length]
}

/// Generate pseudo-random register data (full u16 values).
pub fn generate_random_registers(length: usize) -> Vec<u16> {
    (0..length).map(|_| rand::random::<u16>()).collect()
}
