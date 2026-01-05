# 🎵 m2mm (MIDI to Mabinogi Mobile MML)

MIDI 파일을 마비노기 모바일 MML 형식으로 변환하는 프로그램입니다.

## 📥 다운로드

**[최신 버전 다운로드](https://drive.google.com/file/d/1GGnDze2b9k71TVpGITMgMA1AKTJXM91I/view?usp=sharing)**

Windows 10/11 (64-bit) 지원

---

## 🚀 사용법

1. 프로그램 실행
2. MIDI 파일(.mid)을 드래그하거나 선택
3. 변환된 MML 코드 복사
4. 게임에 붙여넣기

### 변환 옵션

- **일반 변환**: 피치(음높이) 기준으로 파트 분리
- **악기별 변환**: MIDI 악기별로 파트 분리
- **글자수 제한**: 게임 내 입력 제한에 맞춰 조절 (기본 2400자)

---

## 💡 주요 기능

- **무제한 Voice 생성**: 복잡한 화음도 모두 보존
- **템포 변경 지원**: 곡 중간에 템포가 바뀌어도 정확하게 변환
- **자동 파트 분리**: 멜로디, 베이스, 화음 자동 구분
- **높은 변환 정확도**: TPB 변환 정확도 99%+

---

## ❓ FAQ

**Q: MP3는 안되나요?**  
A: MIDI만 지원합니다. MP3는 음표 데이터가 아닌 소리 파형이라 변환이 불가능합니다.

**Q: Voice가 너무 많이 생성돼요**  
A: 복잡한 화음이 많으면 10개 이상 생성될 수 있습니다. 필요한 파트(Voice 0~5)만 선택하세요.

**Q: 변환이 이상해요**  
A: 원본 MIDI 파일 품질을 확인하거나, 글자수 제한을 늘려보세요. 피아노/기타 편곡이 오케스트라보다 결과가 좋습니다.

---

## 📧 문의

버그 제보 및 문의: **molla202512@gmail.com**

---

## 🔧 개발자 정보

### 기술 스택
- **Backend**: Rust, Tauri v2
- **Frontend**: Svelte 5, TypeScript, Tailwind CSS

### 빌드 방법

```bash
npm install
npm run tauri dev    # 개발 모드
npm run tauri build  # 프로덕션 빌드
```

### 분석 도구 (개발용)

```bash
cd src-tauri
cargo run --bin analyze your_file.mid       # 기본 분석
cargo run --bin full_analysis your_file.mid # 상세 분석
```

---

<details>
<summary><b>📋 버전 히스토리</b></summary>

### v1.2.0 (2025-01)

#### 추가
- 무제한 Voice 생성 시스템
- 템포 변경 지원 (곡 중간에 BPM 변경 가능)
- 고음 우선 할당 알고리즘
- Voice 조기 종료 시스템

#### 개선
- TPB 변환 정확도 99%+ 달성
- 화음 타이밍 동기화 개선
- 멜로디 선택 로직 안정성 향상

#### 수정
- 화음 타이밍 꼬임 현상 해결
- 고음 씹힘 현상 해결
- Voice 할당 충돌 문제 해결

---

### v1.0.0 (2024-12)

#### 초기 릴리즈
- MIDI to MML 기본 변환
- 일반 변환 / 악기별 변환 모드
- 글자수 제한 자동 크롭
- Voice 6개 자동 분리
- Drag & Drop 지원

</details>

---

**Made with ❤️ for Mabinogi Mobile**