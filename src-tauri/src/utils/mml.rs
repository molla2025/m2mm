// MIDI 노트 번호를 MML 음계 이름과 옥타브로 변환
pub fn midi_to_note_name(midi_note: u8) -> (String, i32) {
    let note_names = ["C", "C+", "D", "D+", "E", "F", "F+", "G", "G+", "A", "A+", "B"];
    let octave = (midi_note as i32 / 12) - 1;
    let note_index = (midi_note % 12) as usize;
    let name = note_names[note_index].to_string();
    (name, octave)
}

// GM 악기 program(0~127)을 일반 유저가 알기 쉬운 한글 악기명으로 (UI 파트 라벨용).
// 전문 분류명(리드/파이프/앙상블/신스리드…) 대신 누구나 아는 대표 악기 이름을 쓴다.
// "기타"는 '기타(etc)'로 오해될 수 있어 "통기타"로.
pub fn gm_family_name(program: u8) -> &'static str {
    match program / 8 {
        0 => "피아노",
        1 => "실로폰",   // 크로매틱 타악기 (실로폰·마림바·오르골)
        2 => "오르간",
        3 => "통기타",   // 기타 (etc 오해 방지)
        4 => "베이스",
        5 => "현악",     // 솔로 현악 (바이올린·첼로·하프)
        6 => "현악",     // 현악 합주·합창 (String Ensemble)
        7 => "트럼펫",   // 금관
        8 => "색소폰",   // 리드 (색소폰·클라리넷)
        9 => "플루트",   // 파이프 (플루트·피리)
        10 | 11 | 12 => "신스", // 신디사이저 (리드·패드·FX)
        13 => "전통악기", // 민속
        14 => "타악기",  // 퍼커션 (드럼·스틸드럼)
        _ => "효과음",
    }
}