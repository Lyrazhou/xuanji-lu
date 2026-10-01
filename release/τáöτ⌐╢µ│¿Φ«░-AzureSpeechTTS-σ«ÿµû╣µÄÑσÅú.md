# Azure Speech TTS 官方接口研究注记

本注记仅记录用于《璇玑录》TTS 集成的官方接口要点。

Azure Speech Text to Speech 的 REST 调用需要 Azure Speech 资源、资源 Key 与 Endpoint。音色列表可通过资源 Endpoint 的 `/tts/cognitiveservices/voices/list` 路径获取，使用 `Ocp-Apim-Subscription-Key` 认证。每个资源/区域可用的音色不同，因此应用应动态请求列表，不应硬编码完整音色集合。

实时合成使用区域 TTS Endpoint：`https://<region>.tts.speech.microsoft.com/cognitiveservices/v1`。请求需使用 SSML，包含 `Ocp-Apim-Subscription-Key`、`Content-Type: application/ssml+xml`、`X-Microsoft-OutputFormat` 与 `User-Agent` 请求头。实现选择 `audio-24khz-48kbitrate-mono-mp3` 作为便于 Windows WebView 播放与用户另存的输出格式。

Azure 的免费 F0 层仍需用户创建 Azure Speech 资源并提供其 Endpoint/区域/Key；应用不会预置、保存或导出任何用户 Key，而是沿用 Windows Credential Manager 保存凭据。

## 官方来源

1. [Text to speech REST API — Microsoft Learn](https://learn.microsoft.com/en-us/azure/ai-services/speech-service/rest-text-to-speech)
2. [Language and voice support for Azure Speech — Microsoft Learn](https://learn.microsoft.com/en-us/azure/ai-services/speech-service/language-support)
3. [Azure Speech pricing — Microsoft Azure](https://azure.microsoft.com/en-us/pricing/details/speech/)
