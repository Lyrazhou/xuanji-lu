# 《璇玑录》v260825D 生图模型兼容性研究记录

研究日期：2026-08-25（UTC+8）

## 结论

《璇玑录》的图片 Profile 不应向所有模型固定发送同一组参数。不同模型对 `size`、`quality`、`response_format`、`output_format` 和 `n` 的支持范围不同；因此 v260825D 采用“模型能力预设 + 手动覆写”的分层策略。

| 模型族 | 默认画幅 | 必填/建议参数 | 输出处理 |
|---|---|---|---|
| GPT Image（`gpt-image-*`，含 `gpt-image-2`） | `1536x1024` | 不发送 `response_format`；可用 `output_format: png`；保持单图 | 默认读取 `data[0].b64_json`，并将解码结果保存至本地 |
| DALL·E 3 | `1792x1024` | `n: 1`、`response_format: b64_json`、`quality: standard` | 读取 `data[0].b64_json`；兼容 URL 回退 |
| DALL·E 2 | `1024x1024` | `response_format: b64_json`、`quality: standard` | 读取 `data[0].b64_json`；兼容 URL 回退 |
| 通用 OpenAI 兼容服务 / 未识别模型 | `1024x1024` | `response_format: b64_json`；可由用户覆写 | 接受 Base64、URL，或含 data URI 的返回值 |

## 覆写规则

每个图片 Profile 将提供以下可选覆写字段：画幅尺寸、质量、输出格式和响应格式。只要用户填写，软件便把该值放在自动预设之后，优先发送用户值。原有“额外请求参数（JSON）”保持最高优先级，便于兼容特殊第三方服务。

自动预设只解决已知的标准模型；它不假设所有标有“OpenAI 兼容”的第三方服务都完整实现官方接口。当服务器仍返回 HTTP 错误时，界面将保留服务端原始错误摘要，方便用户在 Profile 中调整覆写参数。

## 依据

官方 Image API 说明指出：GPT Image 模型默认返回 Base64 图像且不支持 `response_format`；DALL·E 3 的合法尺寸为 `1024x1024`、`1792x1024` 或 `1024x1792`，且仅支持单图；GPT Image 使用 `1024x1024`、`1536x1024`、`1024x1536` 等尺寸，`gpt-image-2` 还支持符合规则的任意宽高组合。OpenAI 兼容服务文档也将 DALL·E 3 的标准尺寸和 `url` / `b64_json` 返回格式列为服务兼容要求。

来源：

1. https://developers.openai.com/api/reference/resources/images/methods/generate/
2. https://developers.openai.com/api/docs/guides/image-generation
3. https://github.com/QuantumNous/new-api-docs/blob/main/docs/en/api/openai-image.md
