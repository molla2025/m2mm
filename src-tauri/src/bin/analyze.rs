use std::fs;
use std::path::PathBuf;

// Import from the main crate
use mobinogi_mml_lib::analyzer;
use mobinogi_mml_lib::converter;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get test.mid path from command line or use default
    let args: Vec<String> = std::env::args().collect();
    let midi_path = if args.len() > 1 {
        PathBuf::from(&args[1])
    } else {
        PathBuf::from("../../test.mid")
    };

    println!("Analyzing MIDI file: {}", midi_path.display());
    println!();

    // Read MIDI file
    let midi_data = fs::read(&midi_path)?;
    println!("File size: {} bytes\n", midi_data.len());

    // Analyze MIDI
    let analysis = analyzer::analyze_midi(&midi_data)?;
    let report = analyzer::print_analysis(&analysis);
    println!("{}", report);

    // Now try to convert it with the current converter
    println!("\n=== Conversion Test ===\n");
    
    match converter::extract_midi_notes(&midi_data, 24) {
        Ok((notes, bpm)) => {
            println!("✓ Extracted {} notes successfully", notes.len());
            println!("✓ BPM: {}", bpm);
            
            // Group notes by start time to see chord structure
            use std::collections::HashMap;
            let mut notes_by_start: HashMap<u32, Vec<&converter::Note>> = HashMap::new();
            for note in &notes {
                notes_by_start.entry(note.start).or_insert_with(Vec::new).push(note);
            }
            
            let mut start_times: Vec<u32> = notes_by_start.keys().copied().collect();
            start_times.sort();
            
            println!("\nFirst 20 timing points:");
            for (i, start) in start_times.iter().take(20).enumerate() {
                let notes_at_time = &notes_by_start[start];
                println!(
                    "{}. Tick {} ({} notes): {:?}",
                    i + 1,
                    start,
                    notes_at_time.len(),
                    notes_at_time.iter().map(|n| n.note).collect::<Vec<_>>()
                );
            }
            
            // Test voice allocation
            println!("\n=== Voice Allocation Test ===\n");
            let voices = converter::allocate_voices_smart(notes.clone());
            println!("Allocated {} voices", voices.len());
            
            for (idx, voice) in voices.iter().enumerate() {
                if !voice.is_empty() {
                    println!(
                        "Voice {}: {} notes (tick range: {}-{})",
                        idx,
                        voice.len(),
                        voice.first().map(|n| n.start).unwrap_or(0),
                        voice.last().map(|n| n.end).unwrap_or(0)
                    );
                    
                    // Show first few notes
                    for (i, note) in voice.iter().take(5).enumerate() {
                        println!(
                            "  {}. Note {} | Tick {}-{} (dur: {})",
                            i + 1,
                            note.note,
                            note.start,
                            note.end,
                            note.duration
                        );
                    }
                    if voice.len() > 5 {
                        println!("  ... and {} more", voice.len() - 5);
                    }
                }
            }
            
            // Analyze note dropping and voice conflicts
            println!("\n=== Note Dropping Analysis ===\n");
            
            let mut dropped_notes = 0;
            let mut total_chords = 0;
            let mut notes_by_start_drop: HashMap<u32, Vec<&converter::Note>> = HashMap::new();
            for note in &notes {
                notes_by_start_drop.entry(note.start).or_insert_with(Vec::new).push(note);
            }
            
            let mut start_times_list: Vec<u32> = notes_by_start_drop.keys().copied().collect();
            start_times_list.sort();
            
            for start_time in &start_times_list {
                let simultaneous = &notes_by_start_drop[start_time];
                if simultaneous.len() > 1 {
                    total_chords += 1;
                    if simultaneous.len() > 6 {
                        dropped_notes += simultaneous.len() - 6;
                        println!("⚠️  Tick {}: {} notes (dropping {} notes)", 
                            start_time, simultaneous.len(), simultaneous.len() - 6);
                    }
                }
            }
            
            println!("\nTotal chords: {}", total_chords);
            println!("Total notes dropped: {}", dropped_notes);
            
            // Comprehensive chord conflict analysis
            println!("\n=== Comprehensive Chord Conflict Analysis ===\n");
            
            // Find all timing points with chords
            let mut chord_timings: Vec<u32> = Vec::new();
            for start_time in &start_times_list {
                let simultaneous = &notes_by_start_drop[start_time];
                if simultaneous.len() > 1 {
                    chord_timings.push(*start_time);
                }
            }
            
            println!("Total chord timing points: {}", chord_timings.len());
            println!("Voice limit: 6");
            println!();
            
            // Analyze each chord for potential issues
            let mut problematic_chords = Vec::new();
            for timing in &chord_timings {
                let simultaneous = &notes_by_start_drop[timing];
                let chord_size = simultaneous.len();
                
                if chord_size > 6 {
                    problematic_chords.push((*timing, chord_size, "Exceeds voice limit"));
                }
                
                // Check if voices are occupied at this timing
                let mut occupied_voices = 0;
                for voice in &voices {
                    if !voice.is_empty() {
                        let last = voice.last().unwrap();
                        if last.end > *timing {
                            occupied_voices += 1;
                        }
                    }
                }
                
                if occupied_voices >= 5 && chord_size >= 3 {
                    problematic_chords.push((*timing, chord_size, "High voice contention"));
                }
            }
            
            println!("Problematic chords found: {}", problematic_chords.len());
            for (i, (timing, size, reason)) in problematic_chords.iter().take(20).enumerate() {
                println!("{}. Tick {}: {} notes - {}", i + 1, timing, size, reason);
                
                // Show what notes are in this chord
                let chord_notes = &notes_by_start_drop[timing];
                let note_nums: Vec<u8> = chord_notes.iter().map(|n| n.note).collect();
                println!("   Notes: {:?}", note_nums);
                
                // Show which voices are occupied
                print!("   Occupied voices: ");
                for (v_idx, voice) in voices.iter().enumerate() {
                    if !voice.is_empty() {
                        let last = voice.last().unwrap();
                        if last.end > *timing {
                            print!("V{}(N{}, ends@{}) ", v_idx, last.note, last.end);
                        }
                    }
                }
                println!();
            }
            
            // Detailed analysis of dropped notes
            println!("\n=== Detailed Drop Analysis (Specific Cases) ===\n");
            
            let problem_ticks = vec![73920, 143232, 184512, 209472];
            for tick in problem_ticks {
                if let Some(original_notes) = notes_by_start_drop.get(&tick) {
                    println!("Tick {}: {} notes in chord", tick, original_notes.len());
                    
                    let note_nums: Vec<u8> = original_notes.iter().map(|n| n.note).collect();
                    println!("  Original notes: {:?}", note_nums);
                    
                    // Check voice state at this tick
                    println!("  Voice states:");
                    for (v_idx, voice) in voices.iter().enumerate() {
                        if !voice.is_empty() {
                            // Find if this voice has a note that overlaps with this tick
                            let mut overlapping = false;
                            let mut current_note = None;
                            for note in voice {
                                if note.start < tick && note.end > tick {
                                    overlapping = true;
                                    current_note = Some(note);
                                    break;
                                } else if note.start == tick {
                                    current_note = Some(note);
                                    break;
                                }
                            }
                            
                            if overlapping {
                                if let Some(n) = current_note {
                                    println!("    V{}: OCCUPIED (N{}, {}-{})", v_idx, n.note, n.start, n.end);
                                }
                            } else if let Some(n) = current_note {
                                println!("    V{}: Assigned N{} at this tick", v_idx, n.note);
                            } else {
                                // Find last note before this tick
                                let last_before = voice.iter()
                                    .filter(|n| n.end <= tick)
                                    .last();
                                if let Some(last) = last_before {
                                    println!("    V{}: Available (last: N{} ended@{})", v_idx, last.note, last.end);
                                }
                            }
                        } else {
                            println!("    V{}: Empty", v_idx);
                        }
                    }
                    
                    // Show what got assigned
                    let mut assigned_at_tick = Vec::new();
                    for (v_idx, voice) in voices.iter().enumerate() {
                        for note in voice {
                            if note.start == tick {
                                assigned_at_tick.push((v_idx, note.note));
                            }
                        }
                    }
                    
                    assigned_at_tick.sort_by_key(|(v, _)| *v);
                    print!("  Assigned: ");
                    for (v_idx, note) in &assigned_at_tick {
                        print!("V{}=N{} ", v_idx, note);
                    }
                    println!();
                    
                    let assigned_set: std::collections::HashSet<u8> = 
                        assigned_at_tick.iter().map(|(_, n)| *n).collect();
                    let dropped: Vec<u8> = note_nums.iter()
                        .filter(|n| !assigned_set.contains(n))
                        .copied()
                        .collect();
                    if !dropped.is_empty() {
                        println!("  DROPPED: {:?} ⚠️⚠️⚠️", dropped);
                    }
                    println!();
                }
            }
            
            // Analyze high note distribution
            println!("\n=== High Note Analysis (>= Note 72 / C5) ===\n");
            
            let high_notes: Vec<&converter::Note> = notes.iter()
                .filter(|n| n.note >= 72)
                .collect();
            
            println!("High notes count: {} / {}", high_notes.len(), notes.len());
            
            // Check which voices got high notes
            let mut high_notes_by_voice: HashMap<usize, usize> = HashMap::new();
            for (idx, voice) in voices.iter().enumerate() {
                let high_count = voice.iter().filter(|n| n.note >= 72).count();
                if high_count > 0 {
                    high_notes_by_voice.insert(idx, high_count);
                    println!("Voice {}: {} high notes", idx, high_count);
                }
            }
            
            // Analyze later section (last 25% of the song)
            let max_tick = notes.iter().map(|n| n.end).max().unwrap_or(0);
            let later_section_start = (max_tick as f64 * 0.75) as u32;
            
            println!("\n=== Later Section Analysis (Tick {} onwards) ===\n", later_section_start);
            
            let later_notes: Vec<&converter::Note> = notes.iter()
                .filter(|n| n.start >= later_section_start)
                .collect();
            
            println!("Later section notes: {}", later_notes.len());
            
            let later_high_notes: Vec<&&converter::Note> = later_notes.iter()
                .filter(|n| n.note >= 72)
                .collect();
            
            println!("Later section high notes: {}", later_high_notes.len());
            
            // Check voice conflicts in later section
            let mut later_voice_notes: HashMap<usize, Vec<&converter::Note>> = HashMap::new();
            for (idx, voice) in voices.iter().enumerate() {
                let voice_later: Vec<&converter::Note> = voice.iter()
                    .filter(|n| n.start >= later_section_start)
                    .collect();
                if !voice_later.is_empty() {
                    let high_in_later = voice_later.iter().filter(|n| n.note >= 72).count();
                    println!("Voice {} in later section: {} notes ({} high)", 
                        idx, voice_later.len(), high_in_later);
                    later_voice_notes.insert(idx, voice_later);
                }
            }
            
            // Find overlapping notes (potential voice conflicts)
            println!("\n=== Voice Overlap Check (Later Section) ===\n");
            
            for voice_idx in 0..voices.len() {
                if let Some(voice_notes) = later_voice_notes.get(&voice_idx) {
                    for note in voice_notes.iter() {
                        // Check if this note overlaps with notes in other voices
                        for other_idx in (voice_idx + 1)..voices.len() {
                            if let Some(other_notes) = later_voice_notes.get(&other_idx) {
                                for other_note in other_notes.iter() {
                                    // Check overlap
                                    if note.start < other_note.end && other_note.start < note.end {
                                        println!("Overlap: V{} N{} (Tick {}-{}) with V{} N{} (Tick {}-{})",
                                            voice_idx, note.note, note.start, note.end,
                                            other_idx, other_note.note, other_note.start, other_note.end);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            
            // Test MML generation
            println!("\n=== MML Generation Test ===\n");
            
            for (idx, voice) in voices.iter().enumerate() {
                if !voice.is_empty() {
                    let first_note = voice[0].note;
                    let mut start_octave = (first_note as i32 / 12) - 1;
                    start_octave = start_octave.max(2).min(6);
                    
                    let mml = converter::generate_mml_final(&voice, bpm, start_octave, false);
                    
                    println!("Voice {}: {} chars", idx, mml.len());
                    println!("  First 200 chars: {}", 
                        if mml.len() > 200 { &mml[..200] } else { &mml });
                    
                    if mml.len() > 200 {
                        println!("  ...");
                    }
                }
            }
            
            // Detailed chord analysis for first and last 30 timing points
            println!("\n=== Detailed Chord Allocation (First 30 timing points) ===\n");
            
            let mut chord_analysis: HashMap<u32, Vec<(usize, u8)>> = HashMap::new();
            for (voice_idx, voice) in voices.iter().enumerate() {
                for note in voice {
                    chord_analysis.entry(note.start)
                        .or_insert_with(Vec::new)
                        .push((voice_idx, note.note));
                }
            }
            
            let mut timing_points: Vec<u32> = chord_analysis.keys().copied().collect();
            timing_points.sort();
            
            for (i, timing) in timing_points.iter().take(30).enumerate() {
                let mut voices_at_time = chord_analysis[timing].clone();
                voices_at_time.sort_by_key(|(v, _)| *v);
                
                print!("{}. Tick {}: ", i + 1, timing);
                for (voice_idx, note) in &voices_at_time {
                    print!("V{}=N{} ", voice_idx, note);
                }
                println!();
                
                // Show if this is a chord
                if voices_at_time.len() > 1 {
                    let notes: Vec<u8> = voices_at_time.iter().map(|(_, n)| *n).collect();
                    let max_note = *notes.iter().max().unwrap();
                    let min_note = *notes.iter().min().unwrap();
                    println!("   └─ CHORD: {} notes, range N{}-N{}", 
                        voices_at_time.len(), min_note, max_note);
            }
        }
            
        // Detailed voice occupation analysis
        println!("\n=== Voice Occupation Analysis (Sample Chords) ===\n");
            
        // Take every 50th chord timing for detailed analysis
        let sample_interval = (chord_timings.len() / 20).max(1);
        for (i, timing) in chord_timings.iter().step_by(sample_interval).take(20).enumerate() {
            let original_notes = &notes_by_start_drop[timing];
            let original_count = original_notes.len();
                
            // Find what actually got assigned at this timing
            let mut assigned_notes = Vec::new();
            for (v_idx, voice) in voices.iter().enumerate() {
                for note in voice {
                    if note.start == *timing {
                        assigned_notes.push((v_idx, note.note));
                    }
                }
            }
                
            let assigned_count = assigned_notes.len();
            let dropped_count = original_count.saturating_sub(assigned_count);
                
            if dropped_count > 0 || original_count >= 4 {
                println!("{}. Tick {}: {} notes → {} assigned, {} DROPPED", 
                    i + 1, timing, original_count, assigned_count, dropped_count);
                    
                let original_note_nums: Vec<u8> = original_notes.iter().map(|n| n.note).collect();
                println!("   Original: {:?}", original_note_nums);
                    
                assigned_notes.sort_by_key(|(v, _)| *v);
                print!("   Assigned: ");
                for (v_idx, note) in &assigned_notes {
                    print!("V{}=N{} ", v_idx, note);
                }
                println!();
                    
                if dropped_count > 0 {
                    let assigned_set: std::collections::HashSet<u8> = 
                        assigned_notes.iter().map(|(_, n)| *n).collect();
                    let dropped: Vec<u8> = original_note_nums.iter()
                        .filter(|n| !assigned_set.contains(n))
                        .copied()
                        .collect();
                    println!("   DROPPED NOTES: {:?} ⚠️", dropped);
                }
            }
        }
            
        // Last 30 timing points analysis
            println!("\n=== Detailed Chord Allocation (Last 30 timing points) ===\n");
            
            let last_30_start = timing_points.len().saturating_sub(30);
            for (i, timing) in timing_points.iter().skip(last_30_start).enumerate() {
                let mut voices_at_time = chord_analysis[timing].clone();
                voices_at_time.sort_by_key(|(v, _)| *v);
                
                print!("{}. Tick {}: ", i + 1, timing);
                for (voice_idx, note) in &voices_at_time {
                    print!("V{}=N{} ", voice_idx, note);
                }
                println!();
                
                // Show if this is a chord
                if voices_at_time.len() > 1 {
                    let notes: Vec<u8> = voices_at_time.iter().map(|(_, n)| *n).collect();
                    let max_note = *notes.iter().max().unwrap();
                    let min_note = *notes.iter().min().unwrap();
                    let high_notes_count = notes.iter().filter(|&&n| n >= 72).count();
                    if high_notes_count > 0 {
                        println!("   └─ CHORD: {} notes, range N{}-N{} ({} high notes) ⚠️", 
                            voices_at_time.len(), min_note, max_note, high_notes_count);
                    } else {
                        println!("   └─ CHORD: {} notes, range N{}-N{}", 
                            voices_at_time.len(), min_note, max_note);
                    }
                }
            }
        }
        Err(e) => {
            println!("✗ Conversion failed: {}", e);
        }
    }

    Ok(())
}