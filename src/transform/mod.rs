//! Transform implementations for Kibana saved objects
//!
//! This module provides concrete transformer implementations that replace
//! the functionality previously provided by the jsrmx crate.

mod field_dropper;
mod field_escaper;
mod managed_flag;

pub use field_dropper::FieldDropper;
pub use field_escaper::{FieldEscaper, FieldUnescaper};
pub use managed_flag::ManagedFlagAdder;
