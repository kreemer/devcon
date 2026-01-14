// Re-export generated protobuf code
pub mod agent {
    include!(concat!(env!("OUT_DIR"), "/devcon.rs"));
}

pub use agent::*;
