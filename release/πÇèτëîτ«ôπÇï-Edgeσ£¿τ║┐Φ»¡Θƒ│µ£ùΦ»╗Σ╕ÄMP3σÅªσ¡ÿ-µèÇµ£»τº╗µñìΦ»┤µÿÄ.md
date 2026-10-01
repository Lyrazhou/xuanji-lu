# 《牌箓》Edge 在线语音朗读与 MP3 另存：技术移植说明

> **文档目的：** 将《璇玑录》已实现的中文在线朗读、播放／停止、MP3 另存、音色选择与历史入口方案，移植到《牌箓》的 Windows 本地版。本文面向接手《牌箓》任务的开发代理，可作为实现规格、代码骨架与验收清单使用。
>
> **推荐技术路线：** Tauri 2 + React/TypeScript + Rust，使用 `edge-tts-rust = "0.1.3"` 直连 Microsoft Edge Read Aloud 在线语音协议。该包提供异步音色读取与合成接口，采用 MIT 许可证。[1] [2]

## 1. 目标与非目标

该功能应当让用户在《牌箓》中主动朗读需要理解的文本，例如抽牌解读、牌阵总评、学习／训练题目、AI 点评，以及已归档案例。用户在设置中选择微软提供的中文音色；点击扬声器图标开始播放，再次点击同一图标立刻停止；点击下载图标选择位置后另存为 MP3。音频只保留在当前内存中，除非用户明确点击另存，不得自动写入数据库、历史案例、导出包或备份。

本方案**不使用 Python**，不要求用户安装 Python 或运行本地服务；也**不要求用户填写 Azure Endpoint、Azure Key 或其他 TTS Key**。它不应复用或读取《牌箓》的 LLM／生图 Profile 密钥。它需要网络，且本质上依赖 Microsoft Edge 的在线朗读服务与非面向开发者承诺稳定性的协议实现；当服务或协议变化时，语音功能可能失效，但不应影响占卜、档案、图片或其他本地功能。

| 设计决策 | 移植要求 |
|---|---|
| 合成位置 | Rust 后端直接访问 Edge 在线语音；前端不得自行访问第三方语音服务。 |
| 音频生命周期 | 合成结果经 IPC 以 Base64 返回前端，仅在内存缓存；用户点击“另存”时才写成 MP3 文件。 |
| 配置持久化 | 仅保存所选 `shortName` 与上次音频另存目录；不保存音频二进制。 |
| 历史覆盖 | 历史详情提供播放和另存；查询列表的案例标题旁提供紧凑的直接播放／停止入口。 |
| 并发策略 | 可沿用《璇玑录》的“每个语音组件独立播放”策略；除非产品另有要求，不强行做全局单音频互斥。 |

## 2. 依赖与桌面配置

在 `src-tauri/Cargo.toml` 的依赖区加入以下依赖；如果《牌箓》已有 `base64`，无需重复添加。`edge-tts-rust` 的 `EdgeTtsClient` 提供 `list_voices()` 与 `synthesize()` 等核心能力。[1] [2]

```toml
[dependencies]
base64 = "0.22.1"
edge-tts-rust = "0.1.3"
```

Tauri 前端会在 WebView 中执行播放。内容安全策略必须保持最小授权，但需要允许内存媒体来源；Tauri 官方也强调 CSP 应仅允许应用实际需要且可信的来源。[3]

```json
{
  "app": {
    "security": {
      "csp": "default-src 'self' asset: http://asset.localhost; img-src 'self' asset: http://asset.localhost https: data: blob:; media-src 'self' data: blob:; connect-src 'self' https: http:; style-src 'self' 'unsafe-inline'; script-src 'self'"
    }
  }
}
```

如果《牌箓》现有 CSP 使用对象写法而非单行字符串，应只在其原有策略中增加 `media-src 'self' data: blob:`，不要为此放宽 `script-src`、`connect-src` 或新增通配域名。若软件读取本地牌面图片，原有 `asset:` / `http://asset.localhost` 许可应继续按现有安全范围维持。

