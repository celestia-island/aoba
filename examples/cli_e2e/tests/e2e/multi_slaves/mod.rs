// Multi-slaves test modules
pub mod adjacent_registers;
pub mod basic;
pub mod same_station;

pub use adjacent_registers::test_multi_slaves_adjacent_registers;
pub use basic::test_multi_slaves;
pub use same_station::test_multi_slaves_same_station;
