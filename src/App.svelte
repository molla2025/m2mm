<script lang="ts">
  import { onMount } from "svelte"
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
  let charLimit = $state(2400)
  let mode = $state("solo") // "solo"(혼자 3) / "duo"(2인 4) / "ensemble"(합주 6)
  let errorMessage = $state("")
  let copiedIndex = $state(-1)
  let copyTimerId: number | null = null
  let isUpdating = $state(false)

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

    // Drag & Drop 이벤트
    const appWindow = getCurrentWindow()
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

  // 분석 결과로 추천 모드와 안내 문구를 만든다.
  // 웬만하면 단독을 추천하고, 단독이 확실히 버거운 경우(악기 많음 / 동시음 매우 많음)만 화음을 추천.
  function getRecommendation(a: MidiAnalysis): { text: string; warn: string } {
    if (a.instruments >= 5) {
      return {
        text: `악기가 ${a.instruments}종으로 많아요. 혼자서는 다 살리기 어려우니 6명이 한 음씩 맡는 화음 연주가 더 좋을 거예요. (단독·2인으로도 되지만 일부 악기가 빠져요.)`,
        warn:
          a.instruments > 6
            ? `악기가 ${a.instruments}종이나 돼서 화음(동시음 6개)에도 다 못 담아요. 중요한 악기 위주로만 변환되고 일부는 빠집니다.`
            : "",
      }
    }
    if (a.max_polyphony >= 12) {
      return {
        text: `동시에 울리는 음이 최대 ${a.max_polyphony}개로 아주 많아요. 혼자서는 빠지는 음이 많으니 화음 연주가 더 좋을 거예요. (베이스를 따로 둘 거면 2인도 괜찮아요.)`,
        warn: "",
      }
    }
    return {
      text: "혼자서 연주하기 좋은 곡이에요. 단독 연주를 추천해요. (베이스를 한 명이 더 받쳐주려면 2인도 좋아요.)",
      warn: "",
    }
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
    if (name.includes("금관") || name.includes("리드")) return ROLE_STYLES.warning
    if (name.includes("기타")) return ROLE_STYLES.error
    if (name.includes("파이프") || name.includes("오르간")) return ROLE_STYLES.accent
    if (name.includes("신스") || name.includes("앙상블")) return ROLE_STYLES.info
    return ROLE_STYLES.neutral
  }

  function fmtTime(seconds: number): string {
    const s = Math.floor(seconds)
    const m = Math.floor(s / 60)
    return m > 0 ? `${m}분 ${s % 60}초` : `${s % 60}초`
  }
</script>

<div class="flex h-screen flex-col bg-base-100 text-base-content overflow-hidden">
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
        <h1 class="text-sm font-bold leading-tight">M2MM</h1>
        <p class="text-[11px] text-base-content/45">MIDI → 모비노기 MML 변환기</p>
      </div>
    </div>
    <span class="badge badge-outline badge-sm text-base-content/50">v0.1.1</span>
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
                data-tip="모비노기 악보 한 칸의 글자 수 제한이에요 (현재 2,400자). 곡이 길면 이 한도에 맞춰 자동으로 잘립니다."
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
      <div class="grid h-full place-items-center p-6">
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

            <!-- 추천은 글로만 -->
            <div
              class="flex items-start gap-2 rounded-2xl border border-base-300 bg-base-100 p-3"
            >
              <span class="text-base leading-none">💡</span>
              <p class="text-xs leading-relaxed text-base-content/70">{rec.text}</p>
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
                  화음 연주 <span class="font-normal opacity-75">· 6명 (동시음 6개)</span>
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
              <span>
                {mode === "solo"
                  ? "혼자 연주"
                  : mode === "duo"
                    ? "2명 연주"
                    : `${result.voices.length}명 합주`}
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

        {#if mode === "duo" && result.voices.length > 1}
          <!-- 2인 안내 -->
          <div
            class="flex items-start gap-2 rounded-2xl border border-info/40 bg-info/5 px-4 py-2.5 text-[11px] leading-relaxed text-base-content/70"
          >
            <span class="text-sm leading-none">🎻</span>
            <span>
              <span class="font-semibold text-info">2인용</span>
              — 한 명이 <b>앞 파트(동시음 3개)</b>를 한꺼번에 맡고, 다른 한 명이 <b>마지막 베이스</b>를
              받쳐줘요.
            </span>
          </div>
        {:else if mode === "ensemble" && result.voices.length > 1}
          <!-- 합주 참여 순서 안내 -->
          <div
            class="flex items-start gap-2 rounded-2xl border border-info/40 bg-info/5 px-4 py-2.5 text-[11px] leading-relaxed text-base-content/70"
          >
            <span class="text-sm leading-none">🎻</span>
            <span>
              <span class="font-semibold text-info">{result.voices.length}명 합주용</span>
              — 각자 단음 악기로 한 파트씩 맡아요. 아래 <b>번호 순서대로</b> 1번이 먼저 시작하고,
              나머지가 차례로 합주에 참여하면 돼요.
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
