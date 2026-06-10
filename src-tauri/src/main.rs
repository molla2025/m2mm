#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tauri::Manager;

mod converter;
mod utils;

use converter::{
    allocate_voices_by_instrument, extract_midi_notes, generate_mml_final, Note, TempoChange,
    GRID_SIZE, TPB,
};
use utils::mml::gm_family_name;

// 모비노기 모바일 한 악보의 최대 보이스 수
const MAX_VOICES: usize = 6;

// 틱을 실제 시간(초)으로 변환
fn ticks_to_seconds(ticks: u32, bpm: u32) -> f64 {
    // ticks / TPB = quarter notes (박자 수)
    // quarter notes / BPM * 60 = 초
    let quarter_notes = ticks as f64 / TPB as f64;
    quarter_notes / bpm as f64 * 60.0
}

// 첫 노트 기준 시작 옥타브 (모비노기 MML 범위 O2~O6으로 클램프)
fn start_octave_for(first_note: u8) -> i32 {
    let octave = (first_note as i32 / 12) - 1;
    octave.clamp(2, 6)
}

#[derive(Debug, Serialize, Deserialize)]
struct ConversionOptions {
    char_limit: usize,
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
    char_limit: usize,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self { char_limit: 2400 }
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
fn save_settings(app: tauri::AppHandle, char_limit: usize) -> Result<(), String> {
    let settings = AppSettings { char_limit };

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
    let (notes, bpm, tempo_changes) = extract_midi_notes(midi_data)?;
    let total_notes = notes.len();

    // 원본 길이 계산
    let original_duration = if notes.is_empty() {
        0.0
    } else {
        let max_end = notes.iter().map(|n| n.end).max().unwrap_or(0);
        ticks_to_seconds(max_end, bpm)
    };

    let voices = convert_voices(notes, bpm, options.char_limit, &tempo_changes);

    Ok(ConversionResult {
        success: true,
        voices,
        error: None,
        bpm,
        total_notes,
        original_duration,
    })
}

/// 보이스 목록을 받아 글자수 제한(char_limit)에 맞게 곡 끝을 잘라낸 뒤,
/// 각 보이스를 MML 문자열로 변환한다.
///
/// `namer`는 (보이스 인덱스, 최종 노트들) -> 파트 이름 을 결정한다.
fn build_voices_with_limit<F>(
    voices: Vec<Vec<Note>>,
    bpm: u32,
    char_limit: usize,
    tempo_changes: &[TempoChange],
    mut namer: F,
) -> Vec<VoiceResult>
where
    F: FnMut(usize, &[Note]) -> String,
{
    // 빈 voice 제거
    let voices: Vec<Vec<Note>> = voices.into_iter().filter(|v| !v.is_empty()).collect();
    if voices.is_empty() {
        return Vec::new();
    }

    let max_end_time = voices
        .iter()
        .flat_map(|v| v.iter())
        .map(|n| n.end)
        .max()
        .unwrap_or(0);
    if max_end_time == 0 {
        return Vec::new();
    }

    // 주어진 종료 시점까지 크롭한 모든 voice가 char_limit 이하인지 검사
    let all_within_limit = |end_time: u32| {
        voices.iter().all(|voice| {
            let cropped: Vec<Note> = voice.iter().filter(|n| n.start < end_time).cloned().collect();
            if cropped.is_empty() {
                return true;
            }
            let start_octave = start_octave_for(cropped[0].note);
            generate_mml_final(&cropped, bpm, start_octave, tempo_changes).len() <= char_limit
        })
    };

    // 전체 길이가 OK면 그대로, 아니면 이진 탐색으로 최대 종료 시점 찾기
    let best_end_time = if all_within_limit(max_end_time) {
        max_end_time
    } else {
        let mut left = 0u32;
        let mut right = max_end_time;
        let mut best = 0u32;

        while left <= right {
            let mid = ((left + right) / 2 / GRID_SIZE) * GRID_SIZE;
            if mid == 0 {
                break;
            }

            if all_within_limit(mid) {
                best = mid;
                left = mid + GRID_SIZE;
            } else {
                right = mid.saturating_sub(GRID_SIZE);
            }
        }

        best
    };

    // best_end_time으로 모든 voice 최종 크롭 및 MML 생성
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

        let start_octave = start_octave_for(final_voice[0].note);
        let mml_code = generate_mml_final(&final_voice, bpm, start_octave, tempo_changes);
        let actual_end = final_voice.iter().map(|n| n.end).max().unwrap_or(0);

        results.push(VoiceResult {
            name: namer(idx, &final_voice),
            char_count: mml_code.len(),
            note_count: final_voice.len(),
            duration: ticks_to_seconds(actual_end, bpm),
            content: mml_code,
        });
    }

    results
}

// 보이스 평균 음높이
fn avg_pitch(voice: &[Note]) -> u8 {
    if voice.is_empty() {
        return 0;
    }
    (voice.iter().map(|n| n.note as u32).sum::<u32>() / voice.len() as u32) as u8
}

// 최대 6보이스를 악기 인지로 분배하고 이름을 붙인다.
// - 여러 악기가 섞이면: 악기군 이름 + 번호 (피아노1, 피아노2, 기타1, 플룻1 …), 악기 중요도 순
// - 단일 악기면: 멜로디 + 화음1, 화음2 … (음 높은 순)
fn convert_voices(
    notes: Vec<Note>,
    bpm: u32,
    char_limit: usize,
    tempo_changes: &[TempoChange],
) -> Vec<VoiceResult> {
    let voices = allocate_voices_by_instrument(notes, MAX_VOICES);

    let distinct_instruments: std::collections::HashSet<u8> =
        voices.iter().filter_map(|v| v.first().map(|n| n.program)).collect();

    if distinct_instruments.len() > 1 {
        name_by_instrument(voices, bpm, char_limit, tempo_changes)
    } else {
        name_by_role(voices, bpm, char_limit, tempo_changes)
    }
}

// 여러 악기: 악기군 이름 + 악기별 일련번호 (피아노1, 피아노2, 기타1 …)
fn name_by_instrument(
    voices: Vec<Vec<Note>>,
    bpm: u32,
    char_limit: usize,
    tempo_changes: &[TempoChange],
) -> Vec<VoiceResult> {
    let mut family_idx: HashMap<&str, usize> = HashMap::new();
    build_voices_with_limit(voices, bpm, char_limit, tempo_changes, |_, final_voice| {
        let family = gm_family_name(final_voice[0].program);
        let c = family_idx.entry(family).or_insert(0);
        *c += 1;
        format!("{}{}", family, c)
    })
}

// 단일 악기: 음 높은 순으로 멜로디 + 화음1, 화음2 …
fn name_by_role(
    mut voices: Vec<Vec<Note>>,
    bpm: u32,
    char_limit: usize,
    tempo_changes: &[TempoChange],
) -> Vec<VoiceResult> {
    // 평균 음높이 높은 순 (멜로디가 맨 앞)
    voices.sort_by(|a, b| avg_pitch(b).cmp(&avg_pitch(a)));

    let mut chord_count = 0;
    build_voices_with_limit(voices, bpm, char_limit, tempo_changes, |idx, _| {
        if idx == 0 {
            "멜로디".to_string()
        } else {
            chord_count += 1;
            format!("화음{}", chord_count)
        }
    })
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .invoke_handler(tauri::generate_handler![
            convert_midi,
            save_settings,
            load_settings
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
