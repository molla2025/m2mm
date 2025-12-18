#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

mod utils;
mod converter;

use converter::{
    allocate_voices_smart, generate_mml_final,
    crop_voice_to_limit, get_max_simultaneous_notes, Note, TPB,
};

#[derive(Debug, Serialize, Deserialize)]
struct ConversionOptions {
    mode: String, // "normal" or "instrument"
    char_limit: usize,
    min_note_duration: u32, // 최소 노트 길이 (ticks)
}

#[derive(Debug, Serialize, Deserialize)]
struct VoiceResult {
    name: String,
    content: String,
    char_count: usize,
    note_count: usize,
    duration: f64,
}

#[derive(Debug, Serialize, Deserialize)]
struct ConversionResult {
    success: bool,
    voices: Vec<VoiceResult>,
    error: Option<String>,
    bpm: u32,
    total_notes: usize,
}

#[tauri::command]
fn convert_midi(midi_data: Vec<u8>, options: ConversionOptions) -> ConversionResult {
    match convert_midi_internal(&midi_data, &options) {
        Ok(result) => result,
        Err(e) => ConversionResult {
            success: false,
            voices: vec![],
            error: Some(e),
            bpm: 0,
            total_notes: 0,
        },
    }
}

fn convert_midi_internal(
    midi_data: &[u8],
    options: &ConversionOptions,
) -> Result<ConversionResult, String> {
    let (notes, bpm) = converter::extract_midi_notes(midi_data, options.min_note_duration)?;
    let total_notes = notes.len();

    let voices = if options.mode == "instrument" {
        // 악기별 모드
        convert_by_instrument(notes, bpm, options.char_limit)?
    } else {
        // 일반 모드 (피치별)
        convert_by_pitch(notes, bpm, options.char_limit)?
    };

    Ok(ConversionResult {
        success: true,
        voices,
        error: None,
        bpm,
        total_notes,
    })
}

fn convert_by_pitch(
    notes: Vec<Note>,
    bpm: u32,
    char_limit: usize,
) -> Result<Vec<VoiceResult>, String> {
    let voices = allocate_voices_smart(notes);
    let mut results = Vec::new();

    for (idx, voice) in voices.iter().enumerate() {
        if voice.is_empty() {
            continue;
        }

        let cropped = crop_voice_to_limit(voice, bpm, char_limit);
        if cropped.is_empty() {
            continue;
        }

        let first_note = cropped[0].note;
        let mut start_octave = (first_note as i32 / 12) - 1;
        start_octave = start_octave.max(2).min(6);

        let mml_code = generate_mml_final(&cropped, bpm, start_octave);
        let note_count = cropped.len();

        let end_time = if let Some(last) = cropped.last() {
            last.end as f64 / TPB as f64 / 2.0
        } else {
            0.0
        };

        let name = if idx == 0 {
            "멜로디".to_string()
        } else {
            format!("화음{}", idx)
        };

        results.push(VoiceResult {
            name,
            content: mml_code.clone(),
            char_count: mml_code.len(),
            note_count,
            duration: end_time,
        });
    }

    Ok(results)
}

fn convert_by_instrument(
    notes: Vec<Note>,
    bpm: u32,
    char_limit: usize,
) -> Result<Vec<VoiceResult>, String> {
    let mut instrument_groups: HashMap<String, Vec<Note>> = HashMap::new();
    for note in notes {
        instrument_groups
            .entry(note.instrument.clone())
            .or_insert_with(Vec::new)
            .push(note);
    }

    let mut results = Vec::new();
    let mut instrument_names: Vec<String> = instrument_groups.keys().cloned().collect();
    instrument_names.sort();

    let mut global_voice_idx = 0;

    for instrument_name in instrument_names {
        let instrument_notes = instrument_groups.remove(&instrument_name).unwrap();
        let voices = allocate_voices_smart(instrument_notes);

        if voices.is_empty() {
            continue;
        }

        for (idx, voice) in voices.iter().enumerate() {
            if voice.is_empty() {
                continue;
            }

            let cropped = crop_voice_to_limit(voice, bpm, char_limit);
            if cropped.is_empty() {
                continue;
            }

            let first_note = cropped[0].note;
            let mut start_octave = (first_note as i32 / 12) - 1;
            start_octave = start_octave.max(2).min(6);

            let mml_code = generate_mml_final(&cropped, bpm, start_octave);
            let note_count = cropped.len();

            let end_time = if let Some(last) = cropped.last() {
                last.end as f64 / TPB as f64 / 2.0
            } else {
                0.0
            };

            let name = if global_voice_idx == 0 {
                format!("멜로디 ({})", instrument_name)
            } else if voices.len() == 1 {
                format!("화음{} ({})", global_voice_idx, instrument_name)
            } else {
                format!("화음{} ({}-{})", global_voice_idx, instrument_name, idx + 1)
            };

            results.push(VoiceResult {
                name,
                content: mml_code.clone(),
                char_count: mml_code.len(),
                note_count,
                duration: end_time,
            });

            global_voice_idx += 1;
        }
    }

    Ok(results)
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![convert_midi])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}