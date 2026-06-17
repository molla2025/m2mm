#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tauri::Manager;

mod converter;
mod utils;

use converter::{
    allocate_voices_by_instrument, allocate_voices_capped, extract_midi_notes, generate_mml_final,
    max_polyphony, split_bass_line, Note, TempoChange, GRID_SIZE, TPB,
};
use utils::mml::gm_family_name;
use std::collections::HashSet;

// 화음(합주) 모드 최대 보이스 수
const MAX_VOICES: usize = 6;
// 단독(혼자) 모드 보이스 수
const SOLO_VOICES: usize = 3;
// 2인 모드 보이스 수 (앞 3개 + 베이스 1개)
const DUO_VOICES: usize = 4;
// 악보 1칸 글자 수 기본값(현재 게임 한도). 게임사가 늘리면 이 값만 바꾸면 됨 (1200 → 2400 전례).
// ※ 프론트엔드 App.svelte 의 MML_CHAR_LIMIT 와 같은 값으로 유지할 것.
const DEFAULT_CHAR_LIMIT: usize = 2400;

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
    mode: String, // "solo"(혼자 3) / "duo"(2인 4) / "ensemble"(합주 6)
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

// 파일을 드롭했을 때 모드 추천에 쓰는 분석 결과
#[derive(Debug, Serialize, Deserialize)]
struct MidiAnalysis {
    total_notes: usize,
    instruments: usize,    // 비드럼 악기(program) 종류 수
    max_polyphony: usize,  // 최대 동시발음 수
}

