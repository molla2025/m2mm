#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tauri::Manager;

mod converter;
mod utils;
mod analyzer;

use converter::{allocate_voices_smart, extract_midi_notes, generate_mml_final, Note, TempoChange, TPB};

// 틱을 실제 시간(초)으로 변환
fn ticks_to_seconds(ticks: u32, bpm: u32) -> f64 {
    // ticks / TPB = quarter notes (박자 수)
    // quarter notes / BPM * 60 = 초
    let quarter_notes = ticks as f64 / TPB as f64;
    let seconds = quarter_notes / bpm as f64 * 60.0;
    seconds
}

#[derive(Debug, Serialize, Deserialize)]
struct ConversionOptions {
    mode: String, // "normal" or "instrument"
    char_limit: usize,
    compress_mode: bool, // true: 글자수 우선 (점음표/타이 최소화), false: 정확도 우선
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
    original_duration: f64,
}

#[derive(Debug, Serialize, Deserialize)]
struct AppSettings {
    conversion_mode: String,
    char_limit: usize,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            conversion_mode: "normal".to_string(),
            char_limit: 2400,
        }
    }
}

fn get_settings_path(app: tauri::AppHandle) -> Result<PathBuf, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;

    fs::create_dir_all(&app_data_dir)
        .map_err(|e| format!("Failed to create app data dir: {}", e))?;

    Ok(app_data_dir.join("settings.json"))
}

#[tauri::command]
fn save_settings(app: tauri::AppHandle, mode: String, char_limit: usize) -> Result<(), String> {
    let settings = AppSettings {
        conversion_mode: mode,
        char_limit,
    };

    let settings_path = get_settings_path(app)?;
    let json = serde_json::to_string_pretty(&settings)
        .map_err(|e| format!("Failed to serialize settings: {}", e))?;

    fs::write(settings_path, json).map_err(|e| format!("Failed to write settings: {}", e))?;

    Ok(())
}

#[tauri::command]
fn load_settings(app: tauri::AppHandle) -> Result<AppSettings, String> {
    let settings_path = get_settings_path(app)?;

    if !settings_path.exists() {
        return Ok(AppSettings::default());
    }

    let json =
        fs::read_to_string(settings_path).map_err(|e| format!("Failed to read settings: {}", e))?;

    let settings: AppSettings =
        serde_json::from_str(&json).map_err(|e| format!("Failed to parse settings: {}", e))?;

    Ok(settings)
}

#[tauri::command]
fn analyze_test_midi(midi_data: Vec<u8>) -> Result<String, String> {
    let analysis = analyzer::analyze_midi(&midi_data)?;
    Ok(analyzer::print_analysis(&analysis))
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
            original_duration: 0.0,
        },
    }
}

fn convert_midi_internal(
    midi_data: &[u8],
    options: &ConversionOptions,
) -> Result<ConversionResult, String> {
    let (notes, bpm, tempo_changes) = extract_midi_notes(midi_data, 24)?;
    let total_notes = notes.len();

    // 원본 길이 계산
    let original_duration = if notes.is_empty() {
        0.0
    } else {
        let max_end = notes.iter().map(|n| n.end).max().unwrap_or(0);
        ticks_to_seconds(max_end, bpm)
    };

    let voices = if options.mode == "instrument" {
        // 악기별 모드
        convert_by_instrument(notes, bpm, options.char_limit, options.compress_mode, &tempo_changes)?
    } else {
        // 일반 모드 (피치별)
        convert_by_pitch(notes, bpm, options.char_limit, options.compress_mode, &tempo_changes)?
    };

    Ok(ConversionResult {
        success: true,
        voices,
        error: None,
        bpm,
        total_notes,
        original_duration,
    })
}