## 3. 数据模型与本地偏好

建议在《牌箓》的 Preferences 或等效设置模型中增加以下两个字段，并对旧数据库／旧 JSON 使用默认空字符串兼容读取。字段名可按《牌箓》命名规范调整，但前后端 JSON 名称必须一致。

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Preferences {
    // ……《牌箓》既有字段……
    #[serde(default)]
    pub tts_voice_short_name: String,
    #[serde(default)]
    pub last_audio_save_directory: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TtsVoice {
    pub short_name: String,
    pub display_name: String,
    pub local_name: String,
    pub gender: String,
    pub locale: String,
    pub locale_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TtsAudio {
    pub audio_base64: String,
    pub mime_type: String,
    pub file_extension: String,
}
```

对于 SQLite `meta(key, value)` 风格的偏好存储，读取时使用空串默认值，保存时使用 UPSERT。至少应验证：保存 `zh-CN-XiaoxiaoNeural` 后，再次读取仍返回同一字符串。若《牌箓》的“初始化／清空 Profile”会清理设置，应同步清除 `tts_voice_short_name`，但不应删除用户已另存在任意目录的 MP3。

## 4. Rust 后端：音色读取与合成

### 4.1 服务层接口

新建或扩展 `src-tauri/src/ai.rs`／`tts.rs`。`shortName` 必须做输入校验，避免将不受控字符带入协议请求。以下代码是《璇玑录》的可移植核心；错误文案可以更换为《牌箓》的产品语言。

```rust
use std::time::Instant;
use base64::Engine;
use edge_tts_rust::{EdgeTtsClient, SpeakOptions};
use serde_json::json;

fn is_safe_voice_short_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 120
        && value.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

pub async fn list_edge_tts_voices(storage: &Storage) -> Result<Vec<TtsVoice>, String> {
    let client = EdgeTtsClient::new()
        .map_err(|e| format!("初始化 Edge 在线语音失败：{e}"))?;

    let mut voices = client
        .list_voices()
        .await
        .map_err(|e| format!("读取 Microsoft Edge 在线音色列表失败：{e}"))?
        .into_iter()
        .filter_map(|voice| {
            if !voice.locale.eq_ignore_ascii_case("zh-CN")
                || !is_safe_voice_short_name(&voice.short_name)
            {
                return None;
            }
            let display_name = voice.friendly_name.clone().unwrap_or_else(|| voice.name.clone());
            Some(TtsVoice {
                short_name: voice.short_name,
                display_name,
                local_name: voice.name,
                gender: voice.gender,
                locale: voice.locale,
                locale_name: "简体中文（中国）".to_string(),
            })
        })
        .collect::<Vec<_>>();

    voices.sort_by(|a, b| a.local_name.cmp(&b.local_name).then(a.short_name.cmp(&b.short_name)));
    if voices.is_empty() {
        return Err("Microsoft Edge 在线语音未返回可用的简体中文（zh-CN）音色。请检查网络后重试。".to_string());
    }

    // 如开启调试日志，只记录计数、耗时、音色名等元数据；不得记录朗读正文。
    storage.append_debug_log(&json!({
        "event": "edge-tts-voices",
        "meta": { "provider": "Microsoft Edge online speech", "voiceCount": voices.len() }
    }));
    Ok(voices)
}

pub async fn synthesize_edge_tts(
    storage: &Storage,
    voice_short_name: &str,
    text: &str,
) -> Result<TtsAudio, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err("没有可朗读的文字。".to_string());
    }
    if trimmed.chars().count() > 4000 {
        return Err("单次语音朗读最多支持 4,000 个字符，请缩短本次内容。".to_string());
    }
    if !is_safe_voice_short_name(voice_short_name) {
        return Err("所选中文音色名称无效。请重新读取音色列表后选择。".to_string());
    }

    let started = Instant::now();
    let client = EdgeTtsClient::new()
        .map_err(|e| format!("初始化 Edge 在线语音失败：{e}"))?;
    let result = client
        .synthesize(trimmed, SpeakOptions {
            voice: voice_short_name.to_string(),
            ..Default::default()
        })
        .await
        .map_err(|e| format!("Microsoft Edge 在线语音合成失败：{e}"))?;

    if result.audio.len() < 64 {
        return Err("Microsoft Edge 在线语音返回的音频内容为空或不完整。".to_string());
    }

    storage.append_debug_log(&json!({
        "event": "edge-tts-response",
        "meta": {
            "provider": "Microsoft Edge online speech",
            "voice": voice_short_name,
            "elapsedMs": started.elapsed().as_millis(),
            "audioBytes": result.audio.len()
        }
    }));

    Ok(TtsAudio {
        audio_base64: base64::engine::general_purpose::STANDARD.encode(result.audio),
        mime_type: "audio/mpeg".to_string(),
        file_extension: "mp3".to_string(),
    })
}
```

`edge-tts-rust` 还支持流式合成、速率／音量／音高与边界事件；《牌箓》第一期应只使用完整 MP3 合成，避免在功能首次迁移时引入分片播放、字幕同步或长文分段的额外复杂度。[1]

### 4.2 Tauri 命令层

命令应由 `State<AppState>` 取得存储对象，而不是把任意内部路径、密钥或网络配置暴露给前端。示例：

```rust
#[tauri::command]
async fn list_edge_tts_voices(state: State<'_, AppState>) -> Result<Vec<TtsVoice>, String> {
    ai::list_edge_tts_voices(&state.storage).await
}

