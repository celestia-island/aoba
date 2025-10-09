/// Generate pseudo-random coil data (0 or 1 values).
pub fn generate_random_coils(length: usize) -> Vec<u16> {
    (0..length)
        .map(|_| if rand::random::<u8>().is_multiple_of(2) { 0 } else { 1 })
        .collect()
}

/// Generate pseudo-random register data (full u16 values).
pub fn generate_random_registers(length: usize) -> Vec<u16> {
    (0..length).map(|_| rand::random::<u16>()).collect()
}
