//! cn105-proto: Mitsubishi CN105 HVAC serial protocol implementation in Rust.

pub mod builder;
pub mod checksum;
pub mod parser;
pub mod temperature;
pub mod types;

pub use builder::*;
pub use checksum::{calculate_checksum, verify_checksum};
pub use parser::{Cn105Event, FrameParser, ParseError};
pub use temperature::*;
pub use types::*;
