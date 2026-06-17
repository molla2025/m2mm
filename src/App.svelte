<script lang="ts">
  import { onMount } from "svelte"
  import { fade } from "svelte/transition"
  import { invoke } from "@tauri-apps/api/core"
  import { getCurrentWindow } from "@tauri-apps/api/window"
  import { open } from "@tauri-apps/plugin-dialog"

  interface VoiceResult {
    name: string
    content: string
    char_count: number
    note_count: number
    duration: number
  }

  interface ConversionResult {
    success: boolean
    voices: VoiceResult[]
    error: string | null
    bpm: number
    total_notes: number
    original_duration: number
  }

  interface MidiAnalysis {
    total_notes: number
    instruments: number
    max_polyphony: number
  }

  let isDragging = $state(false)
  let isAnalyzing = $state(false)
  let isConverting = $state(false)
  let analysis = $state<MidiAnalysis | null>(null)
  let pendingBytes: number[] | null = null
  let result = $state<ConversionResult | null>(null)
  let fileName = $state("")
  // 모비노기 악보 1칸 글자 수 한도. 게임사가 늘리면 이 값만 바꾸면 됨 (1200 → 2400 으로 늘어난 전례 있음)
  const MML_CHAR_LIMIT = 2400
  let charLimit = $state(MML_CHAR_LIMIT)
  let mode = $state("solo") // "solo"(혼자 3) / "duo"(2인 4) / "ensemble"(합주 6)
  let errorMessage = $state("")
  let copiedIndex = $state(-1)
  let copyTimerId: number | null = null
  let isUpdating = $state(false)
  let showHelp = $state(false) // "연주 방법" 도움말 모달
  let showSplash = $state(true) // 시작 스플래시 화면
  const MIN_SPLASH_MS = 1600 // 너무 빨리 사라지지 않도록 최소 표시 시간
  const splashStart = Date.now()

  // 앱 시작 시 새 버전이 있으면 자동으로 내려받아 설치하고 재시작
  async function checkForUpdates() {
    try {
      const { check } = await import("@tauri-apps/plugin-updater")
      const update = await check()
      if (update) {
        isUpdating = true
        await update.downloadAndInstall()
        const { relaunch } = await import("@tauri-apps/plugin-process")
        await relaunch()
      }
    } catch (error) {
      console.error("업데이트 확인 실패:", error)
      isUpdating = false
    }
  }

  onMount(async () => {
    const appWindow = getCurrentWindow()
    // onMount 시점엔 스플래시 DOM이 이미 준비됨 → 바로 창을 띄운다.
    // (rAF는 창이 hidden이면 호출 안 될 수 있어 직접 show; 흰 화면은 스플래시가 덮어서 안 보임)
    appWindow.show()
    appWindow.setFocus()

    // 새 버전 자동 업데이트 (백그라운드, UI를 막지 않음)
    checkForUpdates()

    // Rust 백엔드에서 설정 불러오기
    try {
      const settings = await invoke<{ char_limit: number; mode: string }>(
        "load_settings",
      )
      charLimit = settings.char_limit
      mode = settings.mode
    } catch (error) {
      console.error("Failed to load settings:", error)
    }

    // 준비 완료 + 최소 표시 시간 보장 후 스플래시를 서서히 닫는다
    const elapsed = Date.now() - splashStart
    setTimeout(() => (showSplash = false), Math.max(0, MIN_SPLASH_MS - elapsed))

    // Drag & Drop 이벤트
    appWindow.onDragDropEvent(event => {
      if (event.payload.type === "drop") {
        isDragging = false
        handleFileDrop(event.payload.paths)
      } else if (event.payload.type === "enter" || event.payload.type === "over") {
        isDragging = true
      } else if (event.payload.type === "leave") {
        isDragging = false
      }
    })
  })

  async function handleFileSelect() {
    try {
      const selected = await open({
        multiple: false,
        filters: [{ name: "MIDI", extensions: ["mid", "midi"] }],
      })

      if (selected && typeof selected === "string") {
        handleFileDrop([selected])
      }
    } catch (error) {
      console.error("File selection error:", error)
    }
  }

  async function handleFileDrop(paths: string[]) {
    isDragging = false
    if (paths.length === 0) return

    const filePath = paths[0]

    if (
      !filePath.toLowerCase().endsWith(".mid") &&
      !filePath.toLowerCase().endsWith(".midi")
    ) {
      errorMessage = "MIDI 파일(.mid)만 지원됩니다."
      return
    }

    fileName = filePath.split(/[\\/]/).pop() || ""
    await analyzeFile(filePath)
  }

  // 파일을 읽어 분석만 하고, 추천 화면을 띄운다 (아직 변환은 안 함)
  async function analyzeFile(filePath: string) {
    isAnalyzing = true
    errorMessage = ""
    result = null
    analysis = null

    try {
      const fs = await import("@tauri-apps/plugin-fs")
      const bytes = await fs.readFile(filePath)
      pendingBytes = Array.from(bytes)

      analysis = await invoke<MidiAnalysis>("analyze_midi", {
        midiData: pendingBytes,
      })
    } catch (error: any) {
      errorMessage = `분석 오류: ${error.toString()}`
      pendingBytes = null
    } finally {
      isAnalyzing = false
    }
  }

  // 선택한 모드로 실제 변환을 수행한다.
  async function convertWithMode(m: string) {
    if (!pendingBytes) return
    mode = m
    isConverting = true
    errorMessage = ""

    try {
      await invoke("save_settings", { charLimit, mode })
    } catch (error) {
      console.error("Failed to save settings:", error)
    }

    try {
      const conversionResult = await invoke<ConversionResult>("convert_midi", {
        midiData: pendingBytes,
        options: { char_limit: charLimit, mode },
      })

      if (conversionResult.success) {
        result = conversionResult
      } else {
        errorMessage = conversionResult.error || "변환 중 오류가 발생했습니다."
      }
    } catch (error: any) {
      errorMessage = `변환 오류: ${error.toString()}`
    } finally {
      isConverting = false
    }
  }

  function copyToClipboard(content: string, index: number) {
    if (copyTimerId !== null) clearTimeout(copyTimerId)

    copiedIndex = index
    navigator.clipboard.writeText(content)

    copyTimerId = setTimeout(() => {
      copiedIndex = -1
      copyTimerId = null
    }, 2500) as unknown as number
  }

  function reset() {
    if (copyTimerId !== null) {
      clearTimeout(copyTimerId)
      copyTimerId = null
    }
    result = null
    analysis = null
    pendingBytes = null
    fileName = ""
    errorMessage = ""
    copiedIndex = -1
  }

  // 동시음이 이 이상이면 6화음으로도 원본을 다 못 담아 싱크가 어긋날 수 있는 경계
  const TOO_MANY_SIMULTANEOUS = 20

  // 분석 결과로 추천 모드와 경고를 만든다 (안내 문구는 템플릿에서 하이라이트로 렌더).
  // 웬만하면 단독을 추천하고, 악기가 많거나(>=5) 동시음이 매우 많을 때(>=12)만 화음을 추천.
  function getRecommendation(a: MidiAnalysis): {
    mode: "solo" | "ensemble"
    warn: string
  } {
    let warn = ""
    if (a.max_polyphony >= TOO_MANY_SIMULTANEOUS) {
      warn = `이 곡은 동시에 울리는 음이 최대 ${a.max_polyphony}개로 너무 많아요. 화음(6명)으로 변환해도 원본의 음을 다 못 담아 화음·싱크가 원본과 다를 수 있어요.`
    } else if (a.instruments > 6) {
      warn = `악기가 ${a.instruments}종이라 화음(6명)에도 다 못 담아요. 중요한 악기 위주로만 변환되고 일부는 빠집니다.`
    }

    const mode = a.instruments >= 5 || a.max_polyphony >= 12 ? "ensemble" : "solo"
    return { mode, warn }
  }

  // 역할/악기군 → daisyUI 시맨틱 색상 (전체 클래스 문자열을 정적으로 보유해야 Tailwind가 생성함)
  type RoleStyle = { card: string; badge: string }
  const ROLE_STYLES: Record<string, RoleStyle> = {
    primary: { card: "border-primary/40 from-primary/10 to-primary/5", badge: "badge-primary" },
    secondary: { card: "border-secondary/40 from-secondary/10 to-secondary/5", badge: "badge-secondary" },
    accent: { card: "border-accent/40 from-accent/10 to-accent/5", badge: "badge-accent" },
    info: { card: "border-info/40 from-info/10 to-info/5", badge: "badge-info" },
    warning: { card: "border-warning/40 from-warning/10 to-warning/5", badge: "badge-warning" },
    error: { card: "border-error/40 from-error/10 to-error/5", badge: "badge-error" },
    neutral: { card: "border-base-300 from-base-300/20 to-base-300/5", badge: "badge-neutral" },
  }

  function getRoleStyle(name: string): RoleStyle {
    // 단일 악기: 역할 라벨
    if (name.includes("멜로디")) return ROLE_STYLES.primary
    if (name.includes("화음")) return ROLE_STYLES.secondary
    if (name.includes("베이스")) return ROLE_STYLES.warning
    // 여러 악기: 악기군 라벨
    if (name.includes("피아노")) return ROLE_STYLES.primary
    if (name.includes("현악")) return ROLE_STYLES.secondary
    if (name.includes("트럼펫") || name.includes("색소폰")) return ROLE_STYLES.warning
    if (name.includes("통기타")) return ROLE_STYLES.error
    if (name.includes("플루트") || name.includes("오르간")) return ROLE_STYLES.accent
    if (name.includes("신스")) return ROLE_STYLES.info
    return ROLE_STYLES.neutral
  }


  function fmtTime(seconds: number): string {
    const s = Math.floor(seconds)
    const m = Math.floor(s / 60)
    return m > 0 ? `${m}분 ${s % 60}초` : `${s % 60}초`
  }