#[derive(Debug, Serialize, Deserialize)]
struct AppSettings {
    char_limit: usize,
    mode: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            char_limit: DEFAULT_CHAR_LIMIT,
            mode: "solo".to_string(),
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
fn save_settings(app: tauri::AppHandle, char_limit: usize, mode: String) -> Result<(), String> {
    let settings = AppSettings { char_limit, mode };

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

// 변환 전에 파일을 분석해 모드 추천에 필요한 지표를 돌려준다.
#[tauri::command]
fn analyze_midi(midi_data: Vec<u8>) -> Result<MidiAnalysis, String> {
    let (notes, _bpm, _tempo) = extract_midi_notes(&midi_data)?;
    let instruments = notes.iter().map(|n| n.program).collect::<HashSet<u8>>().len();
    Ok(MidiAnalysis {
        total_notes: notes.len(),
        instruments,
        max_polyphony: max_polyphony(&notes),
    })
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

    let voices = convert_voices(notes, bpm, options.char_limit, &options.mode, &tempo_changes);

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

// 보이스의 대표 악기(노트 수 다수결). gap-fill 로 빌려온 음이 첫머리에 와도 흔들리지 않게,
// 첫 음이 아니라 다수결로 음색 이름을 정한다.
fn dominant_program(voice: &[Note]) -> u8 {
    let mut counts: HashMap<u8, usize> = HashMap::new();
    for n in voice {
        *counts.entry(n.program).or_default() += 1;
    }
    counts
        .into_iter()
        .max_by_key(|&(_, c)| c)
        .map(|(p, _)| p)
        .unwrap_or(0)
}

// 모드에 따라 보이스를 분배하고 이름을 붙인다.
// - solo(단독): 가장 중요한 3보이스. 멜로디 + 화음1, 화음2
// - duo(2인): 4보이스. 앞 3개(멜로디·화음) + 마지막 베이스 (1명이 앞 3개, 1명이 베이스)
// - ensemble(화음): 최대 6보이스를 악기 인지로 분배 (악기별 이름 / 단일이면 멜로디·화음)
fn convert_voices(
    notes: Vec<Note>,
    bpm: u32,
    char_limit: usize,
    mode: &str,
    tempo_changes: &[TempoChange],
) -> Vec<VoiceResult> {
    match mode {
        "duo" => {
            // 2인: 베이스 라인을 따로 떼어 1보이스, 나머지(멜로디·화음)는 3보이스 → 역할 분리
            let (bass, rest) = split_bass_line(notes);
            let mut voices = allocate_voices_capped(rest, DUO_VOICES - 1);
            voices.extend(allocate_voices_capped(bass, 1)); // 단음 베이스 라인
            name_by_role(voices, bpm, char_limit, tempo_changes, true)
        }
        "ensemble" => {
            let voices = allocate_voices_by_instrument(notes, MAX_VOICES);
            // 음색군(family) 기준으로 여러 음색이면 악기군 이름, 단일 음색이면 역할 이름
            let distinct: std::collections::HashSet<u8> = voices
                .iter()
                .filter(|v| !v.is_empty())
                .map(|v| dominant_program(v) / 8)
                .collect();
            if distinct.len() > 1 {
                name_by_instrument(voices, bpm, char_limit, tempo_changes)
            } else {
                name_by_role(voices, bpm, char_limit, tempo_changes, false)
            }
        }
        _ => {
            // solo
            let voices = allocate_voices_capped(notes, SOLO_VOICES);
            name_by_role(voices, bpm, char_limit, tempo_changes, false)
        }
    }
}

// 여러 악기: 악기군 이름 + 악기별 일련번호 (피아노1, 피아노2, 기타1 …)
// 같은 악기명 카드가 UI에서 붙어 보이도록, 악기명 첫 등장 순서로 묶어 정렬한 뒤 번호를 매긴다.
// (현악=GM5/6, 신스=GM10/11/12 처럼 다른 GM군이 같은 이름이 돼도 흩어지지 않게)
fn name_by_instrument(
    mut voices: Vec<Vec<Note>>,
    bpm: u32,
    char_limit: usize,
    tempo_changes: &[TempoChange],
) -> Vec<VoiceResult> {
    let mut group_order: HashMap<&str, usize> = HashMap::new();
    for v in &voices {
        if v.is_empty() {
            continue;
        }
        let name = gm_family_name(dominant_program(v));
        let next = group_order.len();
        group_order.entry(name).or_insert(next);
    }
    // 안정 정렬: 같은 악기명끼리 모이고, 그룹 순서는 첫 등장(중요도) 순 유지
    voices.sort_by_key(|v| {
        group_order
            .get(gm_family_name(dominant_program(v)))
            .copied()
            .unwrap_or(usize::MAX)
    });

    let mut family_idx: HashMap<&str, usize> = HashMap::new();
    build_voices_with_limit(voices, bpm, char_limit, tempo_changes, |_, final_voice| {
        let family = gm_family_name(dominant_program(final_voice));
        let c = family_idx.entry(family).or_insert(0);
        *c += 1;
        format!("{}{}", family, c)
    })
}

// 음 높은 순으로 멜로디 + 화음1, 화음2 …
// mark_bass=true 면 맨 마지막(최저음) 보이스를 "베이스"로 라벨 (2인 모드용)
fn name_by_role(
    mut voices: Vec<Vec<Note>>,
    bpm: u32,
    char_limit: usize,
    tempo_changes: &[TempoChange],
    mark_bass: bool,
) -> Vec<VoiceResult> {
    // 평균 음높이 높은 순 (멜로디가 맨 앞, 베이스가 맨 뒤)
    voices.sort_by_key(|b| std::cmp::Reverse(avg_pitch(b)));

    let last = voices.len().saturating_sub(1);
    let mut chord_count = 0;
    build_voices_with_limit(voices, bpm, char_limit, tempo_changes, |idx, _| {
        if idx == 0 {
            "멜로디".to_string()
        } else if mark_bass && idx == last {
            "베이스".to_string()
        } else {
            chord_count += 1;
            format!("화음{}", chord_count)
        }
    })
}

fn main() {
    tauri::Builder::default()
        // 앱은 하나만 실행. 또 실행하면 기존 창을 앞으로 가져온다. (반드시 첫 플러그인)
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .invoke_handler(tauri::generate_handler![
            analyze_midi,
            convert_midi,
            save_settings,
            load_settings
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
