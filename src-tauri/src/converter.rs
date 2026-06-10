use std::collections::HashMap;

use crate::utils::mml::midi_to_note_name;

// 상수 - 표준 TPB (4분음표당 틱 수)
pub const TPB: u32 = 384;
pub const GRID_SIZE: u32 = 24;

// 매칭 실패 시 기본 길이 (16분음표 = 96틱)
const FALLBACK_LENGTH: &str = "16";
const FALLBACK_TICKS: u32 = 96;

// 드럼 채널 (General MIDI) - 변환에서 제외
const DRUM_CHANNEL: u8 = 9;

#[derive(Debug, Clone)]
pub struct Note {
    pub note: u8,
    pub start: u32,
    pub end: u32,
    pub duration: u32,
    pub velocity: u8,
    pub program: u8, // GM 악기 번호 (0~127)
}

#[derive(Debug, Clone)]
pub struct TempoChange {
    pub tick: u32,
    pub bpm: u32,
}

// 점음표 포함 정확한 길이 매핑
fn get_exact_lengths() -> HashMap<u32, &'static str> {
    let mut map = HashMap::new();

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
        result.push((FALLBACK_LENGTH.to_string(), FALLBACK_TICKS));
    }

    result
}

// 안전한 근사치 찾기 (타이 없이)
fn find_safe_approximation(ticks: u32, exact_lengths: &HashMap<u32, &str>) -> Vec<(String, u32)> {
    let closest = exact_lengths
        .keys()
        .min_by_key(|&&x| ((x as i64) - (ticks as i64)).abs())
        .copied()
        .unwrap_or(FALLBACK_TICKS);

    if let Some(&length_str) = exact_lengths.get(&closest) {
        vec![(length_str.to_string(), closest)]
    } else {
        vec![(FALLBACK_LENGTH.to_string(), FALLBACK_TICKS)]
    }
}