</script>

<div class="flex h-screen flex-col bg-base-100 text-base-content overflow-hidden">
  {#if showSplash}
    <!-- 시작 스플래시: 창은 이미 이게 그려진 채로 등장하고, 준비되면 서서히 사라짐 -->
    <div
      class="absolute inset-0 z-[60] flex flex-col items-center justify-center gap-6 bg-base-100"
      out:fade={{ duration: 480 }}
    >
      <div class="splash-logo">
        <svg width="108" height="108" viewBox="0 0 1024 1024" aria-hidden="true">
          <defs>
            <linearGradient id="splashGrad" x1="0" y1="0" x2="1" y2="1">
              <stop offset="0" stop-color="#6366f1" />
              <stop offset="0.55" stop-color="#7c5cf0" />
              <stop offset="1" stop-color="#8b5cf6" />
            </linearGradient>
          </defs>
          <rect width="1024" height="1024" rx="232" fill="url(#splashGrad)" />
          <g fill="#ffffff">
            <ellipse cx="392" cy="708" rx="124" ry="96" transform="rotate(-22 392 708)" />
            <rect x="497" y="286" width="42" height="430" rx="8" />
            <path d="M539 290 C 690 312 770 392 742 520 C 760 404 660 356 539 372 Z" />
          </g>
          <path
            fill="#22d3ee"
            d="M742 232 c 18 56 28 66 84 84 c -56 18 -66 28 -84 84 c -18 -56 -28 -66 -84 -84 c 56 -18 66 -28 84 -84 Z"
          />
        </svg>
      </div>

      <div class="text-center">
        <h1
          class="bg-gradient-to-r from-primary to-secondary bg-clip-text text-3xl font-extrabold tracking-tight text-transparent"
        >
          딸깍악보
        </h1>
        <p class="mt-1.5 text-xs text-base-content/45">1분이면 누구나 뚝딱</p>
      </div>

      <div class="mt-1 h-[3px] w-40 overflow-hidden rounded-full bg-base-content/10">
        <div
          class="splash-bar h-full w-2/5 rounded-full bg-gradient-to-r from-primary to-secondary"
        ></div>
      </div>
    </div>
  {/if}

  {#if isUpdating}
    <!-- 자동 업데이트 설치 중 오버레이 -->
    <div
      class="absolute inset-0 z-50 flex flex-col items-center justify-center gap-4 bg-base-100/95 backdrop-blur-sm"
    >
      <span class="loading loading-spinner loading-lg text-primary"></span>
      <div class="text-center">
        <p class="text-sm font-semibold">새 버전 설치 중…</p>
        <p class="mt-1 text-xs text-base-content/45">설치가 끝나면 자동으로 다시 시작됩니다</p>
      </div>
    </div>
  {/if}

  {#if showHelp}
    <!-- 연주 방법 도움말 모달 -->
    <div class="modal modal-open" role="dialog" aria-modal="true">
      <div
        class="modal-box flex max-h-[70vh] max-w-lg flex-col overflow-hidden border border-base-300 bg-base-200"
      >
        <h3 class="mb-3 shrink-0 text-base font-bold">🎵 연주 방법 (상세)</h3>

        <div class="flex-1 space-y-4 overflow-y-auto pr-1 text-xs leading-relaxed text-base-content/80">
          <div>
            <p class="mb-1 font-semibold text-base-content">① 동시음 = 여러 단음을 겹치기</p>
            <p class="text-base-content/70">
              MML 한 악보엔 단음(한 번에 한 음) 파트를 여러 개 넣을 수 있고, 그것들이 동시에 울려 화음이
              돼요. 한 악보엔 최대 <b>6파트(동시음 6개)</b>까지. 이 앱은 곡을 멜로디·화음·베이스 파트로
              쪼개서, 가장 중요한 <b>맨 위(멜로디)</b>와 <b>맨 아래(베이스)</b>를 먼저 살리고 가운데 화음은
              자리가 남는 만큼 채워요.
            </p>
          </div>

          <div>
            <p class="mb-1 font-semibold text-base-content">② 누가 몇 파트를 맡나 (핵심)</p>
            <ul class="list-disc space-y-1 pl-4 text-base-content/70">
              <li>
                <b class="text-warning">곡을 처음 여는 사람(시작자)</b>만 1·2·3화음 악기를 골라 한 번에
                <b>최대 3파트</b>까지 혼자 맡을 수 있어요. (3화음 = 3파트 동시)
              </li>
              <li>
                <b class="text-info">나중에 들어오는 사람(참여자)</b>은 어떤 악기를 들었든
                <b>무조건 1파트(1화음)</b>. 여러 파트 특권은 시작자에게만 있어요.
              </li>
              <li>
                시작자가 여러 파트를 한 번에 치려면 그 파트들이 <b>같은 악기</b>여야 해요 — 악기 하나는 한
                음색이라, 3화음 피아노로 피아노 3줄은 ✓ 지만 피아노+트럼펫 동시는 ✗.
              </li>
            </ul>
          </div>

          <div>
            <p class="mb-1 font-semibold text-base-content">③ 모드별 구성</p>
            <ul class="space-y-1.5 text-base-content/70">
              <li>
                🎹 <span class="font-semibold text-primary">단독 · 1명</span> — 시작자가
                <span class="rounded bg-warning/15 px-1 font-bold text-warning">3화음</span> 악기로
                멜로디+화음 <b>3파트</b>를 혼자 다 연주. 핵심 3줄만 추려 담아요.
              </li>
              <li>
                🎻 <span class="font-semibold text-secondary">2인 · 2명</span> — 시작자가 3화음으로
                <b>앞 3파트</b>, 참여자 1명이 1화음으로 <b>베이스</b>. 동시음 <b>4개</b> 커버 — 저음을 따로
                떼서 더 탄탄해요.
              </li>
              <li>
                🎶 <span class="font-semibold text-info">합주 · 최대 6명</span> — 6파트를 악기별로 나눠요.
                같은 악기 파트가 여러 개면 시작자가 묶고, 나머지는 1명당 1파트.
              </li>
            </ul>
          </div>

          <div class="rounded-xl border border-accent/30 bg-accent/5 p-3">
            <p class="mb-1 font-semibold text-accent">④ 고급 — 인원 줄이기 (전략)</p>
            <p class="text-base-content/70">
              같은 악기 파트가 여러 개면 시작자가 3화음으로 한 번에 잡아 인원을 줄일 수 있어요. 인원 ↔ 악기
              준비의 전략 차이예요.
            </p>
            <p class="mt-2 text-[11px] text-base-content/60">
              예) 피아노 3 + 금관 2 + 현악 1 (총 6파트)<br />
              · 시작자 = <span class="font-semibold text-warning">3화음 피아노</span> → 피아노 3줄 (1명)<br
              />
              · 금관 2명 + 현악 1명 (각 1화음)<br />
              → <b class="text-accent">최소 4명</b> · 전부 1화음씩 나누면 6명
            </p>
          </div>

          <div>
            <p class="mb-1 font-semibold text-base-content">⑤ 그 외</p>
            <ul class="list-disc space-y-1 pl-4 text-base-content/70">
              <li>
                합주는 카드의 <b>번호 순서대로</b> — 시작자가 1번을 열고 나머지가 차례로 합류해요.
              </li>
              <li>
                한 파트(악보)는 최대 <b>{charLimit.toLocaleString()}자</b>. 곡이 길면 그 한도에 맞춰
                뒷부분이 잘릴 수 있어요.
              </li>
            </ul>
          </div>
        </div>

        <div class="modal-action mt-3 shrink-0 border-t border-base-300 pt-3">
          <button type="button" class="btn btn-sm btn-primary" onclick={() => (showHelp = false)}>
            알겠어요
          </button>
        </div>
      </div>
      <button
        type="button"
        class="modal-backdrop"
        aria-label="닫기"
        onclick={() => (showHelp = false)}
      ></button>
    </div>
  {/if}

  <!-- Header -->
  <header
    class="flex items-center justify-between border-b border-base-300 bg-base-200/70 px-5 py-3 backdrop-blur-xl"
  >
    <div class="flex items-center gap-3">
      <div
        class="flex h-9 w-9 items-center justify-center rounded-xl bg-gradient-to-br from-primary to-secondary text-lg shadow-lg shadow-primary/25"
      >
        🎵
      </div>
      <div>
        <h1 class="text-sm font-bold leading-tight">딸깍악보</h1>
        <p class="text-[11px] text-base-content/45">누구나 1분이면 OK · MIDI → 모비노기 MML</p>
      </div>
    </div>
    <span class="badge badge-outline badge-sm text-base-content/50">v0.1.2</span>
  </header>

  <!-- Main content -->
  <main class="min-h-0 flex-1 overflow-hidden">
    {#if !result && !analysis}
      <!-- ── 1단계: 드롭존 + 글자 수만 ── -->
      <div class="grid h-full place-items-center p-6">
        <div class="flex w-full max-w-md flex-col gap-4">
          <!-- 드롭존 -->
          <button
            type="button"
            onclick={handleFileSelect}
            disabled={isAnalyzing}
            class="group flex flex-col items-center justify-center gap-4 rounded-3xl border-2 border-dashed px-6 py-16 transition-all duration-200 hover:border-primary hover:bg-primary/5 {isDragging
              ? 'scale-[1.02] border-primary bg-primary/10 shadow-[0_0_40px_-8px_var(--color-primary)]'
              : 'border-base-300 bg-base-200/40'}"
          >
            {#if isAnalyzing}
              <span class="loading loading-spinner loading-lg text-primary"></span>
              <div class="text-center">
                <p class="text-sm font-semibold">분석 중…</p>
                <p class="mt-0.5 text-xs text-base-content/45">{fileName}</p>
              </div>
            {:else}
              <div
                class="flex h-16 w-16 items-center justify-center rounded-2xl bg-gradient-to-br from-primary/15 to-secondary/15 text-primary transition-transform duration-200 group-hover:scale-110"
              >
                <svg class="h-8 w-8" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    stroke-width="1.8"
                    d="M7 16a4 4 0 01-.88-7.903A5 5 0 1115.9 6L16 6a5 5 0 011 9.9M15 13l-3-3m0 0l-3 3m3-3v12"
                  />
                </svg>
              </div>
              <div class="text-center">
                <p class="text-base font-semibold">MIDI 파일을 끌어다 놓으세요</p>
                <p class="mt-1 text-xs text-base-content/45">또는 클릭해서 선택 · .mid / .midi</p>
              </div>
            {/if}
          </button>

          <!-- 글자 수 설정 -->
          <div class="rounded-2xl border border-base-300 bg-base-200/60 px-4 py-3">
            <div class="mb-2 flex items-center gap-1.5">
              <span class="text-xs font-medium text-base-content/70">악보 한 칸 최대 글자 수</span>
              <span
                class="tooltip tooltip-right text-base-content/35"
                data-tip={`모비노기 악보 한 칸의 글자 수 제한이에요 (현재 ${charLimit.toLocaleString()}자). 곡이 길면 이 한도에 맞춰 자동으로 잘립니다.`}
              >
                <svg class="h-3.5 w-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    stroke-width="2"
                    d="M8.228 9c.549-1.165 2.03-2 3.772-2 2.21 0 4 1.343 4 3 0 1.4-1.278 2.575-3.006 2.907-.542.104-.994.54-.994 1.093m0 3h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
                  />
                </svg>
              </span>
            </div>
            <label class="input input-sm w-full bg-base-100">
              <input type="number" bind:value={charLimit} min="500" max="5000" step="100" />
              <span class="text-base-content/40">자</span>
            </label>
          </div>

          {#if errorMessage}
            <div class="alert alert-error py-2 text-xs">
              <span>{errorMessage}</span>
            </div>
          {/if}

          <p class="text-center text-[10px] text-base-content/30">
            문의 · molla202512@gmail.com
          </p>
        </div>
      </div>
    {:else if !result}
      {@const rec = getRecommendation(analysis!)}
      <!-- ── 2단계: 분석 결과 + 모드 추천 ── -->
      <div class="h-full overflow-y-auto">
        <div class="flex min-h-full items-center justify-center p-6">
          <div class="flex w-full max-w-md flex-col gap-4">
          <div class="rounded-3xl border border-base-300 bg-base-200/50 p-5 shadow-xl">
            <h2 class="truncate text-sm font-semibold">{fileName}</h2>
            <div
              class="mb-4 mt-1 flex flex-wrap items-center gap-x-3 gap-y-1 text-[11px] text-base-content/50"
            >
              <span>악기 {analysis!.instruments}종</span>
              <span class="text-base-content/25">·</span>
              <span>최대 동시음 {analysis!.max_polyphony}개</span>
              <span class="text-base-content/25">·</span>
              <span>음표 {analysis!.total_notes.toLocaleString()}개</span>
            </div>

            <!-- 추천 안내 (하이라이트) -->
            <div
              class="flex items-start gap-2 rounded-2xl border border-base-300 bg-base-100 p-3 text-xs leading-relaxed text-base-content/70"
            >
              <span class="text-base leading-none">💡</span>
              <div class="space-y-1">
                {#if rec.mode === "solo"}
                  <p>
                    혼자서 연주하기 좋은 곡이에요.
                    <span class="rounded bg-primary/15 px-1 font-bold text-primary">단독 연주</span>를
                    추천해요. (베이스를 한 명이 더 받쳐주려면
                    <span class="font-semibold text-secondary">2인</span>도 좋아요.)
                  </p>
                {:else}
                  <p>
                    {#if analysis!.instruments >= 5}
                      악기가 <span class="font-semibold text-base-content/90"
                        >{analysis!.instruments}종</span
                      >으로 많아요.
                    {:else}
                      동시에 울리는 음이 최대
                      <span class="font-semibold text-base-content/90"
                        >{analysis!.max_polyphony}개</span
                      >로 많아요.
                    {/if}
                    혼자선 다 살리기 어려우니
                    <span class="rounded bg-primary/15 px-1 font-bold text-primary">화음 연주(6명)</span
                    >가 더 좋아요. (단독·2인도 되지만 일부는 빠져요.)
                  </p>
                {/if}
                <p class="text-base-content/55">
                  ※ 단독은 <span class="rounded bg-warning/15 px-1 font-bold text-warning">3화음</span>
                  악기로 혼자, 합주는 각자 <span class="font-semibold text-info">1화음</span> 악기로!
                  <button
                    type="button"
                    class="font-medium text-info underline-offset-2 hover:underline"
                    onclick={() => (showHelp = true)}>ⓘ 연주 방법</button
                  >
                </p>
              </div>
            </div>

            {#if rec.warn}
              <div class="alert alert-warning mt-3 py-2 text-[11px]">
                <span>{rec.warn}</span>
              </div>
            {/if}

            <!-- 연주 방식 선택 (단독 위 / 화음 아래 고정) -->
            <div class="mt-4 flex flex-col gap-2">
              {#if isConverting}
                <button class="btn btn-primary btn-block" disabled>
                  <span class="loading loading-spinner loading-sm"></span> 변환 중…
                </button>
              {:else}
                <button class="btn btn-primary btn-block" onclick={() => convertWithMode("solo")}>
                  단독 연주 <span class="font-normal opacity-75">· 혼자 (동시음 3개)</span>
                </button>
                <button class="btn btn-outline btn-block" onclick={() => convertWithMode("duo")}>
                  2인 연주 <span class="font-normal opacity-75">· 2명 (동시음 4개, 베이스 분리)</span>
                </button>
                <button
                  class="btn btn-outline btn-block"
                  onclick={() => convertWithMode("ensemble")}
                >
                  화음 연주 <span class="font-normal opacity-75">· 최소 4명 (동시음 6개)</span>
                </button>
              {/if}
            </div>
          </div>

          {#if errorMessage}
            <div class="alert alert-error py-2 text-xs"><span>{errorMessage}</span></div>
          {/if}

          <button
            type="button"
            class="text-center text-xs text-base-content/40 transition-colors hover:text-base-content/70"
            onclick={reset}
          >
            ← 다른 파일 선택
          </button>
          </div>
        </div>
      </div>
    {:else}
      {@const convDuration = Math.max(...result.voices.map(v => v.duration), 0)}
      {@const truncated = Math.floor(result.original_duration) > Math.floor(convDuration)}
      <!-- ── 변환 후: 요약 바 + 파트 카드 ── -->
      <div class="flex h-full flex-col gap-3 p-4">
        <!-- 요약 바 -->
        <div
          class="flex flex-wrap items-center justify-between gap-3 rounded-2xl border border-base-300 bg-base-200/60 px-4 py-3"
        >
          <div class="min-w-0">
            <h2 class="truncate text-sm font-semibold">{fileName}</h2>
            <div class="mt-1 flex flex-wrap items-center gap-x-3 gap-y-1 text-[11px] text-base-content/50">
              <span>BPM {result.bpm}</span>
              <span class="text-base-content/25">·</span>
              <span>음표 {result.total_notes.toLocaleString()}개</span>
              <span class="text-base-content/25">·</span>
              <span>동시음 {result.voices.length}개</span>
              <span class="text-base-content/25">·</span>
              <span class="inline-flex items-center gap-1">
                {mode === "solo" ? "혼자 연주" : mode === "duo" ? "2명" : `${result.voices.length}명`}
                <button
                  type="button"
                  class="text-base-content/40 hover:text-info"
                  aria-label="연주 방법"
                  onclick={() => (showHelp = true)}>ⓘ</button
                >
              </span>
              <span class="text-base-content/25">·</span>
              <span>러닝타임 {fmtTime(convDuration)}</span>
            </div>
          </div>
          <button type="button" onclick={reset} class="btn btn-sm btn-outline gap-2">
            <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"
              />
            </svg>
            새 파일
          </button>
        </div>

        {#if truncated}
          <div class="alert alert-warning py-2 text-xs">
            <svg class="h-4 w-4 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z"
              />
            </svg>
            <span
              >한 칸 글자 수 제한({charLimit.toLocaleString()}자)을 넘어서 뒷부분이 잘렸어요 (원곡 {fmtTime(
                result.original_duration,
              )} → {fmtTime(convDuration)}).</span
            >
          </div>
        {/if}

        {#if mode === "solo"}
          <!-- 단독 안내 -->
          <div
            class="flex items-start gap-2 rounded-2xl border border-info/40 bg-info/5 px-4 py-2.5 text-[11px] leading-relaxed text-base-content/70"
          >
            <span class="text-sm leading-none">🎹</span>
            <span>
              <span class="font-semibold text-info">단독</span> — 반드시
              <span class="rounded bg-warning/15 px-1 font-bold text-warning">3화음</span> 악기로
              <b>혼자</b> 연주하세요.
            </span>
          </div>
        {:else if mode === "duo" && result.voices.length > 1}
          <!-- 2인 안내 -->
          <div
            class="flex items-start gap-2 rounded-2xl border border-info/40 bg-info/5 px-4 py-2.5 text-[11px] leading-relaxed text-base-content/70"
          >
            <span class="text-sm leading-none">🎻</span>
            <span>
              <span class="font-semibold text-info">2인</span> — 시작자
              <span class="rounded bg-warning/15 px-1 font-bold text-warning">3화음</span>(앞 3파트) +
              참여자 <span class="font-semibold text-info">1화음</span>(베이스).
            </span>
          </div>
        {:else if mode === "ensemble" && result.voices.length > 1}
          <!-- 합주 참여 순서 안내 -->
          <div
            class="flex items-start gap-2 rounded-2xl border border-info/40 bg-info/5 px-4 py-2.5 text-[11px] leading-relaxed text-base-content/70"
          >
            <span class="text-sm leading-none">🎻</span>
            <span>
              <span class="font-semibold text-info">합주</span> —
              <b>{result.voices.length}명</b>이 각자 <span class="font-semibold text-info">1화음</span>
              악기로 한 파트씩, <b>번호 순서대로</b> 참여하세요.
            </span>
          </div>
        {/if}

        <!-- 파트 카드 -->
        {#if result.voices.length > 0}
          <div class="min-h-0 flex-1 overflow-y-auto">
            <div class="grid grid-cols-1 gap-3 sm:grid-cols-2 md:grid-cols-3 xl:grid-cols-4">
              {#each result.voices as voice, idx}
                {@const style = getRoleStyle(voice.name)}
                <article
                  class="flex flex-col gap-3 rounded-2xl border bg-gradient-to-br p-4 transition-all duration-300 {copiedIndex ===
                  idx
                    ? 'border-success/60 from-success/10 to-success/5 shadow-[0_0_24px_-6px_var(--color-success)]'
                    : style.card}"
                >
                  <div class="flex items-center justify-between gap-2">
                    <div class="flex min-w-0 items-center gap-1.5">
                      {#if mode !== "solo"}
                        <span
                          class="flex h-5 w-5 shrink-0 items-center justify-center rounded-full bg-base-300 text-[10px] font-bold"
                          title="파트 순서">{idx + 1}</span
                        >
                      {/if}
                      <span class="badge {style.badge} badge-sm font-medium">{voice.name}</span>
                    </div>
                    <span class="text-[11px] text-base-content/40">{voice.note_count}음표</span>
                  </div>

                  <button
                    type="button"
                    onclick={() => copyToClipboard(voice.content, idx)}
                    class="btn btn-block {copiedIndex === idx ? 'btn-success' : 'btn-primary'}"
                  >
                    {#if copiedIndex === idx}
                      ✓ 복사됨
                    {:else}
                      MML 복사
                    {/if}
                  </button>
                </article>
              {/each}
            </div>
          </div>
        {:else}
          <div class="alert alert-warning text-xs">
            <span>변환된 음표가 없습니다.</span>
          </div>
        {/if}
      </div>
    {/if}
  </main>
</div>

<style>
  /* 스플래시 로고: 은은하게 두둥실 + 부드러운 글로우 */
  .splash-logo {
    animation: splash-float 3s ease-in-out infinite;
    filter: drop-shadow(0 10px 26px rgba(124, 92, 240, 0.45));
  }
  @keyframes splash-float {
    0%,
    100% {
      transform: translateY(0);
    }
    50% {
      transform: translateY(-7px);
    }
  }
  /* 로딩 바: 그라데이션 막대가 좌우로 흐르는 인디케이터 */
  .splash-bar {
    animation: splash-slide 1.15s ease-in-out infinite;
  }
  @keyframes splash-slide {
    0% {
      transform: translateX(-110%);
    }
    100% {
      transform: translateX(360%);
    }
  }
  /* 움직임 줄이기 설정 존중 */
  @media (prefers-reduced-motion: reduce) {
    .splash-logo,
    .splash-bar {
      animation: none;
    }
  }
</style>
