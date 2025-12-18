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

  let isDragging = $state(false)
  let isConverting = $state(false)
  let result = $state<ConversionResult | null>(null)
  let fileName = $state("")
  let conversionMode = $state("normal")
  let charLimit = $state(2400)
  let errorMessage = $state("")
  let copiedIndex = $state(-1)

  // localStorage에 저장 (상태 변경시 자동 실행)
  $effect.pre(() => {
    if (typeof window !== "undefined") {
      localStorage.setItem("conversionMode", conversionMode)
      localStorage.setItem("charLimit", charLimit.toString())
    }
  })

  onMount(() => {
    // localStorage 불러오기
    const savedMode = localStorage.getItem("conversionMode")
    const savedLimit = localStorage.getItem("charLimit")

    if (savedMode) conversionMode = savedMode
    if (savedLimit) charLimit = parseInt(savedLimit, 10)

    // Drag & Drop 이벤트
    const appWindow = getCurrentWindow()
    appWindow.onDragDropEvent(event => {
      if (event.payload.type === "drop") {
        isDragging = false
        handleFileDrop(event.payload.paths)
      } else if (event.payload.type === "enter") {
        isDragging = true
      } else if (event.payload.type === "leave") {
        isDragging = false
      } else if (event.payload.type === "over") {
        isDragging = true
      }
    })
  })

  async function handleFileSelect() {
    try {
      const selected = await open({
        multiple: false,
        filters: [
          {
            name: "MIDI",
            extensions: ["mid", "midi"],
          },
        ],
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
    await convertFile(filePath)
  }

  async function convertFile(filePath: string) {
    isConverting = true
    errorMessage = ""
    result = null

    try {
      const fs = await import("@tauri-apps/plugin-fs")
      const bytes = await fs.readFile(filePath)

      const conversionResult = await invoke<ConversionResult>("convert_midi", {
        midiData: Array.from(bytes),
        options: {
          mode: conversionMode,
          char_limit: charLimit,
          compress_mode: false,
        },
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
    copiedIndex = index
    navigator.clipboard.writeText(content)
    setTimeout(() => {
      copiedIndex = -1
    }, 1500)
  }

  function reset() {
    result = null
    fileName = ""
    errorMessage = ""
    copiedIndex = -1
  }
</script>

<div
  class="h-screen flex flex-col bg-gradient-to-br from-slate-900 via-slate-950 to-black text-slate-50 overflow-hidden"
>
  <!-- Header -->
  <header
    class="px-4 py-3 flex flex-col sm:flex-row sm:items-center sm:justify-between gap-2 border-b border-slate-700/30 backdrop-blur-xl bg-slate-900/80 flex-shrink-0"
  >
    <div class="flex items-center gap-3">
      <div
        class="w-8 h-8 rounded-full bg-gradient-to-br from-sky-400 to-indigo-500 flex items-center justify-center text-lg shadow-lg shadow-indigo-500/30"
      >
        🎵
      </div>
      <div>
        <h1 class="text-base font-semibold">M2M</h1>
        <p class="text-[11px] text-slate-400">MIDI to MML Converter</p>
      </div>
    </div>
    <div
      class="flex items-center justify-between sm:justify-end gap-3 text-[10px] text-slate-500"
    >
      <span>Contact: molla202512@gmail.com</span>
      <span class="px-2 py-0.5 rounded-full border border-slate-600/50"
        >v1.0.0</span
      >
    </div>
  </header>

  <!-- Main content -->
  <main class="flex-1 min-h-0 p-3">
    <div
      class="h-full flex flex-col md:grid md:grid-cols-[minmax(0,360px)_minmax(0,1fr)] gap-3"
    >
      {#if !result}
        <!-- 설정 섹션 -->
        <section
          class="rounded-2xl bg-gradient-to-br from-slate-800/50 to-slate-900/50 border border-slate-700/30 p-3 shadow-2xl shadow-slate-950/60"
        >
          <h2 class="text-sm font-semibold mb-3">변환 옵션</h2>
          <div class="flex flex-col gap-3">
            <div class="flex flex-col gap-1.5">
              <label for="mode" class="text-xs text-slate-400">변환 모드</label>
              <select
                id="mode"
                class="select select-bordered select-sm bg-slate-900/90 border-slate-600/60 text-slate-200 text-xs focus:border-sky-400 focus:outline-none"
                bind:value={conversionMode}
              >
                <option value="normal">일반 변환</option>
                <option value="instrument">악기별 변환</option>
              </select>
            </div>
            <div class="flex flex-col gap-1.5">
              <label for="charlimit" class="text-xs text-slate-400"
                >악보 글자 수</label
              >
              <input
                id="charlimit"
                type="number"
                class="input input-bordered input-sm bg-slate-900/90 border-slate-600/60 text-slate-200 text-xs focus:border-sky-400 focus:outline-none"
                bind:value={charLimit}
                min="500"
                max="5000"
                step="100"
              />
            </div>
          </div>
        </section>

        <!-- 드롭존 섹션 -->
        <section
          class="flex-1 rounded-2xl bg-gradient-to-br from-slate-800/50 to-slate-900/50 border border-slate-700/30 p-3 shadow-2xl shadow-slate-950/60 flex flex-col gap-3 min-h-0"
        >
          <button
            class="flex-1 rounded-2xl border-2 border-dashed border-slate-600/70 bg-slate-950/50 px-4 py-8 cursor-pointer text-slate-400 flex items-center justify-center transition-all duration-150 hover:border-sky-400 hover:-translate-y-0.5 active:translate-y-0 {isDragging
              ? 'border-sky-400 shadow-[0_0_0_1px_rgba(56,189,248,0.7)] bg-slate-900/70'
              : ''}"
            type="button"
            onclick={handleFileSelect}
          >
            {#if isConverting}
              <div class="text-center">
                <div
                  class="w-8 h-8 rounded-full border-3 border-slate-600/40 border-t-sky-400 animate-spin mx-auto mb-2"
                ></div>
                <p class="text-sm font-medium mb-1">변환 중...</p>
                <p class="text-xs text-slate-500">{fileName}</p>
              </div>
            {:else}
              <div class="text-center">
                <svg
                  class="w-10 h-10 mb-2.5 opacity-70 mx-auto"
                  fill="none"
                  stroke="currentColor"
                  viewBox="0 0 24 24"
                >
                  <path
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    stroke-width="2"
                    d="M7 16a4 4 0 01-.88-7.903A5 5 0 1115.9 6L16 6a5 5 0 011 9.9M15 13l-3-3m0 0l-3 3m3-3v12"
                  />
                </svg>
                <p class="text-sm font-medium mb-1">MIDI 파일 선택</p>
                <p class="text-xs text-slate-500">드롭 또는 클릭</p>
              </div>
            {/if}
          </button>

          {#if errorMessage}
            <div
              class="alert alert-error rounded-xl p-2.5 text-xs bg-red-500/10 border border-red-500/50 text-red-200"
            >
              <span>{errorMessage}</span>
            </div>
          {/if}
        </section>
      {:else}
        <!-- 결과 요약 섹션 -->
        <section
          class="rounded-2xl bg-gradient-to-br from-slate-800/50 to-slate-900/50 border border-slate-700/30 p-3 shadow-2xl shadow-slate-950/60"
        >
          <div class="flex justify-between items-center gap-2 mb-3">
            <div class="min-w-0 flex-1">
              <h2 class="text-sm font-semibold truncate">{fileName}</h2>
              <p class="text-xs text-slate-400 mt-0.5">변환 결과</p>
            </div>
          </div>

          <div class="flex flex-col gap-2 mb-3">
            <div
              class="rounded-full px-3 py-1.5 border border-slate-600/60 flex items-center justify-between bg-slate-900/90"
            >
              <span class="text-[11px] text-slate-400">BPM</span>
              <span class="text-xs font-medium">{result.bpm}</span>
            </div>
            <div
              class="rounded-full px-3 py-1.5 border border-slate-600/60 flex items-center justify-between bg-slate-900/90"
            >
              <span class="text-[11px] text-slate-400">음표 수</span>
              <span class="text-xs font-medium">{result.total_notes}개</span>
            </div>
            {#if result.voices.length > 0}
              {@const originalSeconds = Math.floor(result.original_duration)}
              {@const convertedSeconds = Math.floor(
                Math.max(...result.voices.map(v => v.duration)),
              )}
              {@const origMin = Math.floor(originalSeconds / 60)}
              {@const origSec = originalSeconds % 60}
              {@const convMin = Math.floor(convertedSeconds / 60)}
              {@const convSec = convertedSeconds % 60}
              {@const origTime =
                origMin > 0 ? `${origMin}분 ${origSec}초` : `${origSec}초`}
              {@const convTime =
                convMin > 0 ? `${convMin}분 ${convSec}초` : `${convSec}초`}
              <div
                class="rounded-full px-3 py-1.5 border border-slate-600/60 flex items-center justify-between bg-slate-900/90"
              >
                <span class="text-[11px] text-slate-400">원본 러닝타임</span>
                <span class="text-xs font-medium text-slate-300"
                  >{origTime}</span
                >
              </div>
              <div
                class="rounded-full px-3 py-1.5 border border-slate-600/60 flex items-center justify-between bg-slate-900/90 {originalSeconds ===
                convertedSeconds
                  ? 'border-green-500/50 bg-green-500/5'
                  : 'border-sky-500/50 bg-sky-500/5'}"
              >
                <span class="text-[11px] text-slate-400">변환 러닝타임</span>
                <span
                  class="text-xs font-medium {originalSeconds ===
                  convertedSeconds
                    ? 'text-green-400'
                    : 'text-sky-400'}">{convTime}</span
                >
              </div>
            {/if}
          </div>

          <button
            class="w-full py-2.5 rounded-xl text-sm font-medium bg-slate-800/60 border border-slate-600/50 text-slate-300 hover:bg-slate-700/70 hover:border-sky-400/50 hover:text-sky-300 transition-all duration-200 flex items-center justify-center gap-2"
            type="button"
            onclick={reset}
          >
            <svg
              class="w-4 h-4"
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"
              />
            </svg>
            다른 파일 변환
          </button>
        </section>

        <!-- 결과 리스트 섹션 -->
        <section
          class="flex-1 rounded-2xl bg-gradient-to-br from-slate-800/50 to-slate-900/50 border border-slate-700/30 p-3 shadow-2xl shadow-slate-950/60 min-h-0 flex flex-col"
        >
          {#if result.voices.length > 0}
            <div class="mb-3 pb-2 border-b border-slate-700/50">
              <h3 class="text-xs font-semibold text-slate-300">변환된 파트</h3>
            </div>
            <div class="overflow-y-auto min-h-0 flex-1">
              <div
                class="flex flex-col md:grid md:grid-cols-[repeat(auto-fill,minmax(220px,1fr))] gap-2 md:gap-3"
              >
                {#each result.voices as voice, idx}
                  <article
                    class="rounded-xl p-3 bg-slate-950/50 border border-slate-700/80 flex flex-col gap-2.5 h-fit relative {copiedIndex ===
                    idx
                      ? 'ring-2 ring-green-400/50 shadow-[0_0_20px_rgba(34,197,94,0.3)]'
                      : ''}"
                  >
                    <div class="flex justify-between items-start gap-2">
                      <div>
                        <h3 class="text-xs font-medium">{voice.name}</h3>
                        <p class="text-[11px] text-slate-500 mt-0.5">
                          {voice.note_count}개 음표 · {voice.char_count}자
                        </p>
                      </div>
                    </div>

                    <button
                      class="btn btn-primary btn-sm rounded-full text-xs font-medium w-full bg-gradient-to-r from-sky-400 to-indigo-500 border-0 text-slate-950 shadow-lg shadow-indigo-500/40 hover:opacity-95 active:translate-y-0.5 {copiedIndex ===
                      idx
                        ? 'bg-gradient-to-r from-green-400 to-emerald-500'
                        : ''}"
                      type="button"
                      onclick={() => copyToClipboard(voice.content, idx)}
                    >
                      {#if copiedIndex === idx}
                        ✓ 복사 완료!
                      {:else}
                        📋 MML 복사하기
                      {/if}
                    </button>
                  </article>
                {/each}
              </div>
            </div>
          {:else}
            <div
              class="alert alert-warning rounded-xl p-2.5 text-xs bg-amber-500/10 border border-amber-500/50 text-amber-100"
            >
              <span>변환된 음표가 없습니다.</span>
            </div>
          {/if}
        </section>
      {/if}
    </div>
  </main>
</div>
