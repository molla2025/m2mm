use std::collections::HashMap;

use crate::utils::instrument::get_instrument_name;
use crate::utils::mml::midi_to_note_name;

// 상수 - 표준 TPB (4분음표당 틱 수)
pub const TPB: u32 = 384;
pub const GRID_SIZE: u32 = 24;

#[derive(Debug, Clone)]
pub struct Note {
    pub note: u8,
    pub start: u32,
    pub end: u32,
    pub duration: u32,
    pub velocity: u8,
    pub instrument: String,
}

#[derive(Debug, Clone)]
pub struct TempoChange {
    pub tick: u32,
    pub bpm: u32,
}

// 점음표 포함 정확한 길이 매핑
fn get_exact_lengths(compress_mode: bool) -> HashMap<u32, &'static str> {
    let mut map = HashMap::new();
    
    if compress_mode {
        // 압축 모드: 점음표 제거, 기본 음표만
        map.insert(1536, "1");
        map.insert(768, "2");
        map.insert(384, "4");
        map.insert(192, "8");
        map.insert(96, "16");
        map.insert(48, "32");
        map.insert(24, "64");
    } else {
        // 정확도 모드: 점음표 포함
        map.insert(2304, "1.");
        map.insert(1536, "1");
        map.insert(1152, "2.");
        map.insert(768, "2");
        map.insert(576, "4.");
        map.insert(384, "4");
        map.insert(288, "8.");
        map.insert(192, "8");
        map.insert(144, "16.");
        map.insert(96, "16");
        map.insert(72, "32.");
        map.insert(48, "32");
        map.insert(36, "64.");
        map.insert(24, "64");
    }
    
    map
}

fn snap_to_grid(tick: u32) -> u32 {
    ((tick as f32 / GRID_SIZE as f32).round() as u32) * GRID_SIZE
}

// 정확히 매칭되는 길이 찾기 (점음표 포함)
fn find_exact_match(ticks: u32, exact_lengths: &HashMap<u32, &str>) -> Option<Vec<(String, u32)>> {
    exact_lengths.get(&ticks).map(|&s| vec![(s.to_string(), ticks)])
}

// 타이 조합 찾기
fn find_tie_combination(
    ticks: u32,
    max_ties: Option<usize>,
    exact_lengths: &HashMap<u32, &str>,
) -> Vec<(String, u32)> {
    let mut result = Vec::new();
    let mut remaining = ticks;
    let mut tie_count = 0;

    let mut lengths: Vec<u32> = exact_lengths.keys().copied().collect();
    lengths.sort_by(|a, b| b.cmp(a));

    for length_ticks in lengths {
        if let Some(max) = max_ties {
            if tie_count >= max {
                break;
            }
        }

        while remaining >= length_ticks {
            if let Some(&length_str) = exact_lengths.get(&length_ticks) {
                result.push((length_str.to_string(), length_ticks));
                remaining -= length_ticks;
                tie_count += 1;

                if let Some(max) = max_ties {
                    if tie_count >= max {
                        break;
                    }
                }
            } else {
                break;
            }
        }
    }

    if result.is_empty() {
        result.push(("16".to_string(), 96));
    }

    result
}

// 안전한 근사치 찾기 (타이 없이)
fn find_safe_approximation(ticks: u32, exact_lengths: &HashMap<u32, &str>) -> Vec<(String, u32)> {
    let closest = exact_lengths
        .keys()
        .min_by_key(|&&x| ((x as i64) - (ticks as i64)).abs())
        .copied()
        .unwrap_or(96);

    if let Some(&length_str) = exact_lengths.get(&closest) {
        vec![(length_str.to_string(), closest)]
    } else {
        vec![("16".to_string(), 96)]
    }
}

// 옥타브별 최적 길이 찾기 (음 씹힘 방지 우선)
fn find_best_length(ticks: u32, octave: i32, exact_lengths: &HashMap<u32, &str>, compress_mode: bool) -> Vec<(String, u32)> {
    // 1. 정확한 매칭 (점음표 포함)
    if let Some(exact) = find_exact_match(ticks, exact_lengths) {
        return exact;
    }

    // 압축 모드: 타이 최소화, 근사치 우선
    if compress_mode {
        return find_safe_approximation(ticks, exact_lengths);
    }

    // 2. 옥타브별 전략 (음 씹힘 방지)
    if octave <= 4 {
        // 중저음: 타이 자유롭게
        find_tie_combination(ticks, None, exact_lengths)
    } else if octave == 5 {
        // 고음: 타이 2개까지만 (음 씹힘 방지)
        let ties = find_tie_combination(ticks, Some(2), exact_lengths);
        if ties.len() <= 2 {
            ties
        } else {
            find_safe_approximation(ticks, exact_lengths)
        }
    } else {
        // 초고음: 근사치만
        find_safe_approximation(ticks, exact_lengths)
    }
}

