use midly::{Smf, TrackEventKind, MetaMessage, MidiMessage, Timing};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct TempoChange {
    pub tick: u32,
    pub bpm: u32,
}

#[derive(Debug, Clone)]
pub struct NoteEvent {
    pub track: usize,
    pub channel: u8,
    pub note: u8,
    pub velocity: u8,
    pub start_tick: u32,
    pub end_tick: u32,
    pub duration_ticks: u32,
    pub start_time_ms: f64,
    pub end_time_ms: f64,
    pub instrument: String,
}

#[derive(Debug)]
pub struct MidiAnalysis {
    pub tempo_changes: Vec<TempoChange>,
    pub notes: Vec<NoteEvent>,
    pub tracks_count: usize,
    pub ticks_per_beat: u32,
    pub total_ticks: u32,
    pub duration_ms: f64,
    pub channels_used: Vec<u8>,
}

pub fn analyze_midi(midi_data: &[u8]) -> Result<MidiAnalysis, String> {
    let smf = Smf::parse(midi_data).map_err(|e| format!("MIDI parse error: {:?}", e))?;

    let ticks_per_beat = match smf.header.timing {
        Timing::Metrical(tpb) => tpb.as_int() as u32,
        Timing::Timecode(fps, subframe) => {
            (fps.as_f32() * subframe as f32) as u32
        }
    };

    let mut tempo_changes = Vec::new();
    let mut notes = Vec::new();
    let mut note_on_events: HashMap<(usize, u8, u8), (u32, u8)> = HashMap::new();
    let mut channels_used = std::collections::HashSet::new();
    let mut max_tick = 0u32;

    // First pass: collect tempo changes and note events
    for (track_idx, track) in smf.tracks.iter().enumerate() {
        let mut current_tick = 0u32;

        for event in track {
            current_tick += event.delta.as_int();

            match event.kind {
                TrackEventKind::Meta(MetaMessage::Tempo(tempo)) => {
                    let microseconds_per_beat = tempo.as_int();
                    let bpm = 60_000_000 / microseconds_per_beat;
                    tempo_changes.push(TempoChange {
                        tick: current_tick,
                        bpm,
                    });
                }
                TrackEventKind::Midi { channel, message } => {
                    channels_used.insert(channel.as_int());
                    
                    match message {
                        MidiMessage::NoteOn { key, vel } => {
                            if vel > 0 {
                                note_on_events.insert(
                                    (track_idx, channel.as_int(), key.as_int()),
                                    (current_tick, vel.as_int()),
                                );
                            } else {
                                // Note off (velocity 0)
                                if let Some((start_tick, velocity)) = note_on_events.remove(&(track_idx, channel.as_int(), key.as_int())) {
                                    if current_tick > max_tick {
                                        max_tick = current_tick;
                                    }
                                    notes.push(NoteEvent {
                                        track: track_idx,
                                        channel: channel.as_int(),
                                        note: key.as_int(),
                                        velocity,
                                        start_tick,
                                        end_tick: current_tick,
                                        duration_ticks: current_tick - start_tick,
                                        start_time_ms: 0.0,
                                        end_time_ms: 0.0,
                                        instrument: format!("Track{}_Ch{}", track_idx, channel.as_int()),
                                    });
                                }
                            }
                        }
                        MidiMessage::NoteOff { key, .. } => {
                            if let Some((start_tick, velocity)) = note_on_events.remove(&(track_idx, channel.as_int(), key.as_int())) {
                                if current_tick > max_tick {
                                    max_tick = current_tick;
                                }
                                notes.push(NoteEvent {
                                    track: track_idx,
                                    channel: channel.as_int(),
                                    note: key.as_int(),
                                    velocity,
                                    start_tick,
                                    end_tick: current_tick,
                                    duration_ticks: current_tick - start_tick,
                                    start_time_ms: 0.0,
                                    end_time_ms: 0.0,
                                    instrument: format!("Track{}_Ch{}", track_idx, channel.as_int()),
                                });
                            }
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }

    // Sort tempo changes by tick
    tempo_changes.sort_by_key(|t| t.tick);

    // If no tempo found, use default 120 BPM
    if tempo_changes.is_empty() {
        tempo_changes.push(TempoChange { tick: 0, bpm: 120 });
    }

    // Convert ticks to milliseconds for each note
    for note in notes.iter_mut() {
        note.start_time_ms = ticks_to_milliseconds(note.start_tick, &tempo_changes, ticks_per_beat);
        note.end_time_ms = ticks_to_milliseconds(note.end_tick, &tempo_changes, ticks_per_beat);
    }

    // Sort notes by start tick
    notes.sort_by_key(|n| n.start_tick);

    let duration_ms = ticks_to_milliseconds(max_tick, &tempo_changes, ticks_per_beat);

    let mut channels_vec: Vec<u8> = channels_used.into_iter().collect();
    channels_vec.sort();

    Ok(MidiAnalysis {
        tempo_changes,
        notes,
        tracks_count: smf.tracks.len(),
        ticks_per_beat,
        total_ticks: max_tick,
        duration_ms,
        channels_used: channels_vec,
    })
}

fn ticks_to_milliseconds(tick: u32, tempo_changes: &[TempoChange], ticks_per_beat: u32) -> f64 {
    let mut time_ms = 0.0;
    let mut current_tick = 0u32;
    let mut current_bpm = tempo_changes[0].bpm;

    for tempo_change in tempo_changes.iter() {
        if tempo_change.tick > tick {
            break;
        }

        if tempo_change.tick > current_tick {
            // Calculate time for segment with previous tempo
            let delta_ticks = tempo_change.tick - current_tick;
            let beats = delta_ticks as f64 / ticks_per_beat as f64;
            let minutes = beats / current_bpm as f64;
            time_ms += minutes * 60_000.0;
            current_tick = tempo_change.tick;
        }

        current_bpm = tempo_change.bpm;
    }

    // Calculate remaining time
    if tick > current_tick {
        let delta_ticks = tick - current_tick;
        let beats = delta_ticks as f64 / ticks_per_beat as f64;
        let minutes = beats / current_bpm as f64;
        time_ms += minutes * 60_000.0;
    }

    time_ms
}

pub fn print_analysis(analysis: &MidiAnalysis) -> String {
    let mut output = String::new();

    output.push_str("=== MIDI Analysis Report ===\n\n");
    
    output.push_str(&format!("Tracks: {}\n", analysis.tracks_count));
    output.push_str(&format!("Ticks Per Beat: {}\n", analysis.ticks_per_beat));
    output.push_str(&format!("Total Ticks: {}\n", analysis.total_ticks));
    output.push_str(&format!("Duration: {:.2}s ({:.2}ms)\n", analysis.duration_ms / 1000.0, analysis.duration_ms));
    output.push_str(&format!("Channels Used: {:?}\n\n", analysis.channels_used));

    output.push_str("=== Tempo Changes ===\n");
    if analysis.tempo_changes.is_empty() {
        output.push_str("No tempo changes found (using default 120 BPM)\n");
    } else {
        for (idx, tempo) in analysis.tempo_changes.iter().enumerate() {
            output.push_str(&format!(
                "{}. Tick {} -> {} BPM\n",
                idx + 1,
                tempo.tick,
                tempo.bpm
            ));
        }
    }

    output.push_str(&format!("\n=== Notes: {} total ===\n", analysis.notes.len()));
    
    // Group notes by track
    let mut notes_by_track: HashMap<usize, Vec<&NoteEvent>> = HashMap::new();
    for note in &analysis.notes {
        notes_by_track.entry(note.track).or_insert_with(Vec::new).push(note);
    }

    for track_idx in 0..analysis.tracks_count {
        if let Some(track_notes) = notes_by_track.get(&track_idx) {
            output.push_str(&format!("\nTrack {}: {} notes\n", track_idx, track_notes.len()));
            
            // Show first 10 notes of each track
            for (i, note) in track_notes.iter().take(10).enumerate() {
                output.push_str(&format!(
                    "  {}. Note {} | Tick {}-{} ({} ticks) | Time {:.2}ms-{:.2}ms | Ch{} | Vel{}\n",
                    i + 1,
                    note.note,
                    note.start_tick,
                    note.end_tick,
                    note.duration_ticks,
                    note.start_time_ms,
                    note.end_time_ms,
                    note.channel,
                    note.velocity
                ));
            }
            
            if track_notes.len() > 10 {
                output.push_str(&format!("  ... and {} more notes\n", track_notes.len() - 10));
            }
        }
    }

    // Check for timing issues
    output.push_str("\n=== Potential Issues ===\n");
    
    if analysis.tempo_changes.len() > 1 {
        output.push_str(&format!("⚠️  Multiple tempo changes detected ({})\n", analysis.tempo_changes.len()));
        output.push_str("   This may cause timing issues in conversion.\n");
    }

    // Check for overlapping notes (chords)
    let mut simultaneous_notes: HashMap<u32, usize> = HashMap::new();
    for note in &analysis.notes {
        *simultaneous_notes.entry(note.start_tick).or_insert(0) += 1;
    }
    
    let max_simultaneous = simultaneous_notes.values().max().copied().unwrap_or(0);
    output.push_str(&format!("Max simultaneous notes: {}\n", max_simultaneous));
    
    if max_simultaneous > 3 {
        output.push_str("⚠️  Many simultaneous notes detected - chord allocation may be complex\n");
    }

    output
}