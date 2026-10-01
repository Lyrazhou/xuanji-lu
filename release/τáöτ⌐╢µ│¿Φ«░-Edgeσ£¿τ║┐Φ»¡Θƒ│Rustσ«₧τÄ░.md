# Edge 在线语音 Rust 实现研究注记

《璇玑录》计划使用 Rust 直连 Microsoft Edge 在线朗读语音，不携带 Python 运行时、不要求 Azure 资源 Key。该能力仍依赖网络；朗读文本会提交给 Microsoft 的在线语音服务，服务不是 Azure Speech 这种面向开发者的官方稳定 API。

## 候选 Rust 实现

`edge-tts-rust` 0.1.3（MIT）是一个 Rust 客户端，公开文档显示支持：异步语音合成、读取音色列表、`SpeakOptions` 中选择音色/语速/音量/音高，以及返回音频字节。其 `EdgeTtsClient` 具有 `list_voices()`、`synthesize()` 和 `save()` 方法，适合本项目将结果回传前端播放或由用户主动另存 MP3。

`kothok-edge-tts` 亦可复现 Edge Read Aloud WebSocket 协议，但其公开项目历史较短。其文档明确指出 Microsoft 会随 Edge 发布周期轮换协议令牌，过期后可能导致 HTTP 403，因此无论采用哪个库，都应将语音功能视为在线增强功能：出现连接、协议或限流错误时必须给出明确提示，不能影响占问、训练、档案等核心本地功能。

## 实施决策

本项目优先采用 `edge-tts-rust`，在 Rust 后端直接读取在线音色与合成 MP3。应用只保留 `tts_voice_short_name` 与音频另存路径等本地偏好；不保留 Azure Endpoint、区域、Profile 或 API Key。生成音频只在内存中交给前端播放，除非用户主动执行“另存”，不得写入历史记录、导出档案或调试正文。

## 来源

1. [edge-tts-rust 0.1.3 — crates.io](https://crates.io/crates/edge-tts-rust)
2. [edge-tts-rust API 文档 — docs.rs](https://docs.rs/edge-tts-rust/0.1.3/edge_tts_rust/)
3. [EdgeTtsClient API — docs.rs](https://docs.rs/edge-tts-rust/0.1.3/edge_tts_rust/struct.EdgeTtsClient.html)
4. [SpeakOptions API — docs.rs](https://docs.rs/edge-tts-rust/0.1.3/edge_tts_rust/struct.SpeakOptions.html)
5. [kothok-edge-tts — docs.rs](https://docs.rs/kothok-edge-tts)
6. [edge-tts — GitHub](https://github.com/rany2/edge-tts)