// 옥타브별 최적 길이 찾기 (음 씹힘 방지 우선)
fn find_best_length(ticks: u32, octave: i32, exact_lengths: &HashMap<u32, &str>) -> Vec<(String, u32)> {
    // 1. 정확한 매칭 (점음표 포함)
    if let Some(exact) = find_exact_match(ticks, exact_lengths) {
        return exact;
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

// note-on/note-off 쌍으로부터 TPB 변환·그리드 스냅을 적용한 Note 생성
fn build_note(
    note_num: u8,
    start: u32,
    velocity: u8,
    program: u8,
    end_tick: u32,
    tpb_ratio: f64,
) -> Note {
    let duration = end_tick.saturating_sub(start);

    // TPB 변환 - 먼저 변환 후 스냅
    let start_converted = (start as f64 * tpb_ratio).round() as u32;
    let duration_converted = (duration as f64 * tpb_ratio).round() as u32;

    let start_snapped = snap_to_grid(start_converted);
    let end_snapped = snap_to_grid(start_converted + duration_converted);

    // 최소 길이 보장
    let duration_snapped = end_snapped.saturating_sub(start_snapped).max(GRID_SIZE);

    Note {
        note: note_num,
        start: start_snapped,
        end: start_snapped + duration_snapped,
        duration: duration_snapped,
        velocity,
        program,
    }
}

pub fn extract_midi_notes(midi_data: &[u8]) -> Result<(Vec<Note>, u32, Vec<TempoChange>), String> {
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
        let mut active: HashMap<(u8, u8), (u32, u8, u8)> = HashMap::new(); // (start, velocity, program)
        let mut tick = 0u32;

        for event in track {
            tick += event.delta.as_int();

            if let midly::TrackEventKind::Midi { channel, message } = event.kind {
                let ch = channel.as_int();

                match message {
                    midly::MidiMessage::ProgramChange { program } => {
                        channel_programs.insert(ch, program.as_int());
                    }
                    midly::MidiMessage::NoteOn { key, vel } => {
                        let note_num = key.as_int();
                        let velocity = vel.as_int();

                        if velocity > 0 && ch != DRUM_CHANNEL {
                            let program = channel_programs.get(&ch).copied().unwrap_or(0);
                            active.insert((ch, note_num), (tick, velocity, program));
                        } else if velocity == 0 {
                            // velocity 0 NoteOn = NoteOff
                            if let Some((start, velocity, program)) = active.remove(&(ch, note_num)) {
                                notes.push(build_note(
                                    note_num, start, velocity, program, tick, tpb_ratio,
                                ));
                            }
                        }
                    }
                    midly::MidiMessage::NoteOff { key, .. } => {
                        let note_num = key.as_int();
                        if let Some((start, velocity, program)) = active.remove(&(ch, note_num)) {
                            notes.push(build_note(
                                note_num, start, velocity, program, tick, tpb_ratio,
                            ));
                        }
                    }
                    _ => {}
                }
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

// 폴리포니 노트를 최대 max_voices 개의 단음 보이스로 분배.
// 멜로디(최고음)와 베이스(최저음)는 항상 보호하고, 모든 레인이 울리는 중이면
// 가운데 내성부 음의 꼬리를 잘라(steal) 새 음에게 자리를 내준다.
// 반환 보이스는 평균 음높이 내림차순(멜로디 → 베이스)으로 정렬된다.
pub fn allocate_voices_capped(mut notes: Vec<Note>, max_voices: usize) -> Vec<Vec<Note>> {
    if max_voices == 0 || notes.is_empty() {
        return Vec::new();
    }

    // 시작 tick 오름차순, 동시 시작이면 높은 음 우선
    notes.sort_by(|a, b| a.start.cmp(&b.start).then(b.note.cmp(&a.note)));

    let mut lanes: Vec<Vec<Note>> = vec![Vec::new(); max_voices];

    for n in notes {
        // 1) 비어있는(직전 음이 끝난) 레인 중 음높이가 가장 가까운 곳에 배치 → 선율 연속성
        let mut target: Option<usize> = None;
        let mut best_gap = i32::MAX;
        for (i, lane) in lanes.iter().enumerate() {
            let free = lane.last().map_or(true, |l| l.end <= n.start);
            if free {
                let gap = lane
                    .last()
                    .map_or(0, |l| (l.note as i32 - n.note as i32).abs());
                if gap < best_gap {
                    best_gap = gap;
                    target = Some(i);
                }
            }
        }
        if let Some(i) = target {
            lanes[i].push(n);
            continue;
        }

        // 2) 모든 레인이 울리는 중 → 멜로디/베이스는 보호, 가운데 내성부의 꼬리를 잘라 자리 확보.
        //    (동시에 시작한 음은 자르면 길이가 0이 되므로 victim 후보에서 제외)
        let hi = lanes
            .iter()
            .filter_map(|l| l.last().map(|x| x.note))
            .max()
            .unwrap_or(0);
        let lo = lanes
            .iter()
            .filter_map(|l| l.last().map(|x| x.note))
            .min()
            .unwrap_or(0);

        // 내성부 victim 후보: 꼬리를 자를 수 있는 것(시간차) vs 교체만 가능한 동시발음
        let mut trunc_victim: Option<usize> = None; // 직전 음을 잘라 둘 다 보존
        let mut trunc_gap = i32::MAX;
        let mut repl_victim: Option<usize> = None; // 동시발음이라 교체(직전 음 버림)만 가능
        let mut repl_gap = i32::MAX;
        for (i, lane) in lanes.iter().enumerate() {
            if let Some(last) = lane.last() {
                if last.note == hi || last.note == lo {
                    continue; // 멜로디/베이스 보호
                }
                let gap = (last.note as i32 - n.note as i32).abs();
                if last.start < n.start {
                    if gap < trunc_gap {
                        trunc_gap = gap;
                        trunc_victim = Some(i);
                    }
                } else if gap < repl_gap {
                    repl_gap = gap;
                    repl_victim = Some(i);
                }
            }
        }

        if let Some(i) = trunc_victim {
            // 내성부 직전 음의 꼬리를 잘라 자리 확보 → 직전 음과 새 음 모두 보존
            if let Some(last) = lanes[i].last_mut() {
                last.end = n.start;
                last.duration = n.start.saturating_sub(last.start);
            }
            lanes[i].push(n);
        } else if (n.note > hi || n.note < lo) && repl_victim.is_some() {
            // 자를 게 없지만 새 음이 새 멜로디/베이스면 동시발음 내성부 하나를 교체
            let i = repl_victim.unwrap();
            lanes[i].pop();
            lanes[i].push(n);
        }
        // 그 외(보호 대상만 울리거나 새 음이 내성부)면 새 음은 버린다.
    }

    // 빈 레인 제거 후 평균 음높이 내림차순 정렬(멜로디가 앞으로)
    let mut voices: Vec<Vec<Note>> = lanes.into_iter().filter(|l| !l.is_empty()).collect();
    voices.sort_by(|a, b| avg_pitch(b).cmp(&avg_pitch(a)));
    voices
}

// 보이스 평균 음높이
fn avg_pitch(voice: &[Note]) -> u32 {
    if voice.is_empty() {
        return 0;
    }
    voice.iter().map(|n| n.note as u32).sum::<u32>() / voice.len() as u32
}

// 구간 겹침 기준 최대 동시발음 수 (이 악기가 실제로 필요로 하는 보이스 수)
pub fn max_polyphony(notes: &[Note]) -> usize {
    let mut events: Vec<(u32, i32)> = Vec::with_capacity(notes.len() * 2);
    for n in notes {
        events.push((n.start, 1));
        events.push((n.end, -1));
    }
    events.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    let mut cur = 0i32;
    let mut max = 0i32;
    for (_, d) in events {
        cur += d;
        max = max.max(cur);
    }
    max as usize
}

// 악기(program)별로 그룹화하고 노트 수(중요도) 내림차순 정렬
fn group_by_instrument(notes: Vec<Note>) -> Vec<Vec<Note>> {
    let mut groups: HashMap<u8, Vec<Note>> = HashMap::new();
    for n in notes {
        groups.entry(n.program).or_default().push(n);
    }
    let mut groups: Vec<Vec<Note>> = groups.into_values().collect();
    // 노트 수 많은 악기 = 중요도 높음
    groups.sort_by(|a, b| b.len().cmp(&a.len()));
    groups
}

/// 악기 인지 보이스 분배 (총 max_voices 예산).
/// - 악기가 하나면 전 예산을 그 악기에 (단음 악기면 1보이스만 나옴).
/// - 여러 악기면 악기당 최대 PER_INSTRUMENT_CAP(3)까지, 중요도(노트 수) 순으로
///   레벨별로 한 보이스씩 돌아가며 채운다 → 중요한 악기가 더 받되 최대한 많은 악기를 대표.
pub fn allocate_voices_by_instrument(notes: Vec<Note>, max_voices: usize) -> Vec<Vec<Note>> {
    if max_voices == 0 || notes.is_empty() {
        return Vec::new();
    }

    let groups = group_by_instrument(notes);

    // 악기 1개 → 전 예산을 그 악기에
    if groups.len() <= 1 {
        let only = groups.into_iter().next().unwrap_or_default();
        return allocate_voices_capped(only, max_voices);
    }

    // 여러 악기: 악기당 min(실제 동시발음, 3)까지, 중요도순 레벨별 분배
    const PER_INSTRUMENT_CAP: usize = 3;
    let caps: Vec<usize> = groups
        .iter()
        .map(|g| max_polyphony(g).min(PER_INSTRUMENT_CAP))
        .collect();
    let mut alloc = vec![0usize; groups.len()];
    let mut remaining = max_voices;

    'fill: loop {
        let mut progressed = false;
        for (i, a) in alloc.iter_mut().enumerate() {
            if remaining == 0 {
                break 'fill;
            }
            if *a < caps[i] {
                *a += 1;
                remaining -= 1;
                progressed = true;
            }
        }
        if !progressed {
            break; // 모든 악기가 상한에 도달
        }
    }

    // 악기별로 배정된 보이스 수만큼 capped 분배
    let mut voices = Vec::new();
    for (group, a) in groups.into_iter().zip(alloc) {
        if a > 0 {
            voices.extend(allocate_voices_capped(group, a));
        }
    }
    voices
}

// 길이(틱)를 쉼표 토큰들로 출력하고 실제 출력된 틱 합을 반환
fn push_rest(
    mml: &mut Vec<String>,
    ticks: u32,
    default_length: &str,
    exact_lengths: &HashMap<u32, &str>,
) -> u32 {
    let mut emitted = 0;
    for (len_str, len_ticks) in find_best_length(ticks, 4, exact_lengths) {
        if len_str == default_length {
            mml.push("R".to_string());
        } else {
            mml.push(format!("R{}", len_str));
        }
        emitted += len_ticks;
    }
    emitted
}

// 한 음을 길이(틱)만큼 출력. lead_tie=true 면 맨 앞에 타이(&)로 직전 음과 연결.
// 실제 출력된 틱 합을 반환.
fn push_note(
    mml: &mut Vec<String>,
    note_name: &str,
    ticks: u32,
    octave: i32,
    lead_tie: bool,
    default_length: &str,
    exact_lengths: &HashMap<u32, &str>,
) -> u32 {
    let mut emitted = 0;
    for (i, (len_str, len_ticks)) in find_best_length(ticks, octave, exact_lengths)
        .into_iter()
        .enumerate()
    {
        if lead_tie || i > 0 {
            mml.push("&".to_string());
        }
        if len_str == default_length {
            mml.push(note_name.to_string());
        } else {
            mml.push(format!("{}{}", note_name, len_str));
        }
        emitted += len_ticks;
    }
    emitted
}

pub fn generate_mml_final(voice_notes: &[Note], bpm: u32, start_octave: i32, tempo_changes: &[TempoChange]) -> String {
    if voice_notes.is_empty() {
        return String::new();
    }

    let exact_lengths = get_exact_lengths();
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
        let lengths = find_best_length(note.duration, octave, &exact_lengths);
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
        let note_end = note.start + note.duration;

        // 1) note.start 이전(갭/쉼표 구간)에 위치한 템포 변경 삽입
        while tempo_change_index < tempo_changes.len()
            && tempo_changes[tempo_change_index].tick <= note.start
        {
            let t = &tempo_changes[tempo_change_index];
            let gap = t.tick.saturating_sub(current_tick);
            if gap > 0 {
                current_tick += push_rest(&mut mml, gap, &default_length, &exact_lengths);
            }
            mml.push(format!("T{}", t.bpm));
            tempo_change_index += 1;
        }

        // 2) note.start 까지 남은 갭 쉼표
        let gap = note.start.saturating_sub(current_tick);
        if gap > 0 {
            current_tick += push_rest(&mut mml, gap, &default_length, &exact_lengths);
        }

        // 3) 옥타브 명령
        let (note_name, octave) = midi_to_note_name(note.note);
        if octave != current_octave {
            mml.push(format!("O{}", octave));
            current_octave = octave;
        }

        // 4) 음표 출력 - 음 길이 도중에 걸리는 템포 변경은 타이(&)로 분할하고 사이에 T 삽입
        let mut seg_start = note.start;
        let mut lead_tie = false; // 첫 세그먼트는 타이 없이 시작
        loop {
            // 이번 세그먼트 (seg_start, note_end) 안쪽에 걸리는 다음 템포 변경
            let split_tick = if tempo_change_index < tempo_changes.len()
                && tempo_changes[tempo_change_index].tick > seg_start
                && tempo_changes[tempo_change_index].tick < note_end
            {
                Some(tempo_changes[tempo_change_index].tick)
            } else {
                None
            };

            let seg_end = split_tick.unwrap_or(note_end);
            let seg_ticks = seg_end.saturating_sub(seg_start);
            if seg_ticks > 0 {
                current_tick += push_note(
                    &mut mml,
                    &note_name,
                    seg_ticks,
                    octave,
                    lead_tie,
                    &default_length,
                    &exact_lengths,
                );
                lead_tie = true; // 다음 세그먼트는 타이로 연결
            }

            match split_tick {
                Some(_) => {
                    let t = &tempo_changes[tempo_change_index];
                    mml.push(format!("T{}", t.bpm));
                    tempo_change_index += 1;
                    seg_start = seg_end;
                }
                None => break,
            }
        }
    }

    mml.join("")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(num: u8, start: u32, duration: u32) -> Note {
        Note {
            note: num,
            start,
            end: start + duration,
            duration,
            velocity: 100,
            program: 0,
        }
    }

    fn note_prog(num: u8, start: u32, duration: u32, program: u8) -> Note {
        Note {
            program,
            ..note(num, start, duration)
        }
    }

    // 노트 길이 한가운데에 걸리는 템포 변경이 누락되지 않고, 타이로 분할되어 삽입되어야 한다.
    #[test]
    fn tempo_change_inside_a_note_is_emitted_with_tie_split() {
        let notes = vec![note(60, 384, 768)]; // C4, tick 384~1152 (틱 768을 통과)
        let tempos = vec![
            TempoChange { tick: 0, bpm: 120 },
            TempoChange { tick: 768, bpm: 90 },
        ];

        let mml = generate_mml_final(&notes, 120, 4, &tempos);

        assert!(mml.contains("T90"), "노트 중간 템포가 누락됨: {mml}");
        assert!(mml.contains("T90&"), "템포 경계에서 타이 분할이 안 됨: {mml}");
    }

    // 첫 템포가 노트 중간에 걸려도 그 이후의 템포 변경까지 모두 출력되어야 한다(영구 차단 방지).
    #[test]
    fn multiple_tempo_changes_are_not_blocked() {
        let notes = vec![note(60, 0, 1536)]; // C4, tick 0~1536
        let tempos = vec![
            TempoChange { tick: 0, bpm: 120 },
            TempoChange { tick: 384, bpm: 100 },
            TempoChange { tick: 768, bpm: 90 },
        ];

        let mml = generate_mml_final(&notes, 120, 4, &tempos);

        assert!(mml.contains("T100"), "첫 노트 중간 템포 누락: {mml}");
        assert!(mml.contains("T90"), "이후 템포가 차단됨: {mml}");
    }

    // 캡 분배: 보이스 수가 max_voices 를 절대 넘지 않아야 한다.
    #[test]
    fn capped_never_exceeds_max_voices() {
        // 같은 시각에 8음 동시 타격
        let mut notes = Vec::new();
        for (i, p) in [48, 52, 55, 60, 64, 67, 72, 76].into_iter().enumerate() {
            notes.push(note(p, 0, 384));
            // 약간씩 다른 길이로 겹치게
            notes.push(note(p, 384, 384 + i as u32 * 24));
        }
        for k in [1usize, 3, 6] {
            let voices = allocate_voices_capped(notes.clone(), k);
            assert!(voices.len() <= k, "k={k} 인데 보이스 {}개", voices.len());
            // 각 보이스는 단음(시간 겹침 없음)이어야 한다.
            for v in &voices {
                for w in v.windows(2) {
                    assert!(w[0].end <= w[1].start, "보이스 내 음이 겹침: {:?}", w);
                }
            }
        }
    }

    // 멀티악기 샘플로 악기 인지 분배 결과 확인 (cargo test explore_instruments -- --ignored --nocapture)
    #[test]
    #[ignore]
    fn explore_instruments() {
        use crate::utils::mml::gm_family_name;
        let path = "../sample/jin-jino-ju-ren-hong-lianno-gong-shishort-ver.mid";
        let (notes, bpm, _t) = extract_midi_notes(&std::fs::read(path).unwrap()).unwrap();
        println!("\n{path}\n총 노트: {}, BPM: {}", notes.len(), bpm);

        let voices = allocate_voices_by_instrument(notes.clone(), 6);
        let kept: usize = voices.iter().map(|v| v.len()).sum();
        println!(
            "\n[최대 6보이스] {}개, 보존 {:.1}%",
            voices.len(),
            100.0 * kept as f64 / notes.len() as f64
        );
        for v in &voices {
            let prog = v[0].program;
            let oct = ((v[0].note as i32 / 12) - 1).clamp(2, 6);
            let mml_len = generate_mml_final(v, bpm, oct, &_t).len();
            println!(
                "   {} (prog{}): {}음, MML {}자",
                gm_family_name(prog),
                prog,
                v.len(),
                mml_len
            );
        }
    }

    // 악기 인지 분배: 합주 시 각 악기가 적어도 하나의 보이스로 대표되어야 한다.
    #[test]
    fn ensemble_represents_each_instrument() {
        // 피아노(prog0) 화음 + 기타(prog24) 단선 + 플룻(prog73) 단선
        let mut notes = Vec::new();
        for t in 0..8u32 {
            // 피아노: 3음 화음
            notes.push(note_prog(60, t * 384, 384, 0));
            notes.push(note_prog(64, t * 384, 384, 0));
            notes.push(note_prog(67, t * 384, 384, 0));
            // 기타
            notes.push(note_prog(52, t * 384, 384, 24));
            // 플룻
            notes.push(note_prog(79, t * 384, 384, 73));
        }
        let voices = allocate_voices_by_instrument(notes, 6);
        let programs: std::collections::HashSet<u8> =
            voices.iter().map(|v| v[0].program).collect();
        assert!(programs.contains(&0), "피아노가 대표되어야 함");
        assert!(programs.contains(&24), "기타가 대표되어야 함");
        assert!(programs.contains(&73), "플룻이 대표되어야 함");
    }

    // 캡 분배: 멜로디(최고음)와 베이스(최저음)는 보호되어야 한다.
    #[test]
    fn capped_preserves_melody_and_bass() {
        // 6음 동시(40~84), 멜로디=84 베이스=40, max_voices=3 으로 강하게 압축
        let pitches = [40u8, 50, 60, 67, 74, 84];
        let notes: Vec<Note> = pitches.iter().map(|&p| note(p, 0, 768)).collect();
        let voices = allocate_voices_capped(notes, 3);

        let kept: Vec<u8> = voices.iter().flat_map(|v| v.iter()).map(|n| n.note).collect();
        assert!(kept.contains(&84), "멜로디(최고음)가 보존되어야 함: {kept:?}");
        assert!(kept.contains(&40), "베이스(최저음)가 보존되어야 함: {kept:?}");
        // 평균 음높이 내림차순 정렬: 첫 보이스 평균 >= 마지막 보이스 평균
        if voices.len() >= 2 {
            assert!(avg_pitch(&voices[0]) >= avg_pitch(voices.last().unwrap()));
        }
    }
}