pub fn extract_midi_notes(midi_data: &[u8], _min_duration: u32) -> Result<(Vec<Note>, u32, Vec<TempoChange>), String> {
    let smf = midly::Smf::parse(midi_data).map_err(|e| format!("MIDI 파싱 오류: {}", e))?;

    let tpb = match smf.header.timing {
        midly::Timing::Metrical(t) => t.as_int() as u32,
        _ => return Err("SMPTE 타이밍 지원하지 않음".to_string()),
    };

    // 모든 템포 변경 이벤트 추출
    let mut tempo_changes = Vec::new();
    for track in &smf.tracks {
        let mut tick = 0u32;
        for event in track {
            tick += event.delta.as_int();
            if let midly::TrackEventKind::Meta(midly::MetaMessage::Tempo(tempo)) = event.kind {
                let bpm = (60_000_000.0 / tempo.as_int() as f64).round() as u32;
                tempo_changes.push((tick, bpm));
            }
        }
    }

    // 템포 변경을 tick 순으로 정렬하고 중복 제거
    tempo_changes.sort_by_key(|&(tick, _)| tick);
    tempo_changes.dedup_by_key(|&mut (tick, _)| tick);

    // BPM - 첫 번째 템포 또는 기본값
    let bpm = tempo_changes.first().map(|&(_, bpm)| bpm).unwrap_or(120);

    // TPB 변환 비율 계산
    let tpb_ratio = TPB as f64 / tpb as f64;

    // 템포 변경을 변환된 tick으로 스냅
    let tempo_changes_converted: Vec<TempoChange> = tempo_changes
        .into_iter()
        .map(|(tick, bpm)| {
            let tick_converted = (tick as f64 * tpb_ratio).round() as u32;
            let tick_snapped = snap_to_grid(tick_converted);
            TempoChange {
                tick: tick_snapped,
                bpm,
            }
        })
        .collect();

    // 음표 추출
    let mut notes = Vec::new();
    for track in &smf.tracks {
        let mut channel_programs: HashMap<u8, u8> = HashMap::new();
        let mut active: HashMap<(u8, u8), (u32, u8, u8)> = HashMap::new();
        let mut tick = 0u32;

        for event in track {
            tick += event.delta.as_int();

            match event.kind {
                midly::TrackEventKind::Midi { channel, message } => {
                    let ch = channel.as_int();

                    match message {
                        midly::MidiMessage::ProgramChange { program } => {
                            channel_programs.insert(ch, program.as_int());
                        }
                        midly::MidiMessage::NoteOn { key, vel } => {
                            let note_num = key.as_int();
                            let velocity = vel.as_int();

                            if velocity > 0 && note_num <= 127 && ch != 9 {
                                let key = (ch, note_num);
                                active.insert(key, (tick, velocity, ch));
                            } else if velocity == 0 && note_num <= 127 {
                                let key = (ch, note_num);
                                if let Some((start, velocity, channel)) = active.remove(&key) {
                                    let duration = tick.saturating_sub(start);

                                    // TPB 변환 - 먼저 변환 후 스냅
                                    let start_converted = (start as f64 * tpb_ratio).round() as u32;
                                    let duration_converted = (duration as f64 * tpb_ratio).round() as u32;

                                    let start_snapped = snap_to_grid(start_converted);
                                    let end_converted = start_converted + duration_converted;
                                    let end_snapped = snap_to_grid(end_converted);
                                    
                                    let mut duration_snapped = end_snapped.saturating_sub(start_snapped);

                                    // 최소 길이 보장
                                    if duration_snapped < GRID_SIZE {
                                        duration_snapped = GRID_SIZE;
                                    }

                                    let program = channel_programs.get(&channel).copied().unwrap_or(0);
                                    let instrument = get_instrument_name(program);

                                    notes.push(Note {
                                        note: note_num,
                                        start: start_snapped,
                                        end: start_snapped + duration_snapped,
                                        duration: duration_snapped,
                                        velocity,
                                        instrument,
                                    });
                                }
                            }
                        }
                        midly::MidiMessage::NoteOff { key, .. } => {
                            let note_num = key.as_int();
                            if note_num <= 127 {
                                let key = (ch, note_num);
                                if let Some((start, velocity, channel)) = active.remove(&key) {
                                    let duration = tick.saturating_sub(start);

                                    // TPB 변환 - 먼저 변환 후 스냅
                                    let start_converted = (start as f64 * tpb_ratio).round() as u32;
                                    let duration_converted = (duration as f64 * tpb_ratio).round() as u32;

                                    let start_snapped = snap_to_grid(start_converted);
                                    let end_converted = start_converted + duration_converted;
                                    let end_snapped = snap_to_grid(end_converted);
                                    
                                    let mut duration_snapped = end_snapped.saturating_sub(start_snapped);

                                    // 최소 길이 보장
                                    if duration_snapped < GRID_SIZE {
                                        duration_snapped = GRID_SIZE;
                                    }

                                    let program = channel_programs.get(&channel).copied().unwrap_or(0);
                                    let instrument = get_instrument_name(program);

                                    notes.push(Note {
                                        note: note_num,
                                        start: start_snapped,
                                        end: start_snapped + duration_snapped,
                                        duration: duration_snapped,
                                        velocity,
                                        instrument,
                                    });
                                }
                            }
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }

    // 정렬 및 중복 제거
    notes.sort_by(|a, b| a.start.cmp(&b.start).then(b.note.cmp(&a.note)));

    let mut deduplicated = Vec::new();
    let mut i = 0;
    while i < notes.len() {
        let current = &notes[i];
        let mut duplicates = vec![current.clone()];
        let mut j = i + 1;

        while j < notes.len()
            && notes[j].start == current.start
            && notes[j].note == current.note
        {
            duplicates.push(notes[j].clone());
            j += 1;
        }

        let best = duplicates
            .into_iter()
            .max_by_key(|n| n.velocity)
            .unwrap();
        deduplicated.push(best);
        i = j;
    }

    Ok((deduplicated, bpm, tempo_changes_converted))
}

// 음역대별 역할 기반 Voice 할당 (화음 모드)
pub fn allocate_voices_by_range(notes: Vec<Note>) -> Vec<Vec<Note>> {
    // 음역대별로 노트 분류
    // 저음부 (O2-O3): note < 60 (C4) - 베이스
    // 중음부 (O3-O4): 60 <= note < 72 (C5) - 화음/반주
    // 고음부 (O5+): note >= 72 - 멜로디
    
    let mut bass_notes = Vec::new();
    let mut chord_notes = Vec::new();
    let mut melody_notes = Vec::new();
    
    for note in notes {
        if note.note < 60 {
            bass_notes.push(note);
        } else if note.note < 72 {
            chord_notes.push(note);
        } else {
            melody_notes.push(note);
        }
    }
    
    let mut voices: Vec<Vec<Note>> = Vec::new();
    
    // 1. 멜로디 파트 (고음부) - 스마트 할당
    if !melody_notes.is_empty() {
        let melody_voices = allocate_voices_smart(melody_notes);
        voices.extend(melody_voices);
    }
    
    // 2. 화음 파트 (중음부) - 스마트 할당
    if !chord_notes.is_empty() {
        let chord_voices = allocate_voices_smart(chord_notes);
        voices.extend(chord_voices);
    }
    
    // 3. 베이스 파트 (저음부) - 스마트 할당
    if !bass_notes.is_empty() {
        let bass_voices = allocate_voices_smart(bass_notes);
        voices.extend(bass_voices);
    }
    
    voices
}

pub fn allocate_voices_smart(notes: Vec<Note>) -> Vec<Vec<Note>> {
    let mut voices: Vec<Vec<Note>> = Vec::new();

    // 시작 시간별로 노트 그룹화
    let mut start_times: HashMap<u32, Vec<Note>> = HashMap::new();
    for note in notes {
        start_times.entry(note.start).or_insert_with(Vec::new).push(note);
    }

    let mut sorted_times: Vec<u32> = start_times.keys().copied().collect();
    sorted_times.sort();

    let mut last_melody_note: Option<u8> = None;

    for start_tick in sorted_times {
        let mut simultaneous = start_times.remove(&start_tick).unwrap();

        if simultaneous.len() == 1 {
            // 단일 노트 - 고음은 Voice 0 우선, 그 외는 사용 가능한 첫 번째 voice에 할당
            let note = simultaneous.into_iter().next().unwrap();
            let mut assigned = false;

            // 고음(Note 72 이상) 또는 이전 멜로디와 가까운 음이면 Voice 0에 우선 할당
            let is_high_note = note.note >= 72;
            let is_close_to_melody = if let Some(last_note) = last_melody_note {
                (note.note as i32 - last_note as i32).abs() <= 5
            } else {
                false
            };

            // Voice 0에 우선 할당해야 하는 경우
            if is_high_note || is_close_to_melody {
                if voices.is_empty() {
                    voices.push(Vec::new());
                }
                
                if voices[0].is_empty() || voices[0].last().unwrap().end <= note.start {
                    voices[0].push(note.clone());
                    last_melody_note = Some(note.note);
                    assigned = true;
                } else {
                    let last_v0_note = voices[0].last().unwrap();
                    if is_high_note && last_v0_note.note < note.note {
                        if let Some(last_note_mut) = voices[0].last_mut() {
                            last_note_mut.end = note.start;
                            last_note_mut.duration = note.start.saturating_sub(last_note_mut.start);
                        }
                        voices[0].push(note.clone());
                        last_melody_note = Some(note.note);
                        assigned = true;
                    }
                }
            }
            
            if !assigned {
                // 사용 가능한 voice 찾기 또는 새로 생성
                let mut found = false;
                for i in 0..voices.len() {
                    if voices[i].is_empty() || voices[i].last().unwrap().end <= note.start {
                        voices[i].push(note.clone());
                        if i == 0 {
                            last_melody_note = Some(note.note);
                        }
                        found = true;
                        break;
                    }
                }
                
                if !found {
                    // 새 voice 생성
                    let mut new_voice = Vec::new();
                    new_voice.push(note.clone());
                    voices.push(new_voice);
                }
            }
        } else {
            // 다성 화음 - 높은 음부터 정렬
            simultaneous.sort_by(|a, b| b.note.cmp(&a.note));

            // 멜로디 선택: 가장 높은 음을 기본으로, 이전 음과의 연속성 고려
            let melody_idx = if let Some(last_note) = last_melody_note {
                let mut candidates_within_range = Vec::new();
                for (idx, note) in simultaneous.iter().enumerate() {
                    let distance = (note.note as i32 - last_note as i32).abs();
                    if distance <= 5 {
                        candidates_within_range.push((idx, note.note, distance));
                    }
                }
                
                if !candidates_within_range.is_empty() {
                    candidates_within_range.sort_by(|a, b| b.1.cmp(&a.1));
                    candidates_within_range[0].0
                } else {
                    0
                }
            } else {
                0
            };

            let melody = simultaneous[melody_idx].clone();
            let bass = simultaneous.last().unwrap().clone();

            // 나머지 화음은 중간 음들 - 음높이 순서 유지
            let harmony_notes: Vec<Note> = simultaneous
                .iter()
                .filter(|n| n.note != melody.note && n.note != bass.note)
                .cloned()
                .collect();

            // 할당 우선순위: 멜로디 -> 베이스 -> 화음들 (높은 음부터)
            let mut priority_notes = vec![melody.clone()];
            if bass.note != melody.note {
                priority_notes.push(bass);
            }
            priority_notes.extend(harmony_notes);

            // 각 노트를 voice에 할당
            for (idx, note) in priority_notes.iter().enumerate() {
                let mut assigned = false;
                
                // 멜로디(첫 번째 노트)이고 고음인 경우 Voice 0 조기 종료 시도
                if idx == 0 && note.note >= 72 {
                    if voices.is_empty() {
                        voices.push(Vec::new());
                    }
                    
                    if !voices[0].is_empty() && voices[0].last().unwrap().end > note.start {
                        let last_v0_note = voices[0].last().unwrap();
                        if last_v0_note.note < note.note {
                            if let Some(last_note_mut) = voices[0].last_mut() {
                                last_note_mut.end = note.start;
                                last_note_mut.duration = note.start.saturating_sub(last_note_mut.start);
                            }
                            voices[0].push(note.clone());
                            last_melody_note = Some(note.note);
                            assigned = true;
                        }
                    } else if voices[0].is_empty() || voices[0].last().unwrap().end <= note.start {
                        voices[0].push(note.clone());
                        last_melody_note = Some(note.note);
                        assigned = true;
                    }
                }
                
                if !assigned {
                    // 사용 가능한 voice 찾기
                    for i in 0..voices.len() {
                        if voices[i].is_empty() || voices[i].last().unwrap().end <= note.start {
                            voices[i].push(note.clone());
                            if i == 0 {
                                last_melody_note = Some(note.note);
                            }
                            assigned = true;
                            break;
                        }
                    }
                }
                
                if !assigned {
                    // 새 voice 생성
                    let mut new_voice = Vec::new();
                    new_voice.push(note.clone());
                    voices.push(new_voice);
                }
            }
        }
    }

    voices
}

pub fn generate_mml_final(voice_notes: &[Note], bpm: u32, start_octave: i32, compress_mode: bool, tempo_changes: &[TempoChange]) -> String {
    if voice_notes.is_empty() {
        return String::new();
    }

    let exact_lengths = get_exact_lengths(compress_mode);
    let mut mml = Vec::new();

    // 헤더
    mml.push(format!("T{}", bpm));
    mml.push("V15".to_string());
    mml.push(format!("O{}", start_octave));

    let mut current_octave = start_octave;
    let mut tempo_change_index = 1; // 0은 시작 템포이므로 1부터 시작

    // 기본 길이 계산
    let mut length_counts: HashMap<String, usize> = HashMap::new();
    for note in voice_notes {
        let octave = (note.note as i32 / 12) - 1;
        let lengths = find_best_length(note.duration, octave, &exact_lengths, compress_mode);
        let first_length = lengths[0].0.trim_end_matches('.').to_string();
        *length_counts.entry(first_length).or_insert(0) += 1;
    }

    let mut default_length = "8".to_string();
    for preferred in &["8", "16", "4"] {
        if length_counts.contains_key(*preferred) {
            default_length = preferred.to_string();
            break;
        }
    }
    if default_length == "8" && !length_counts.contains_key("8") {
        if let Some(max_key) = length_counts.iter().max_by_key(|(_, &count)| count).map(|(k, _)| k) {
            default_length = max_key.clone();
        }
    }

    mml.push(format!("L{}", default_length));

    let mut current_tick = 0u32;

    for note in voice_notes {
        // 템포 변경 확인 (현재 tick과 note.start 사이)
        while tempo_change_index < tempo_changes.len() {
            let tempo_change = &tempo_changes[tempo_change_index];
            if tempo_change.tick <= note.start && tempo_change.tick >= current_tick {
                // 템포 변경 전까지 쉼표 삽입
                let gap_before_tempo = tempo_change.tick.saturating_sub(current_tick);
                if gap_before_tempo > 0 {
                    let rest_lengths = find_best_length(gap_before_tempo, 4, &exact_lengths, compress_mode);
                    for (rest_length, rest_ticks) in rest_lengths {
                        if rest_length == default_length {
                            mml.push("R".to_string());
                        } else {
                            mml.push(format!("R{}", rest_length));
                        }
                        current_tick += rest_ticks;
                    }
                }
                // 템포 변경 명령 삽입
                mml.push(format!("T{}", tempo_change.bpm));
                tempo_change_index += 1;
            } else {
                break;
            }
        }

        // 갭 계산
        let gap = note.start.saturating_sub(current_tick);

        // 쉼표 삽입 (O4 고정 - 동기화)
        if gap > 0 {
            let rest_lengths = find_best_length(gap, 4, &exact_lengths, compress_mode);

            for (rest_length, rest_ticks) in rest_lengths {
                if rest_length == default_length {
                    mml.push("R".to_string());
                } else {
                    mml.push(format!("R{}", rest_length));
                }
                current_tick += rest_ticks;
            }
        }

        // 음표 출력
        let (note_name, octave) = midi_to_note_name(note.note);

        if octave != current_octave {
            mml.push(format!("O{}", octave));
            current_octave = octave;
        }

        // 옥타브별 최적 길이 선택
        let lengths = find_best_length(note.duration, octave, &exact_lengths, compress_mode);

        // 첫 음표
        let (first_length, first_ticks) = &lengths[0];
        if first_length == &default_length {
            mml.push(note_name.clone());
        } else {
            mml.push(format!("{}{}", note_name, first_length));
        }
        current_tick += first_ticks;

        // 타이로 연결
        for (length_str, length_ticks) in lengths.iter().skip(1) {
            mml.push("&".to_string());
            if length_str == &default_length {
                mml.push(note_name.clone());
            } else {
                mml.push(format!("{}{}", note_name, length_str));
            }
            current_tick += length_ticks;
        }
    }

    mml.join("")
}

