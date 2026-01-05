// Re-export modules for library usage
pub mod utils;
pub mod converter;
pub mod analyzer;

pub use converter::{
    extract_midi_notes, allocate_voices_smart, generate_mml_final,
    Note, TPB, GRID_SIZE,
};