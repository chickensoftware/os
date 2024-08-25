pub(in crate::base) mod rsd;
pub(in crate::base) mod sdt;
pub(in crate::base) mod madt;

use core::fmt;

#[derive(Copy, Clone)]
pub enum ACPIError {
    InvalidRSDAddress,
    InvalidXSDTAddress,
    TableNotFound([char; 4]),
    InvalidMemoryMap
}

impl fmt::Debug for ACPIError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ACPIError::InvalidRSDAddress => write!(f, "ACPI Parsing Error: Invalid RSD Address."),
            ACPIError::InvalidXSDTAddress => write!(f, "ACPI Parsing Error: Invalid XSDT Address."),
            ACPIError::InvalidMemoryMap => write!(f, "ACPI Parsing Error: Invalid Memory Map."),
            ACPIError::TableNotFound(signature) => {
                write!(f, "ACPI Parsing Error: Table not found: {:?}", signature)
            }
        }
    }
}