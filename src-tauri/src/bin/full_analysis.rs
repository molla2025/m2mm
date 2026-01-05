use std::fs;
use std::path::PathBuf;
use std::collections::{HashMap, HashSet};

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

    println!("╔════════════════════════════════════════════════════════════════╗");
    println!("║          COMPREHENSIVE MIDI vs MML ANALYSIS                    ║");
    println!("╚════════════════════════════════════════════════════════════════╝");
    println!();
    println!("Analyzing: {}", midi_path.display());
    println!();

    // Read MIDI file
    let midi_data = fs::read(&midi_path)?;
    
    // Analyze MIDI
    let analysis = analyzer::analyze_midi(&midi_data)?;
    
    println!("═══ MIDI File Information ═══");
    println!("  Total notes in MIDI: {}", analysis.notes.len());
    println!("  Duration: {:.2}s", analysis.duration_ms / 1000.0);
    println!("  BPM: {}", analysis.tempo_changes[0].bpm);
    println!();

    // Extract and convert
    let (extracted_notes, bpm) = converter::extract_midi_notes(&midi_data, 24)?;
    println!("═══ Extraction Phase ═══");
    println!("  Notes extracted: {}", extracted_notes.len());
    println!("  Notes lost in extraction: {}", analysis.notes.len() - extracted_notes.len());
    println!();

    // Voice allocation
    let voices = converter::allocate_voices_smart(extracted_notes.clone());
    
    let mut total_assigned = 0;
    for voice in &voices {
        total_assigned += voice.len();
    }
    
    println!("═══ Voice Allocation Phase ═══");
    println!("  Notes assigned to voices: {}", total_assigned);
    println!("  Notes lost in allocation: {}", extracted_notes.len() - total_assigned);
    println!();

    // Detailed loss breakdown by timing
    println!("═══ DETAILED LOSS ANALYSIS ═══");
    println!();
    
    // Group original notes by start time
    let mut original_by_time: HashMap<u32, Vec<&converter::Note>> = HashMap::new();
    for note in &extracted_notes {
        original_by_time.entry(note.start).or_insert_with(Vec::new).push(note);
    }
    
    // Group assigned notes by start time
    let mut assigned_by_time: HashMap<u32, HashSet<u8>> = HashMap::new();
    for voice in &voices {
        for note in voice {
            assigned_by_time.entry(note.start)
                .or_insert_with(HashSet::new)
                .insert(note.note);
        }
    }
    
    // Find all timing points
    let mut all_timings: Vec<u32> = original_by_time.keys().copied().collect();
    all_timings.sort();
    
    // Count losses by section
    let total_duration = *all_timings.last().unwrap_or(&0);
    let section_size = total_duration / 10; // 10 sections
    let mut losses_by_section = vec![0; 10];
    let mut total_losses = 0;
    let mut loss_details = Vec::new();
    
    for timing in &all_timings {
        let original_notes = &original_by_time[timing];
        let assigned_notes = assigned_by_time.get(timing).cloned().unwrap_or_default();
        
        let original_set: HashSet<u8> = original_notes.iter().map(|n| n.note).collect();
        let lost_notes: Vec<u8> = original_set.iter()
            .filter(|n| !assigned_notes.contains(n))
            .copied()
            .collect();
        
        if !lost_notes.is_empty() {
            let section = (*timing / section_size).min(9) as usize;
            losses_by_section[section] += lost_notes.len();
            total_losses += lost_notes.len();
            
            loss_details.push((*timing, original_notes.len(), lost_notes.clone()));
        }
    }
    
    println!("Total notes lost: {} ({:.1}% of extracted notes)", 
        total_losses, 
        (total_losses as f64 / extracted_notes.len() as f64) * 100.0
    );
    println!();
    
    println!("Losses by section (10% intervals):");
    for (i, count) in losses_by_section.iter().enumerate() {
        let start_percent = i * 10;
        let end_percent = (i + 1) * 10;
        let bar_length = (*count as f64 / total_losses.max(1) as f64 * 40.0) as usize;
        let bar = "█".repeat(bar_length);
        println!("  {:3}%-{:3}%: {:4} notes lost  {}", 
            start_percent, end_percent, count, bar);
    }
    println!();
    
    // Show worst cases
    println!("═══ WORST CASE TIMINGS (Top 20) ═══");
    println!();
    
    let mut sorted_losses = loss_details.clone();
    sorted_losses.sort_by_key(|(_, _, lost)| std::cmp::Reverse(lost.len()));
    
    for (i, (timing, original_count, lost_notes)) in sorted_losses.iter().take(20).enumerate() {
        let time_seconds = (*timing as f64 / 384.0) * 60.0 / bpm as f64;
        println!("{}. Tick {} ({:.1}s): {} notes → {} LOST", 
            i + 1, timing, time_seconds, original_count, lost_notes.len());
        
        let original = &original_by_time[timing];
        let original_note_nums: Vec<u8> = original.iter().map(|n| n.note).collect();
        println!("   Original: {:?}", original_note_nums);
        println!("   Lost: {:?}", lost_notes);
        
        // Check voice states at this timing
        let mut voices_occupied = 0;
        for (v_idx, voice) in voices.iter().enumerate() {
            let mut has_overlap = false;
            for note in voice {
                if note.start < *timing && note.end > *timing {
                    has_overlap = true;
                    break;
                }
            }
            if has_overlap {
                voices_occupied += 1;
            }
        }
        println!("   Voices occupied: {}/6", voices_occupied);
        println!();
    }
    
    // Analyze by chord size
    println!("═══ LOSSES BY CHORD SIZE ═══");
    println!();
    
    let mut losses_by_chord_size: HashMap<usize, (usize, usize)> = HashMap::new();
    for (timing, original_count, lost_notes) in &loss_details {
        let entry = losses_by_chord_size.entry(*original_count).or_insert((0, 0));
        entry.0 += 1; // count of occurrences
        entry.1 += lost_notes.len(); // total notes lost
    }
    
    let mut chord_sizes: Vec<usize> = losses_by_chord_size.keys().copied().collect();
    chord_sizes.sort();
    
    for size in chord_sizes {
        let (occurrences, total_lost) = losses_by_chord_size[&size];
        let avg_lost = total_lost as f64 / occurrences as f64;
        println!("  {}-note chords: {} occurrences, {} notes lost (avg {:.1} per chord)", 
            size, occurrences, total_lost, avg_lost);
    }
    println!();
    
    // Analyze by note range (pitch)
    println!("═══ LOSSES BY NOTE RANGE ═══");
    println!();
    
    let mut lost_notes_all = Vec::new();
    for (_, _, lost_notes) in &loss_details {
        lost_notes_all.extend(lost_notes);
    }
    
    let mut losses_by_range: HashMap<&str, usize> = HashMap::new();
    for note in &lost_notes_all {
        let range = match note {
            0..=35 => "Very Low (C0-B1)",
            36..=47 => "Low (C2-B2)",
            48..=59 => "Mid-Low (C3-B3)",
            60..=71 => "Mid (C4-B4)",
            72..=83 => "Mid-High (C5-B5)",
            84..=95 => "High (C6-B6)",
            _ => "Very High (C7+)",
        };
        *losses_by_range.entry(range).or_insert(0) += 1;
    }
    
    let mut ranges: Vec<(&str, usize)> = losses_by_range.into_iter().collect();
    ranges.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    
    for (range, count) in ranges {
        let percentage = (count as f64 / lost_notes_all.len() as f64) * 100.0;
        println!("  {}: {} notes ({:.1}%)", range, count, percentage);
    }
    println!();
    
    // Voice utilization analysis
    println!("═══ VOICE UTILIZATION ═══");
    println!();
    
    for (idx, voice) in voices.iter().enumerate() {
        if voice.is_empty() {
            println!("  Voice {}: EMPTY", idx);
            continue;
        }
        
        let note_count = voice.len();
        let duration = voice.last().unwrap().end - voice.first().unwrap().start;
        let duration_sec = (duration as f64 / 384.0) * 60.0 / bpm as f64;
        
        // Calculate note density
        let density = note_count as f64 / duration_sec;
        
        // Count high notes
        let high_notes = voice.iter().filter(|n| n.note >= 72).count();
        let high_percentage = (high_notes as f64 / note_count as f64) * 100.0;
        
        println!("  Voice {}: {} notes, {:.1}s duration, {:.1} notes/sec, {:.0}% high notes", 
            idx, note_count, duration_sec, density, high_percentage);
    }
    println!();
    
    // Recommendation
    println!("═══ RECOMMENDATIONS ═══");
    println!();
    
    if total_losses == 0 {
        println!("  ✓ Perfect! No notes lost.");
    } else {
        let loss_percentage = (total_losses as f64 / extracted_notes.len() as f64) * 100.0;
        
        if loss_percentage < 2.0 {
            println!("  ✓ Excellent quality ({:.1}% loss)", loss_percentage);
            println!("    Losses are minimal and likely unavoidable due to voice limits.");
        } else if loss_percentage < 5.0 {
            println!("  ⚠ Good quality ({:.1}% loss)", loss_percentage);
            println!("    Some improvements possible:");
            println!("    - Most losses in 7+ note chords (unavoidable with 6 voices)");
            println!("    - Consider voice reclamation improvements");
        } else {
            println!("  ✗ Quality issues detected ({:.1}% loss)", loss_percentage);
            println!("    Significant improvements needed:");
            
            // Find the section with most losses
            let (max_section, max_losses) = losses_by_section.iter()
                .enumerate()
                .max_by_key(|(_, &count)| count)
                .unwrap();
            
            println!("    - Most losses in section {}%-{}%", max_section * 10, (max_section + 1) * 10);
            println!("    - {} notes lost in this section alone", max_losses);
            
            // Check if high notes are being lost
            let high_notes_lost: usize = lost_notes_all.iter().filter(|&&n| n >= 72).count();
            if high_notes_lost > total_losses / 4 {
                println!("    - Many high notes being lost ({} / {})", high_notes_lost, total_losses);
                println!("    - High note priority may need adjustment");
            }
        }
    }
    println!();
    
    println!("╔════════════════════════════════════════════════════════════════╗");
    println!("║                    ANALYSIS COMPLETE                           ║");
    println!("╚════════════════════════════════════════════════════════════════╝");

    Ok(())
}