#[tauri::command]
async fn synthesize_edge_tts(
    state: State<'_, AppState>,
    voice_short_name: String,
    text: String,
) -> Result<TtsAudio, String> {
    ai::synthesize_edge_tts(&state.storage, &voice_short_name, &text).await
}

#[tauri::command]
fn save_tts_audio(destination_path: String, audio_base64: String) -> Result<(), String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(audio_base64)
        .map_err(|e| format!("语音音频数据无效：{e}"))?;
    if bytes.len() < 64 || bytes.len() > 40 * 1024 * 1024 {
        return Err("语音音频文件大小无效。".to_string());
    }
    let destination = std::path::Path::new(&destination_path);
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(destination, bytes).map_err(|e| format!("另存语音失败：{e}"))
}
```

务必把三个命令加入 `tauri::generate_handler![...]`。不要将“自动保存音频”植入保存案例、同步、导出或备份命令；另存必须由用户在前端文件选择器中主动确认。

## 5. React 前端：正确的播放／停止实现

### 5.1 关键兼容性修复

**不要仅用下列旧写法：**

```ts
const audio = await invoke<TtsAudio>("synthesize_edge_tts", params);
const player = new Audio(`data:${audio.mimeType};base64,${audio.audioBase64}`);
await player.play();
```

该写法在“点击按钮 → 等待数秒在线合成 → `player.play()`”的链路中可能被 Windows WebView 的自动播放策略拒绝。文件可以另存且可被系统播放器播放，说明问题不在 MP3 内容，而在延迟后的播放调用不再被视为直接的用户手势。

正确做法是：**在点击事件刚开始时创建／恢复 `AudioContext`，再等待合成；合成返回后在已解锁的音频上下文中解码并播放。** 以下可作为通用 `VoiceOutput` 组件的核心实现。

```tsx
const audioContextRef = useRef<AudioContext | null>(null);
const sourceRef = useRef<AudioBufferSourceNode | null>(null);
const [playing, setPlaying] = useState(false);
const [prepared, setPrepared] = useState<{ key: string; audio: TtsAudio } | null>(null);

const stop = () => {
  try { sourceRef.current?.stop(); } catch { /* 已停止时会抛 InvalidStateError，可忽略 */ }
  sourceRef.current = null;
  setPlaying(false);
};

useEffect(() => () => {
  try { sourceRef.current?.stop(); } catch { /* 无需处理 */ }
  sourceRef.current = null;
  void audioContextRef.current?.close();
  audioContextRef.current = null;
}, []);

