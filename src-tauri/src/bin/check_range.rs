use std::fs;
use std::path::PathBuf;
use mobinogi_mml_lib::converter;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let midi_path = if args.len() > 1 {
        PathBuf::from(&args[1])
    } else {
        PathBuf::from("test.mid")
    };

    println!("=== 음역대별 노트 분석 ===\n");
    println!("파일: {}\n", midi_path.display());

    let midi_data = fs::read(&midi_path)?;
    let (notes, bpm, _tempo_changes) = converter::extract_midi_notes(&midi_data, 24)?;

    println!("총 노트 수: {}", notes.len());
    println!("BPM: {}\n", bpm);

    // 음역대별 분류
    let mut bass_notes = Vec::new();
    let mut chord_notes = Vec::new();
    let mut melody_notes = Vec::new();

    for note in &notes {
        if note.note < 60 {
            bass_notes.push(note);
        } else if note.note < 72 {
            chord_notes.push(note);
        } else {
            melody_notes.push(note);
        }
    }

    println!("=== 음역대별 분포 ===");
    println!("저음부 (O2-O3, note < 60): {} 개", bass_notes.len());
    println!("중음부 (O4, 60-71): {} 개", chord_notes.len());
    println!("고음부 (O5+, note >= 72): {} 개\n", melody_notes.len());

    // 각 음역대의 마지막 노트 찾기
    let bass_max_tick = bass_notes.iter().map(|n| n.end).max().unwrap_or(0);
    let chord_max_tick = chord_notes.iter().map(|n| n.end).max().unwrap_or(0);
    let melody_max_tick = melody_notes.iter().map(|n| n.end).max().unwrap_or(0);
    let overall_max_tick = notes.iter().map(|n| n.end).max().unwrap_or(0);

    fn ticks_to_seconds(ticks: u32, bpm: u32) -> f64 {
        let quarter_notes = ticks as f64 / 384.0;
        quarter_notes / bpm as f64 * 60.0
    }

    println!("=== 음역대별 마지막 노트 시간 ===");
    println!("저음부 끝: tick {} = {:.1}초", bass_max_tick, ticks_to_seconds(bass_max_tick, bpm));
    println!("중음부 끝: tick {} = {:.1}초", chord_max_tick, ticks_to_seconds(chord_max_tick, bpm));
    println!("고음부 끝: tick {} = {:.1}초", melody_max_tick, ticks_to_seconds(melody_max_tick, bpm));
    println!("전체 끝: tick {} = {:.1}초\n", overall_max_tick, ticks_to_seconds(overall_max_tick, bpm));

    // 일반 변환 vs 화음 변환 차이 설명
    println!("=== 일반 변환 vs 화음 변환 ===");
    println!("\n[일반 변환]");
    println!("- 모든 노트를 함께 처리");
    println!("- Voice 할당: 동시 발음 기준");
    println!("- 최대 러닝타임: {:.1}초 (전체 노트의 마지막)", ticks_to_seconds(overall_max_tick, bpm));
    
    println!("\n[화음 변환]");
    println!("- 음역대별로 노트를 분리 처리");
    println!("- Voice 할당: 각 음역대 내에서 독립적으로");
    let chord_max = bass_max_tick.max(chord_max_tick).max(melody_max_tick);
    println!("- 최대 러닝타임: {:.1}초 (각 음역대 중 최대)", ticks_to_seconds(chord_max, bpm));

    if chord_max != overall_max_tick {
        println!("\n⚠️  차이 발생!");
        println!("일반: {:.1}초, 화음: {:.1}초", 
            ticks_to_seconds(overall_max_tick, bpm), 
            ticks_to_seconds(chord_max, bpm));
        println!("차이: {:.1}초", 
            (ticks_to_seconds(overall_max_tick, bpm) - ticks_to_seconds(chord_max, bpm)).abs());
    } else {
        println!("\n✓ 동일한 러닝타임");
    }

    // Voice별 할당 확인
    println!("\n=== 일반 변환 Voice 할당 ===");
    let voices_normal = converter::allocate_voices_smart(notes.clone());
    for (i, voice) in voices_normal.iter().enumerate() {
        if !voice.is_empty() {
            let max_end = voice.iter().map(|n| n.end).max().unwrap_or(0);
            println!("Voice {}: {} notes, 끝 {:.1}초", 
                i, voice.len(), ticks_to_seconds(max_end, bpm));
        }
    }

    println!("\n=== 화음 변환 Voice 할당 ===");
    let voices_chord = converter::allocate_voices_by_range(notes.clone());
    for (i, voice) in voices_chord.iter().enumerate() {
        if !voice.is_empty() {
            let max_end = voice.iter().map(|n| n.end).max().unwrap_or(0);
            let avg_note = voice.iter().map(|n| n.note as u32).sum::<u32>() / voice.len() as u32;
            let range = if avg_note >= 72 {
                "멜로디"
            } else if avg_note >= 60 {
                "화음"
            } else {
                "베이스"
            };
            println!("Voice {} ({}): {} notes, 끝 {:.1}초", 
                i, range, voice.len(), ticks_to_seconds(max_end, bpm));
        }
    }

    Ok(())
}