fn convert_by_pitch(
    notes: Vec<Note>,
    bpm: u32,
    char_limit: usize,
    compress_mode: bool,
    tempo_changes: &[TempoChange],
) -> Result<Vec<VoiceResult>, String> {
    let voices = allocate_voices_smart(notes);

    // 빈 voice 제거
    let voices: Vec<Vec<Note>> = voices.into_iter().filter(|v| !v.is_empty()).collect();

    if voices.is_empty() {
        return Ok(Vec::new());
    }

    // 최대 end_time 찾기
    let max_end_time = voices
        .iter()
        .flat_map(|v| v.iter())
        .map(|n| n.end)
        .max()
        .unwrap_or(0);

    if max_end_time == 0 {
        return Ok(Vec::new());
    }

    // 먼저 전체 길이가 char_limit을 만족하는지 체크
    let mut full_length_valid = true;
    for voice in voices.iter() {
        let first_note = voice[0].note;
        let mut start_octave = (first_note as i32 / 12) - 1;
        start_octave = start_octave.max(2).min(6);

        let mml = generate_mml_final(&voice, bpm, start_octave, compress_mode, tempo_changes);

        if mml.len() > char_limit {
            full_length_valid = false;
            break;
        }
    }

    // 전체 길이가 OK면 그대로 사용
    let best_end_time = if full_length_valid {
        max_end_time
    } else {
        // 이진 탐색으로 모든 voice가 char_limit 이하인 최대 end_time 찾기
        let grid_size = 24u32;
        let mut left = 0u32;
        let mut right = max_end_time;
        let mut best = 0u32;

        while left <= right {
            let mid = ((left + right) / 2 / grid_size) * grid_size;
            if mid == 0 {
                break;
            }

            let mut all_valid = true;

            // 각 voice를 mid 시간까지 크롭해서 char_limit 체크
            for voice in voices.iter() {
                let cropped: Vec<Note> = voice.iter().filter(|n| n.start < mid).cloned().collect();

                if cropped.is_empty() {
                    continue;
                }

                let first_note = cropped[0].note;
                let mut start_octave = (first_note as i32 / 12) - 1;
                start_octave = start_octave.max(2).min(6);

                let mml = generate_mml_final(&cropped, bpm, start_octave, compress_mode, tempo_changes);

                if mml.len() > char_limit {
                    all_valid = false;
                    break;
                }
            }

            if all_valid {
                best = mid;
                left = mid + grid_size;
            } else {
                right = mid.saturating_sub(grid_size);
            }
        }

        best
    };

    // best_end_time으로 모든 voice 최종 크롭
    let mut results = Vec::new();
    for (idx, voice) in voices.iter().enumerate() {
        let final_voice: Vec<Note> = voice
            .iter()
            .filter(|n| n.start < best_end_time)
            .cloned()
            .collect();

        if final_voice.is_empty() {
            continue;
        }

        let first_note = final_voice[0].note;
        let mut start_octave = (first_note as i32 / 12) - 1;
        start_octave = start_octave.max(2).min(6);

        let mml_code = generate_mml_final(&final_voice, bpm, start_octave, compress_mode, tempo_changes);
        let note_count = final_voice.len();
        let end_time = ticks_to_seconds(best_end_time, bpm);

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
    compress_mode: bool,
    tempo_changes: &[TempoChange],
) -> Result<Vec<VoiceResult>, String> {
    let mut instrument_groups: HashMap<String, Vec<Note>> = HashMap::new();
    for note in notes {
        instrument_groups
            .entry(note.instrument.clone())
            .or_insert_with(Vec::new)
            .push(note);
    }

    let mut instrument_names: Vec<String> = instrument_groups.keys().cloned().collect();
    instrument_names.sort();

    // 모든 악기의 voice 수집
    let mut all_voices = Vec::new();
    let mut voice_instrument_map = Vec::new();

    for instrument_name in &instrument_names {
        let instrument_notes = instrument_groups.get(instrument_name).unwrap();
        let voices = allocate_voices_smart(instrument_notes.clone());

        for voice in voices.into_iter() {
            if !voice.is_empty() {
                all_voices.push(voice);
                voice_instrument_map.push(instrument_name.clone());
            }
        }
    }

    if all_voices.is_empty() {
        return Ok(Vec::new());
    }

    // 최대 end_time 찾기
    let max_end_time = all_voices
        .iter()
        .flat_map(|v| v.iter())
        .map(|n| n.end)
        .max()
        .unwrap_or(0);

    if max_end_time == 0 {
        return Ok(Vec::new());
    }

    // 먼저 전체 길이가 char_limit을 만족하는지 체크
    let mut full_length_valid = true;
    for voice in all_voices.iter() {
        let first_note = voice[0].note;
        let mut start_octave = (first_note as i32 / 12) - 1;
        start_octave = start_octave.max(2).min(6);

        let mml = generate_mml_final(&voice, bpm, start_octave, compress_mode, tempo_changes);

        if mml.len() > char_limit {
            full_length_valid = false;
            break;
        }
    }

    // 전체 길이가 OK면 그대로 사용
    let best_end_time = if full_length_valid {
        max_end_time
    } else {
        // 이진 탐색으로 모든 voice가 char_limit 이하인 최대 end_time 찾기
        let grid_size = 24u32;
        let mut left = 0u32;
        let mut right = max_end_time;
        let mut best = 0u32;

        while left <= right {
            let mid = ((left + right) / 2 / grid_size) * grid_size;
            if mid == 0 {
                break;
            }

            let mut all_valid = true;

            // 각 voice를 mid 시간까지 크롭해서 char_limit 체크
            for voice in all_voices.iter() {
                let cropped: Vec<Note> = voice.iter().filter(|n| n.start < mid).cloned().collect();

                if cropped.is_empty() {
                    continue;
                }

                let first_note = cropped[0].note;
                let mut start_octave = (first_note as i32 / 12) - 1;
                start_octave = start_octave.max(2).min(6);

                let mml = generate_mml_final(&cropped, bpm, start_octave, compress_mode, tempo_changes);

                if mml.len() > char_limit {
                    all_valid = false;
                    break;
                }
            }

            if all_valid {
                best = mid;
                left = mid + grid_size;
            } else {
                right = mid.saturating_sub(grid_size);
            }
        }

        best
    };

    // best_end_time으로 모든 voice 최종 크롭
    let mut results = Vec::new();
    for (idx, (voice, instrument_name)) in all_voices
        .iter()
        .zip(voice_instrument_map.iter())
        .enumerate()
    {
        let final_voice: Vec<Note> = voice
            .iter()
            .filter(|n| n.start < best_end_time)
            .cloned()
            .collect();

        if final_voice.is_empty() {
            continue;
        }

        let first_note = final_voice[0].note;
        let mut start_octave = (first_note as i32 / 12) - 1;
        start_octave = start_octave.max(2).min(6);

        let mml_code = generate_mml_final(&final_voice, bpm, start_octave, compress_mode, tempo_changes);
        let note_count = final_voice.len();
        let end_time = ticks_to_seconds(best_end_time, bpm);

        let name = if idx == 0 {
            format!("멜로디 ({})", instrument_name)
        } else {
            format!("화음{} ({})", idx, instrument_name)
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

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            convert_midi,
            save_settings,
            load_settings,
            analyze_test_midi
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