const unlockAudio = async () => {
  if (!audioContextRef.current) audioContextRef.current = new AudioContext();
  if (audioContextRef.current.state === "suspended") {
    await audioContextRef.current.resume();
  }
  return audioContextRef.current;
};

const prepare = async (): Promise<TtsAudio | null> => {
  if (!prefs.ttsVoiceShortName) {
    notice("请先在设置中读取并选择微软中文音色。", true);
    return null;
  }
  if (!text.trim()) {
    notice("没有可朗读的文字。", true);
    return null;
  }
  const key = `${prefs.ttsVoiceShortName}|${text}`;
  if (prepared?.key === key) return prepared.audio;

  setBusy("tts-synthesis");
  try {
    const audio = await invoke<TtsAudio>("synthesize_edge_tts", {
      voiceShortName: prefs.ttsVoiceShortName,
      text,
    });
    setPrepared({ key, audio });
    return audio;
  } catch (error) {
    notice(String(error), true);
    return null;
  } finally {
    setBusy("");
  }
};

const toggle = async () => {
  if (playing) {
    stop();
    return;
  }

  // 必须在用户 onClick 的开始阶段执行；不要移到 await prepare() 之后。
  let audioContext: AudioContext;
  try {
    audioContext = await unlockAudio();
  } catch {
    notice("当前系统无法初始化语音播放设备，请检查系统输出设备后重试。", true);
    return;
  }

  const audio = await prepare();
  if (!audio) return;

  try {
    const raw = atob(audio.audioBase64);
    const bytes = new Uint8Array(raw.length);
    for (let i = 0; i < raw.length; i += 1) bytes[i] = raw.charCodeAt(i);
    const buffer = new ArrayBuffer(bytes.length);
    new Uint8Array(buffer).set(bytes);
    const decoded = await audioContext.decodeAudioData(buffer);

    const source = audioContext.createBufferSource();
    source.buffer = decoded;
    source.connect(audioContext.destination);
    source.onended = () => {
      if (sourceRef.current === source) {
        sourceRef.current = null;
        setPlaying(false);
      }
    };
    sourceRef.current = source;
    source.start(0);
    setPlaying(true);
  } catch {
    stop();
    notice("语音音频无法在应用内解码或播放，请检查系统音频设备后重试。", true);
  }
};
```

组件应在合成期间禁用播放按钮，避免同一组件并发启动多个请求。缓存键必须同时包含 `voiceShortName` 和 `text`；一旦用户换音色或可朗读文本发生变化，旧缓存不可复用。完整组件与紧凑组件均复用同一 `toggle()`，避免列表与详情出现不同的播放行为。

### 5.2 MP3 另存

使用 Tauri dialog 的 `save()` 取得用户选择路径，再调用后端 `save_tts_audio`。文件名需要移除 Windows 不允许的字符，并记忆所选目录。示例：

```tsx
const name = `牌箓_${safeFileName(fileStem)}_${new Date()
  .toISOString().replace(/[-:.TZ]/g, "").slice(0, 14)}.mp3`;
const target = await save({
  defaultPath: prefs.lastAudioSaveDirectory
    ? `${prefs.lastAudioSaveDirectory.replace(/[\\/]+$/, "")}\\${name}`
    : name,
  filters: [{ name: "MP3 音频", extensions: ["mp3"] }],
});
if (!target) return;

await invoke("save_tts_audio", { destinationPath: target, audioBase64: audio.audioBase64 });
const directory = target.replace(/[\\/][^\\/]*$/, "");
if (directory && directory !== prefs.lastAudioSaveDirectory) {
  await setPrefs({ ...prefs, lastAudioSaveDirectory: directory });
}
```

仅在非紧凑组件显示下载图标。列表标题旁的快速入口只显示播放／停止，避免每行操作区过宽；用户需要另存时可打开详情。

## 6. 设置页：中文音色选择与“保存后为空”防护

设置页面初始化时，音色数组通常为空，因为在线列表尚未取得。若受控 `<select>` 的 `value` 已是历史保存的 short name、而 `options` 中没有对应项，控件会视觉显示为空。这并不必然代表后端保存失败。

必须同时实现以下三点：第一，保存 `ttsVoiceShortName`；第二，如果已有保存值，设置页挂载后自动请求 `list_edge_tts_voices`；第三，在网络未返回或离线时增加“已保存”回显 option。推荐逻辑如下：

```tsx
const [voices, setVoices] = useState<TtsVoice[]>([]);
const [loadingVoices, setLoadingVoices] = useState(false);

