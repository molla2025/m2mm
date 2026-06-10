// MIDI 노트 번호를 MML 음계 이름과 옥타브로 변환
pub fn midi_to_note_name(midi_note: u8) -> (String, i32) {
    let note_names = ["C", "C+", "D", "D+", "E", "F", "F+", "G", "G+", "A", "A+", "B"];
    let octave = (midi_note as i32 / 12) - 1;
    let note_index = (midi_note % 12) as usize;
    let name = note_names[note_index].to_string();
    (name, octave)
}

// GM 악기 program(0~127)을 16개 악기군의 한글 이름으로 (UI 라벨용)
pub fn gm_family_name(program: u8) -> &'static str {
    match program / 8 {
        0 => "피아노",
        1 => "타악기",
        2 => "오르간",
        3 => "기타",
        4 => "베이스",
        5 => "현악",
        6 => "앙상블",
        7 => "금관",
        8 => "리드",
        9 => "파이프",
        10 => "신스리드",
        11 => "신스패드",
        12 => "신스FX",
        13 => "민속",
        14 => "퍼커션",
        _ => "효과음",
    }
}