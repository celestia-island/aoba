use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryRole {
    Master,
    Slave,
}

impl fmt::Display for EntryRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EntryRole::Master => write!(f, "Master"),
            EntryRole::Slave => write!(f, "Slave"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterMode {
    Coils = 1,
    DiscreteInputs = 2,
    Holding = 3,
    Input = 4,
}

impl RegisterMode {
    pub const fn all() -> &'static [RegisterMode] {
        &[
            Self::Coils,
            Self::DiscreteInputs,
            Self::Holding,
            Self::Input,
        ]
    }

    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Coils,
            2 => Self::DiscreteInputs,
            3 => Self::Holding,
            4 => Self::Input,
            _ => Self::Coils,
        }
    }
}