const loadVoices = async () => {
  setLoadingVoices(true);
  try {
    setVoices(await invoke<TtsVoice[]>("list_edge_tts_voices"));
  } catch (error) {
    notice(String(error), true);
  } finally {
    setLoadingVoices(false);
  }
};

useEffect(() => {
  if (prefs.ttsVoiceShortName) void loadVoices();
}, []);

const savedVoiceMissing = prefs.ttsVoiceShortName
  && !voices.some((voice) => voice.shortName === prefs.ttsVoiceShortName);

<select
  value={prefs.ttsVoiceShortName}
  onChange={(event) => void setPrefs({ ...prefs, ttsVoiceShortName: event.target.value })}
>
  <option value="">请选择中文音色</option>
  {savedVoiceMissing && (
    <option value={prefs.ttsVoiceShortName}>
      已保存：{prefs.ttsVoiceShortName}
    </option>
  )}
  {voices.map((voice) => (
    <option key={voice.shortName} value={voice.shortName}>
      {voice.localName || voice.displayName} · {voice.gender} · {voice.shortName}
    </option>
  ))}
</select>
```

界面需明示：朗读依赖网络；用户主动播放／下载时，当前朗读文本会发送到 Microsoft Edge 在线语音服务；不会发送《牌箓》的模型 Profile 密钥；该能力并非 Azure Speech 的官方稳定开发者 API。

## 7. 业务入口与朗读文本组装

不要将数据库原文整块盲目传给朗读。每个入口应组装结构化、可听懂的语句，并避免重复视觉标签、极长字段或隐私字段。文本以当前记录内容即时生成，**不写回记录**。

| 页面／模块 | 控件 | 建议朗读文本 |
|---|---|---|
| 即时抽牌结果 | 播放、停止、另存 | 用户问题、牌阵／牌面、逐张解读、总评。 |
| 学习／训练出题 | 播放、停止、另存 | 练习题目、当前牌组或牌阵。 |
| 学习／训练点评 | 播放、停止、另存 | 参考解读、评分、亮点、改进方向、整体点评。 |
| 历史抽牌详情 | 播放、停止、另存 | 该条历史问题、牌阵／牌面、逐项与总评。 |
| 历史训练详情 | 播放、停止、另存 | 题目、训练素材、评分与点评。 |
| 历史查询列表 | 紧凑播放、停止 | 与对应详情一致的主要内容，但无下载按钮。 |

列表内按钮应紧贴案例标题右侧，而不是挤进“查看／导出／删除”操作栏。紧凑模式只渲染扬声器／停止方块图标；详情模式渲染扬声器、下载图标和简短状态文本。建议普通图标按钮为 `42 × 42 px`，列表紧凑按钮为 `38 × 38 px`，均有 `aria-label` 和悬停 `title`。

```css
.voice-output { display:inline-flex; align-items:center; gap:10px; min-height:42px; margin-top:17px; }
.icon-action { width:42px; height:42px; padding:0; display:inline-flex; align-items:center; justify-content:center; border-radius:50%; }
.voice-output-compact { display:inline-flex; flex:0 0 auto; min-height:38px; margin:0 1px; }
.voice-output-compact .icon-action { width:38px; height:38px; }
.voice-play.playing { color:#fff0c1; background:rgba(146,45,67,.78); }
.voice-save { color:#a9cfea; }
```

## 8. 安全、隐私与日志约束

语音功能不应改变《牌箓》既有模型密钥策略。尤其要避免在前端调试输出、Rust `println!`、错误上报、数据库、导出档案、安装包、Markdown 说明或文本日志中出现 LLM／图片 Profile 密钥。TTS 无需用户 API Key，也不应借用用户的任意已有 Key。

| 数据／操作 | 允许方式 | 禁止方式 |
|---|---|---|
| 已选音色 | 本地偏好保存 short name。 | 将音色列表或音频二进制写入案例。 |
| 当前朗读文本 | 用户点击播放或另存时发送给 Edge 在线语音。 | 后台预读、自动批量合成、无提示上传历史全文。 |
| MP3 | 仅内存缓存；用户明确另存时写入其选择的路径。 | 自动保存到应用数据目录、随案例／导出包同步。 |
| 调试日志 | 最多记录时间、音色 short name、耗时、字节数、错误类别。 | 记录朗读正文、Base64 音频、密钥、完整请求头。 |

## 9. 测试、验收与发布

后端至少应有一项偏好回归测试：设置并保存 `tts_voice_short_name = "zh-CN-XiaoxiaoNeural"` 后重读仍相等。在线烟雾测试应在有网络的环境中执行一次：列出 `zh-CN` 音色，使用其中一项合成简短文本，并检查得到的 MP3 字节数大于 64。在线测试不可在无网 CI 中作为强制单元测试，以免构建不稳定。

| 验收项 | 预期结果 |
|---|---|
| 音色读取 | 设置页能列出 zh-CN 音色；离线重进页面时仍以“已保存：shortName”显示已有选择。 |
| 音色保存 | 选定音色、关闭／重启应用、重新进入设置后仍有同一选项。 |
| 即时页面播放 | 点击扬声器后等待合成，应用内直接出声；再次点击同一图标立刻停止。 |
| 历史列表播放 | 抽牌与训练两类列表的每条标题旁均有紧凑扬声器；可播放并可停止。 |
| 历史详情 | 两类详情均有播放／停止和 MP3 下载图标。 |
| 另存 | 文件选择器可取消；确认路径后写入 MP3；下次另存默认打开上一次目录。 |
| 数据边界 | 数据库、历史详情和导出包中没有 Base64 音频或自动生成的 MP3。 |
| 失败体验 | 无网、无音色、文本为空、文本超长、音频设备不可用时显示明确中文错误，不崩溃。 |

发布前至少执行 `pnpm build`、`cargo test` 与 Windows x64 Tauri 生产构建。首次交付建议在真实 Windows 环境验证一次“点击后在线合成等待数秒再播放”，这是避免自动播放策略回归的关键场景。完整安装包或补丁均不得把用户已有 `data` 目录作为清理目标。

## 10. 给《牌箓》开发代理的执行摘要

1. 先确认《牌箓》是否同为 Tauri 2 + React + Rust；若是，直接按本文的命令、数据模型与组件结构移植。
2. 用 `edge-tts-rust 0.1.3`，而不是 Python `edge-tts`、Azure Key 或服务端中转。
3. 后端只提供“列音色、合成 Base64 MP3、用户确认后另存”三项命令；合成文本上限 4,000 字符。
4. 前端用 `AudioContext` 在点击时解锁，绝不可等网络合成完成后才首次调用普通 `Audio.play()`。
5. 设置中保存 short name，并实现离线回显 option + 已保存时自动加载在线列表。
6. 为即时结果、训练／学习、历史详情和两类历史查询列表落实相应图标入口；列表只放紧凑播放，详情才放下载。
7. 任何情况下不得将音频写入历史、数据库、导出包或备份；不得处理或输出已有模型密钥。
8. 发版前完成构建、回归、在线烟雾与 Windows 实机播放验证，并在 UI 提示联网与文本发送边界。

## 参考资料

[1]: https://crates.io/crates/edge-tts-rust "edge-tts-rust 0.1.3 — crates.io"
[2]: https://docs.rs/edge-tts-rust/0.1.3/edge_tts_rust/ "edge-tts-rust 0.1.3 API — docs.rs"
[3]: https://v2.tauri.app/security/csp/ "Content Security Policy — Tauri v2 官方文档"
