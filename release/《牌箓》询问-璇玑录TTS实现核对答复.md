# 《牌箓》关于 Edge TTS 实现的核对答复

以下内容以《璇玑录》当前源码和已完成的实时测试为准，可直接转交《牌箓》任务中的 Manus。

## 结论摘要

《璇玑录》确实采用 Rust 后端直接调用 `edge-tts-rust`，而不是 Node、Python、Azure Speech API 或 Windows 本机离线音色。应用层为每一次音色读取和语音合成创建独立客户端，并将连接池大小设为 0、关闭 WebSocket 预热和请求分片复用；语音合成失败时最多自动建立 3 次新连接。

合成结果不是临时文件路径，也不是前端直接访问的 URL，而是 Rust Tauri command 返回的 `TtsAudio`：音频内容为 Base64 字符串，类型为 `audio/mpeg`，扩展名为 `mp3`。前端在内存中将 Base64 解码为 `ArrayBuffer`，使用 Web Audio API 的 `decodeAudioData` 解码为 `AudioBuffer`，再通过 `AudioBufferSourceNode` 播放。只有用户主动点击“另存”时，才调用受限的 Rust command 将 Base64 写入用户选择的 MP3 路径。

## 逐项回答《牌箓》的四个问题

### 1. 是否使用 Rust 的 edge-tts-rust 直接建立独立 WebSocket 连接？

是。核心依赖为：

```toml
edge-tts-rust = "0.1.3"
```

《璇玑录》后端使用：

```rust
let client = EdgeTtsClient::builder()
    .connect_timeout(Duration::from_secs(20))
    .receive_timeout(Duration::from_secs(75))
    .ws_pool_size(0)
    .ws_warmup(false)
    .request_chunk_reuse(false)
    .build()?;
```

随后调用：

```rust
client.synthesize(
    text,
    SpeakOptions {
        voice: voice_short_name.to_string(),
        ..Default::default()
    },
).await
```

`ws_pool_size(0)` 的含义是每个客户端不使用复用的 WebSocket 连接。应用层的重试循环还会在每次失败后重新构造一个客户端，因此重试不是在同一条已失败连接上重复发送。

需要准确区分：`edge-tts-rust` 负责底层 Edge Read Aloud 协议、HTTPS 音色目录请求、WSS 握手和音频帧组装；《璇玑录》应用层负责客户端运输参数、重试、输入限制、Tauri IPC、内存播放和另存文件。

### 2. 合成音频如何从主进程传到前端？

路径是：

```text
Microsoft Edge 在线服务
  → edge-tts-rust 完成 WSS 合成
  → Rust 收集完整 MP3 字节
  → Base64 编码
  → Tauri invoke 返回 TtsAudio
  → 前端 atob 解码
  → Uint8Array / ArrayBuffer
  → AudioContext.decodeAudioData
  → AudioBufferSourceNode
  → AudioContext.destination
```

Rust 返回结构的实际语义为：

```rust
TtsAudio {
    audio_base64: BASE64_MP3,
    mime_type: "audio/mpeg".to_string(),
    file_extension: "mp3".to_string(),
}
```

前端没有把音频写入临时文件，也没有把音频保存进 SQLite、历史案例或 SDE-1。前端的 `prepared` 状态只缓存当前组件生命周期内、按“音色 + 文本”计算的内存结果。

### 3. 播放前是否等待完整音频生成？

是。《璇玑录》当前使用的是“先完整合成，后播放”模式。前端点击播放后，首先在用户点击同步链路中创建／恢复 `AudioContext`，随后调用 Tauri 的 `synthesize_edge_tts` command；该 command 内部等待 `client.synthesize(...).await` 完成，返回完整 MP3 Base64 后，前端再解码并开始播放。

关键前端逻辑为：

```typescript
const unlockAudio = async () => {
  if (!audioContextRef.current) audioContextRef.current = new AudioContext();
  if (audioContextRef.current.state === "suspended") {
    await audioContextRef.current.resume();
  }
  return audioContextRef.current;
};

const audio = await prepare();
const raw = atob(audio.audioBase64);
const bytes = new Uint8Array(raw.length);
for (let index = 0; index < raw.length; index += 1) {
  bytes[index] = raw.charCodeAt(index);
}
const buffer = await audioContext.decodeAudioData(bytes.buffer);
const source = audioContext.createBufferSource();
source.buffer = buffer;
source.connect(audioContext.destination);
source.start(0);
```

这里先解锁 `AudioContext` 再等待网络合成，是为避免“网络请求完成后才第一次调用 `Audio.play()`”触发 Windows WebView 自动播放策略。当前实现不是流式边合成边播放；如果《牌箓》要保留长文本分段，可以按段调用合成并在前端按顺序排队播放，但每段仍应遵守用户点击时先解锁音频上下文的原则。

### 4. 是否包含 muid Cookie、随机 ConnectionId、动态 Sec-MS-GEC、IPv4 优先、超时／重试？

