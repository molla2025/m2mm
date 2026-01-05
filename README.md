# 🎵 m2mm (MIDI to Mabinogi Mobile MML)

MIDI(.mid) 파일을 모비노기(모바일 마비노기) MML 형식으로 변환하는 프로그램

[![Version](https://img.shields.io/badge/version-1.2.0-blue.svg)](https://github.com/yourusername/m2mm)
[![License](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)

## 📥 다운로드

**[📥 최신 버전 다운로드 (v1.2.0)](https://drive.google.com/file/d/1GGnDze2b9k71TVpGITMgMA1AKTJXM91I/view?usp=sharing)**

- Windows 10/11 (64-bit)
- 설치 프로그램 실행 후 바로 사용 가능

---

## ✨ v1.2.0 주요 업데이트 (2025-01-05)

### 🎯 완벽한 화음 보존
- **무제한 Voice 생성**: 이제 6개 제한 없이 필요한 만큼 Voice 자동 생성
- **0% 노트 손실**: 추출된 모든 음표를 완벽하게 보존
- **중요도 순 정렬**: 멜로디 → 베이스 → 화음 순서로 자동 정렬

### 🎼 향상된 음질
- **고음 우선 할당**: 멜로디가 항상 Voice 0에 명확하게 배치
- **정확한 타이밍**: TPB 변환 정확도 99%+ 달성
- **화음 동기화**: 템포 변경 시에도 완벽한 화음 타이밍 유지

### 💪 안정성 개선
- 복잡한 화음(7-10개) 완벽 처리
- Voice 경합 문제 해결
- 모든 MIDI 파일 호환성 향상

<details>
<summary><b>📋 전체 변경사항 보기</b></summary>

### v1.2.0 (2025-01-05)

#### 추가
- 무제한 Voice 생성 시스템
- 고음 우선 할당 알고리즘
- Voice 조기 종료 시스템 (화음 공간 확보)
- 화음 멜로디 보호 로직
- 상세 MIDI 분석 도구 (개발자용)

#### 개선
- TPB 변환 정확도 대폭 향상 (480→384 변환 최적화)
- 화음 분리 일관성 100% 달성
- 멜로디 선택 안정성 향상
- Voice 할당 로직 전면 개선

#### 수정
- 화음 타이밍 꼬임 현상 해결
- 고음 씹힘 현상 완전 해결
- 중간 음역 화음 손실 문제 해결
- Voice 경합으로 인한 드롭 현상 제거

#### 성능
- 변환 속도 유지 (< 0.1초)
- 메모리 사용량 최적화
- 노트 보존율: 92.2% → 100% (추출된 노트 기준)

---

### v1.0.0 (2024-12)

#### 초기 릴리즈
- MIDI to MML 기본 변환 기능
- 일반 변환 / 악기별 변환 모드
- 글자 수 제한 자동 크롭
- Voice 6개 자동 분리
- Drag & Drop 지원

</details>

---

## 🚀 사용 방법

1. **프로그램 실행**
2. **변환 옵션 선택** (선택사항)
   - 일반 변환 / 악기별 변환
   - 악보 글자 수 제한 (기본 2400자)
3. **MIDI 파일(.mid) 드래그 또는 선택**
4. **변환된 파트 확인**
   - 멜로디, 화음1~N까지 자동 생성
   - 중요도 순으로 정렬됨
5. **원하는 파트의 "MML 복사하기" 클릭**
6. **게임 내에서 붙여넣기**

## 💡 변환 결과 이해하기

### Voice 구조
```
Voice 0 (멜로디):  가장 중요 - 항상 먼저 선택
Voice 1 (베이스):  두 번째 중요
Voice 2~N (화음):  나머지 화음들 (높은 음부터)
```

### 선택 가이드
- **간단하게**: Voice 0~2 (3개) - 멜로디 + 베이스 + 주요 화음
- **표준**: Voice 0~5 (6개) - 풍부한 화음
- **풀 버전**: Voice 0~N (전체) - 원곡 완벽 재현

### 글자수 제한
- 설정한 글자수를 초과하면 **자동으로 앞부분만 변환**
- 복잡한 곡일수록 글자수가 빠르게 증가
- 글자수 제한을 늘리면 더 긴 구간 변환 가능

---

## ❓ 자주 묻는 질문

<details>
<summary><b>Q: 왜 MP3는 안되고 MIDI만 되나요?</b></summary>

### MIDI = 악보 데이터 📜
- **"어떤 음을, 언제, 얼마나 길게"** 연주하라는 명령어 집합
- 음표 하나하나가 구조화된 정보로 저장
- **비유**: 레고 조립 설명서

### MP3 = 소리 파형 🌊
- 실제 공기 진동을 초당 44,100번 기록한 데이터
- 음표 정보가 없음 (단순 진폭 값)
- **비유**: 완성된 레고 사진

### 결론
MP3를 변환하려면 복잡한 신호 처리와 화음 분리가 필요하며, 특히 여러 악기가 섞인 경우 거의 불가능합니다.

</details>

<details>
<summary><b>Q: Voice가 너무 많이 생성돼요</b></summary>

정상입니다! 복잡한 화음이 많은 곡은 10개 이상의 Voice가 생성될 수 있습니다.

**해결법**:
- 중요한 Voice만 선택 (Voice 0~5 추천)
- 나머지 Voice는 무시해도 됨
- 멜로디(Voice 0)는 필수

</details>

<details>
<summary><b>Q: 변환 품질이 안 좋아요</b></summary>

**확인사항**:
1. 원본 MIDI 파일 품질 확인
2. 글자수 제한 조정 (더 늘려보기)
3. Voice 0(멜로디)부터 순서대로 선택

**개선 팁**:
- 복잡한 오케스트라 편곡보다 피아노/기타 편곡이 더 좋음
- MIDI 편집 프로그램으로 불필요한 트랙 제거
- 드럼 트랙은 자동으로 제외됨

</details>

---

## 📧 문의

버그 리포트, 기능 제안: **molla202512@gmail.com**

---

## 🔧 개발자를 위한 기술 정보

<details>
<summary><b>기술 스택 및 빌드 방법</b></summary>

### 기술 스택
- **Backend**: Rust, Tauri v2, midly
- **Frontend**: Svelte 5, TypeScript, Tailwind CSS, DaisyUI

### 변환 알고리즘

#### 1. MIDI 파싱
```rust
// TPB 정규화 및 노트 추출
let (notes, bpm) = extract_midi_notes(midi_data, 24);
// TPB 변환 비율: target_tpb / source_tpb
```

#### 2. Voice 할당 (v2.0.0 개선)
```
멜로디 선택:
  - 고음(≥72) 우선 Voice 0 할당
  - 이전 멜로디와 5반음 이내 연속성 고려
  
화음 할당:
  1. 멜로디 (가장 높은 음)
  2. 베이스 (가장 낮은 음)
  3. 나머지 화음 (높은 순)
  
Voice 생성:
  - 무제한 생성
  - 사용 가능한 Voice 없으면 자동 추가
  - 조기 종료 시스템으로 공간 확보
```

#### 3. MML 생성
```
점음표 활용: 1., 2., 4., 8., 16., 32., 64.
타이 연결: c4&c8 (정확한 길이 표현)
옥타브 최적화: o2~o6 자동 계산
```

### 개발 환경 설정

```bash
# 의존성 설치
npm install

# 개발 모드 (Hot Reload)
npm run tauri dev

# 프로덕션 빌드
npm run tauri build

# 분석 도구 (개발자용)
cd src-tauri
cargo run --bin full_analysis ../test.mid
cargo run --bin analyze ../test.mid
```

### 프로젝트 구조
```
m2mm/
├── src/                # Svelte frontend
├── src-tauri/
│   ├── src/
│   │   ├── main.rs           # Tauri 메인
│   │   ├── converter.rs      # 핵심 변환 로직
│   │   ├── analyzer.rs       # MIDI 분석
│   │   ├── utils/            # 유틸리티
│   │   └── bin/              # 분석 도구
│   └── Cargo.toml
└── docs/               # 기술 문서
```

### 라이선스
MIT License

</details>

---

**Made with ❤️ for Mabinogi Mobile Players**