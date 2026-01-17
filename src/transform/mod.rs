//! Transform implementations for Kibana saved objects
//!
//! This module provides concrete transformer implementations that replace
//! the functionality previously provided by the jsrmx crate.

mod field_dropper;
mod field_escaper;
mod managed_flag;
mod multiline_fields;
mod vega_spec;
mod yaml_formatter;

#[cfg(test)]
mod integration_test;

pub use field_dropper::FieldDropper;
pub use field_escaper::{FieldEscaper, FieldUnescaper};
pub use managed_flag::ManagedFlagAdder;
pub use multiline_fields::MultilineFieldFormatter;
pub use vega_spec::{VegaSpecEscaper, VegaSpecUnescaper};
pub use yaml_formatter::YamlFormatter;