| 机制 | 《璇玑录》当前状态 | 负责层级 |
|---|---|---|
| `muid` Cookie | 有。由 `edge-tts-rust` 为请求生成随机值，并通过 WebSocket Cookie 发送。 | 依赖库 |
| `ConnectionId` | 有。由依赖库按请求生成随机 UUID。 | 依赖库 |
| `trustedclienttoken` | 有，但它是库内的公开 Edge 客户端协议常量，不是用户密钥、Azure Key 或设备密钥。 | 依赖库 |
| `Sec-MS-GEC` | 有。依赖库按当前时间窗口和公开客户端常量动态计算 SHA-256，不是固定值，也不绑定电脑。 | 依赖库 |
| `Sec-MS-GEC-Version` | 有。由依赖库的协议常量提供，当前构建对应 Chromium／Edge 143 请求特征。 | 依赖库 |
| IPv4 优先 | **没有显式实现。**《璇玑录》没有设置 `IPV4_ONLY`、自定义 DNS 或强制 IPv4；连接地址解析和 IP 选择由底层网络栈／操作系统决定。 | 未显式指定 |
| 连接超时 | 有，20 秒。 | 《璇玑录》应用层客户端 builder |
| 接收超时 | 有，75 秒。 | 《璇玑录》应用层客户端 builder |
| 自动重试 | 有，最多 3 次，每次构造新客户端／新连接；失败后等待约 500 ms、1000 ms。 | 《璇玑录》应用层 |
| WebSocket 连接池 | 明确关闭，`ws_pool_size(0)`。 | 《璇玑录》应用层客户端 builder |
| 预热连接 | 明确关闭，`ws_warmup(false)`。 | 《璇玑录》应用层客户端 builder |
| 分片复用 | 明确关闭，`request_chunk_reuse(false)`。 | 《璇玑录》应用层客户端 builder |

## 长文本和“300 秒超时”的准确边界

《璇玑录》当前单次 Edge TTS 输入上限是 **4,000 个字符**，并在本次实现中采用完整合成后播放，而不是把长文本作为一个无限长度请求。`edge-tts-rust` 本身支持流式 API，但《璇玑录》没有把流式音频直接暴露到前端。

《璇玑录》中的“Profile 请求超时 300 秒”属于用户文字处理／生图 Profile 的通用配置，**不是当前 Edge TTS 的等待时间**。Edge TTS 使用专门的 20 秒连接超时、75 秒接收超时和 3 次独立连接重试。若《牌箓》要求保留 300 秒配置，应把它作为应用层语音任务的总体 deadline，再在其内部设置合理的单连接超时和分段重试，不建议直接让单个 WebSocket 无限等待 300 秒。

## 为什么《牌箓》的旧 Node 实现可能与《璇玑录》不同

《牌箓》排查报告确认，早期 `node-edge-tts` 版本只使用固定 `TrustedClientToken` 和旧的 Chromium 请求特征，缺少当前服务所需的动态 `Sec-MS-GEC`，服务端拒绝时可能表现为 403、`ECONNRESET` 或类似连接重置。该问题属于旧协议握手代际问题。

《璇玑录》当前 `edge-tts-rust 0.1.3` 已包含动态 `Sec-MS-GEC`、随机 `ConnectionId`、随机 `muid` 和更新的 Chromium 请求特征。因此，《璇玑录》自身若出现含 `Sec-MS-GEC` 的请求仍被 10054 强制关闭，更应继续排查新电脑的网络出口、代理、防火墙、安全软件的 HTTPS／WSS 检查、系统时间以及 Microsoft 对特定网络环境的临时限制；不能再解释为“只有固定 token、缺少动态 GEC”。

## 迁移时不可遗漏的工程要点

《牌箓》如果复用这条路线，建议先锁定一个已验证的 Rust 依赖版本并通过实时音色读取和中文合成测试，不要只复制前端按钮。后端应将音色列表读取失败和语音合成失败分开处理；设置页首启失败不能阻断已保存音色。前端应采用“点击时解锁 AudioContext → 等待合成 → 内存解码播放”的顺序，并且播放停止要保留对应的 `AudioBufferSourceNode` 引用。

另存文件应由 Rust 受限命令执行，接收用户选择的目标路径和 Base64 音频内容；音频只在用户主动另存时落盘。不要把 Base64 音频写进历史记录、数据库或导出包。日志只能记录服务名称、音色、耗时、字节数和重试次数，不能记录完整文本、请求 URL 查询参数、认证信息或音频内容。

## 参考来源

[1]: https://crates.io/crates/edge-tts-rust "edge-tts-rust v0.1.3 — crates.io"
[2]: https://raw.githubusercontent.com/initsnow/edge-tts-rust/master/src/protocol.rs "edge-tts-rust protocol.rs：ConnectionId、muid、Sec-MS-GEC 与请求头"
[3]: https://github.com/rany2/edge-tts/issues/290 "edge-tts issue #290：旧握手缺少 Sec-MS-GEC 的 403 案例"
