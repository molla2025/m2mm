// MIDI 노트 번호를 MML 음계 이름과 옥타브로 변환
pub fn midi_to_note_name(midi_note: u8) -> (String, i32) {
    let note_names = ["C", "C+", "D", "D+", "E", "F", "F+", "G", "G+", "A", "A+", "B"];
    let octave = (midi_note as i32 / 12) - 1;
    let note_index = (midi_note % 12) as usize;
    let name = note_names[note_index].to_string();
    (name, octave)
}