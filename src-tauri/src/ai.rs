use std::collections::BTreeMap;

use std::time::{Duration, Instant};

use chrono::Utc;
use edge_tts_rust::{EdgeTtsClient, SpeakOptions};

use base64::Engine;
use reqwest::{header::{HeaderMap, HeaderName, HeaderValue}, Client};
use serde_json::{json, Value};

use crate::{models::*, storage::Storage};

fn extract_json(raw: &str) -> Result<Value, String> {
    let clean = raw.trim().trim_start_matches("```json").trim_start_matches("```").trim_end_matches("```").trim();
    let start = clean.find('{').ok_or_else(|| "模型没有返回 JSON 对象。".to_string())?;
    let end = clean.rfind('}').ok_or_else(|| "模型 JSON 不完整。".to_string())?;
    serde_json::from_str(&clean[start..=end]).map_err(|e| format!("模型 JSON 无法解析：{e}"))
}

fn headers(profile: &ModelProfile, secret: &str) -> Result<HeaderMap, String> {
    let mut map = HeaderMap::new();
    if !secret.trim().is_empty() {
        match profile.auth_type.as_str() {
            "bearer" => { map.insert("authorization", HeaderValue::from_str(&format!("Bearer {secret}")).map_err(|e|e.to_string())?); }
            "x-api-key" => { map.insert("x-api-key", HeaderValue::from_str(secret).map_err(|e|e.to_string())?); }
            _ => {}
        }
    }
    let extra: BTreeMap<String, String> = serde_json::from_str(&profile.extra_headers).unwrap_or_default();
    for (k,v) in extra { let name=HeaderName::from_bytes(k.as_bytes()).map_err(|_|format!("无效请求头：{k}"))?;let value=HeaderValue::from_str(&v).map_err(|_|format!("无效请求头值：{k}"))?;map.insert(name,value); }
    Ok(map)
}

fn redact_debug_value(value:&Value)->Value{
    match value {
        Value::Object(object)=>Value::Object(object.iter().map(|(key,value)|{let lowered=key.to_ascii_lowercase();let safe_value=if lowered.contains("api_key")||lowered.contains("apikey")||lowered.contains("authorization")||lowered.contains("token")||lowered.contains("secret")||lowered.contains("password"){Value::String("[REDACTED]".to_string())}else{redact_debug_value(value)};(key.clone(),safe_value)}).collect()),
        Value::Array(values)=>Value::Array(values.iter().map(redact_debug_value).collect()),
        _=>value.clone(),
    }
}

fn debug_request_body(body:&Value, include_content:bool)->Value{
    if include_content { return redact_debug_value(body); }
    json!({"model":body.get("model"),"parameterKeys":body.as_object().map(|object|object.keys().cloned().collect::<Vec<_>>()).unwrap_or_default()})
}

fn debug_endpoint(endpoint:&str)->&str { endpoint.split('?').next().unwrap_or(endpoint) }

fn debug_raw_response(raw:&str,include_content:bool)->Value{
    if !include_content { return Value::Null; }
    serde_json::from_str::<Value>(raw).map(|value|redact_debug_value(&value)).unwrap_or_else(|_|Value::String("[非 JSON 响应正文已省略，以免记录潜在敏感内容]".to_string()))
}

async fn request_json(storage:&Storage,profile:&ModelProfile, secret:&str, body:Value)->Result<Value,String>{
    let timeout_seconds = profile.timeout_seconds.clamp(10, 1800) as u64;
    let include_content=storage.prefs().map(|prefs|prefs.debug_log_content).unwrap_or(false);
    let request_body=debug_request_body(&body,include_content);
    let base_event=json!({"timestamp":Storage::now(),"profileName":profile.name,"capability":profile.capability,"protocol":profile.protocol,"endpoint":debug_endpoint(profile.endpoint.trim()),"model":profile.model,"timeoutSeconds":timeout_seconds});
    storage.append_debug_log(&json!({"event":"request","meta":base_event.clone(),"body":request_body}));
    let started=Instant::now();
    let client=Client::builder().timeout(std::time::Duration::from_secs(timeout_seconds)).build().map_err(|e|e.to_string())?;
    let response=match client.post(profile.endpoint.trim()).headers(headers(profile,secret)?).json(&body).send().await {
        Ok(response)=>response,
        Err(e)=>{let timed_out=e.is_timeout();let error=if timed_out{format!("模型请求在 {} 秒后超时。请在该 Profile 中调高“请求超时”，或检查模型服务状态。",timeout_seconds)}else{format!("网络请求失败：{e}")};let log_error=if timed_out{"模型请求超时。".to_string()}else{"网络请求失败；传输错误详情未写入日志。".to_string()};storage.append_debug_log(&json!({"event":"error","meta":base_event.clone(),"elapsedMs":started.elapsed().as_millis(),"error":log_error}));return Err(error)}
    };
    let status=response.status();
    let raw=match response.text().await { Ok(raw)=>raw,Err(e)=>{let error=format!("读取模型响应失败：{e}");storage.append_debug_log(&json!({"event":"error","meta":base_event.clone(),"elapsedMs":started.elapsed().as_millis(),"status":status.as_u16(),"error":"读取模型响应失败；详细错误未写入日志。"}));return Err(error)} };
    if !status.is_success(){let error=format!("模型服务返回 HTTP {}：{}",status,raw.chars().take(400).collect::<String>());storage.append_debug_log(&json!({"event":"error","meta":base_event.clone(),"elapsedMs":started.elapsed().as_millis(),"status":status.as_u16(),"error":format!("模型服务返回 HTTP {}。服务端错误摘要未写入日志。",status),"response":debug_raw_response(&raw,include_content)}));return Err(error)}
    match serde_json::from_str(&raw){
        Ok(value)=>{storage.append_debug_log(&json!({"event":"response","meta":base_event.clone(),"elapsedMs":started.elapsed().as_millis(),"status":status.as_u16(),"response":if include_content{redact_debug_value(&value)}else{json!({"topLevelKeys":value.as_object().map(|object|object.keys().cloned().collect::<Vec<_>>()).unwrap_or_default()})}}));Ok(value)}
        Err(e)=>{let error=format!("模型服务没有返回 JSON：{e}");storage.append_debug_log(&json!({"event":"error","meta":base_event.clone(),"elapsedMs":started.elapsed().as_millis(),"status":status.as_u16(),"error":"模型服务没有返回 JSON；解析错误详情未写入日志。","response":debug_raw_response(&raw,include_content)}));Err(error)}
    }
}

fn chat_content(response:&Value)->Result<String,String>{
    response.pointer("/choices/0/message/content").and_then(Value::as_str)
        .or_else(||response.pointer("/output_text").and_then(Value::as_str))
        .or_else(||response.pointer("/content/0/text").and_then(Value::as_str))
        .map(ToString::to_string).ok_or_else(||"未从模型响应中找到文本内容。请确认 Profile 使用 OpenAI Chat Completions 兼容接口。".to_string())
}

fn profile_for(storage:&Storage, id:&str, capability:&str)->Result<(ModelProfile,String),String>{let profile=storage.profile(id)?;if profile.capability!=capability{return Err(format!("所选 Profile 属于 {} 能力，不能用于 {}。",profile.capability,capability))}let secret=storage.profile_secret(&profile.id)?;Ok((profile,secret))}

pub async fn chat(storage:&Storage, profile_id:&str, system:&str, user:&str)->Result<String,String>{
    let (profile,secret)=profile_for(storage,profile_id,"text")?;
    if profile.protocol!="openai-chat"{return Err("当前版本仅支持文字 Profile 的 OpenAI Chat Completions 兼容协议。".to_string())}
    let mut body=json!({"model":profile.model,"messages":[{"role":"system","content":system},{"role":"user","content":user}],"temperature":0.78});
    if let Value::Object(extra)=serde_json::from_str::<Value>(&profile.extra_params).unwrap_or(json!({})) { if let Value::Object(ref mut object)=body { for (k,v) in extra {object.insert(k,v);} } }
    chat_content(&request_json(storage,&profile,&secret,body).await?)
}

pub async fn interpret_divination(storage:&Storage, profile_id:&str, question:&str, dice_context:&str)->Result<AiResult,String>{
    let system="你是一位温柔而专业的占星骰子解读师。用户会给出一个占卜问题和三颗占星骰子的结果（行星骰、星座骰、宫骰）。请给出温暖、有洞察、贴近问题情境的解读，避免空泛套话。只输出一个合法的 JSON 对象，不要输出任何其他文字、不要用代码块包裹，结构如下：{\"diceReadings\":[{\"dice\":\"行星骰\",\"face\":\"实际骰面\",\"glyph\":\"对应符号\",\"meaning\":\"2-4句\"},{\"dice\":\"星座骰\",\"face\":\"实际骰面\",\"glyph\":\"对应符号\",\"meaning\":\"…\"},{\"dice\":\"宫骰\",\"face\":\"实际骰面\",\"glyph\":\"对应符号\",\"meaning\":\"…\"}],\"summary\":\"4-8句综合分析与具体建议\",\"imagePrompt\":\"A one-sentence English prompt for a mystical 16:9 astrology artwork inspired by this reading, deep blue-violet night sky, golden starlight, symbolic elements of the three dice, dreamy sacred atmosphere\"}。diceReadings 必须依次为行星骰、星座骰、宫骰；face 与 glyph 必须使用实际骰面；每段须结合问题；summary 是重点；imagePrompt 必须英文且描绘画面。";
    let raw=chat(storage,profile_id,system,&format!("我的问题：{question}\n三颗骰子结果：{dice_context}")).await?;
    let value=extract_json(&raw)?;
    let reading=DivinationReading{dice_readings:value.get("diceReadings").and_then(|x|x.as_array()).map(|items|items.iter().map(|x|DiceReadingCard{dice_label:x.get("dice").and_then(Value::as_str).unwrap_or_default().to_string(),face_name:x.get("face").and_then(Value::as_str).unwrap_or_default().to_string(),face_glyph:x.get("glyph").and_then(Value::as_str).unwrap_or_default().to_string(),meaning:x.get("meaning").and_then(Value::as_str).unwrap_or_default().to_string()}).collect()).unwrap_or_default(),summary:value.get("summary").and_then(Value::as_str).unwrap_or(&raw).to_string(),image_prompt:value.get("imagePrompt").and_then(Value::as_str).unwrap_or_default().to_string()};
    Ok(AiResult{reading,raw_text:raw})
}

const TRAINING_TOPIC_SCOPES: [&str; 10] = ["恋爱与亲密关系", "友情与人际相处", "家庭与居住生活", "职场与职业发展", "学业与能力成长", "金钱与消费规划", "身心状态与生活习惯", "兴趣创作与自我表达", "出行、旅行与环境变化", "个人选择与人生方向"];

fn training_scope_instruction(storage: &Storage) -> String {
    let prefs = match storage.prefs() { Ok(prefs) => prefs, Err(_) => return String::new() };
    if prefs.training_topic_scopes.is_empty() { return String::new(); }
    let mut scopes = prefs.training_topic_scopes.iter().filter(|scope| TRAINING_TOPIC_SCOPES.contains(&scope.as_str())).cloned().collect::<Vec<_>>();
    if prefs.training_topic_scopes.iter().any(|scope| scope == "其它") && !prefs.training_custom_scope.trim().is_empty() { scopes.push(format!("其它（{}）", prefs.training_custom_scope.trim())); }
    if scopes.is_empty() { return String::new(); }
    let last_question = storage.list_training().ok().and_then(|records| records.into_iter().next()).map(|record| record.question).unwrap_or_default();
    let avoid = if last_question.trim().is_empty() { String::new() } else { format!("上一道训练题是“{}”；请尽量选择不同的已选范围，避免连续两题属于同一范围。", last_question.chars().take(80).collect::<String>()) };
    format!("本次训练题的范围仅可从以下已选主题中随机选择：{}。题目须具体、贴近日常生活，不要在题干中标注主题名称。{}", scopes.join("、"), avoid)
}

pub async fn generate_training_question(storage:&Storage, profile_id:&str)->Result<Value,String>{
    let dice=storage.roll_local_dice()?;
    let scope_instruction=training_scope_instruction(storage);
    let system=format!("你是占星骰子训练场的出题师。软件已经通过本地操作系统随机源抽取三颗占星骰子，骰面不可由你生成、替换或改写。请依据给定的实际三骰，生成一道贴近日常生活、15 至 40 字的占卜训练问题。{}只输出合法 JSON：{{\"question\":\"15-40字问题\"}}。不输出其他文字，也不要输出骰面字段。",scope_instruction);
    let user=format!("本地已确定的实际三骰：行星骰为 {}；星座骰为 {}；宫骰为 {}。请只生成与此练习相配的问题。",dice.planet,dice.zodiac,dice.house);
    let raw=chat(storage,profile_id,&system,&user).await?;
    let value=extract_json(&raw)?;
    let question=value.get("question").and_then(Value::as_str).map(str::trim).filter(|question|!question.is_empty()).ok_or_else(||"模型没有返回训练问题。".to_string())?;
    Ok(json!({"question":question,"planet":dice.planet,"zodiac":dice.zodiac,"house":dice.house}))
}

pub async fn grade_training(storage:&Storage, profile_id:&str, question:&str, dice_context:&str, answer:&str)->Result<TrainingComparison,String>{
    let system="你是一位温柔而专业的占星骰子导师。用户正在训练场练习解盘：给出一个练习问题、三颗骰子结果，以及用户自己写的解读。请为每颗骰子给出参考解读，从用户的解读中提取他对每颗骰子的理解做逐维度对照，最后给出整体点评，并对用户本次解读给出百分制分数。只输出一个合法 JSON 对象：{\"rows\":[{\"dice\":\"行星骰\",\"userReading\":\"1-2句，未提及则空\",\"refReading\":\"2-3句\"},{\"dice\":\"星座骰\",\"userReading\":\"…\",\"refReading\":\"…\"},{\"dice\":\"宫骰\",\"userReading\":\"…\",\"refReading\":\"…\"}],\"highlights\":\"1-2句\",\"improvements\":\"1-2句\",\"overallComment\":\"2-4句\",\"score\":85}。rows 顺序固定为行星骰、星座骰、宫骰，点评鼓励且具体。score 必须是 1 至 100 的整数；100 分仅表示三颗骰子均有准确、连贯且贴合问题的完整解读，不要为鼓励而虚高分数。";
    let raw=chat(storage,profile_id,system,&format!("练习问题：{question}\n三颗骰子结果：{dice_context}\n我的解读：{answer}")).await?;let value=extract_json(&raw)?;
    let score=value.get("score").and_then(Value::as_i64).filter(|score|(1..=100).contains(score)).unwrap_or(0) as i32;
    Ok(TrainingComparison{rows:value.get("rows").and_then(Value::as_array).map(|rows|rows.iter().map(|x|ComparisonRow{dice_label:x.get("dice").and_then(Value::as_str).unwrap_or_default().to_string(),user_reading:x.get("userReading").and_then(Value::as_str).unwrap_or_default().to_string(),ref_reading:x.get("refReading").and_then(Value::as_str).unwrap_or_default().to_string()}).collect()).unwrap_or_default(),highlights:value.get("highlights").and_then(Value::as_str).unwrap_or_default().to_string(),improvements:value.get("improvements").and_then(Value::as_str).unwrap_or_default().to_string(),overall_comment:value.get("overallComment").and_then(Value::as_str).unwrap_or(&raw).to_string(),score})
}

fn put_if_not_empty(object:&mut serde_json::Map<String,Value>, key:&str, value:&str){ if !value.trim().is_empty(){object.insert(key.to_string(),Value::String(value.trim().to_string()));} }

fn image_request_body(profile:&ModelProfile,prompt:&str)->Value{
    let model=profile.model.trim().to_ascii_lowercase();
    let (default_size, default_quality, default_output_format, default_response_format)=if model.contains("dall-e-3"){
        ("1792x1024", "standard", "", "b64_json")
    } else if model.contains("dall-e-2"){
        ("1024x1024", "standard", "", "b64_json")
    } else if model.contains("gpt-image-"){
        ("1024x1024", "", "", "")
    } else {
        ("1024x1024", "", "", "b64_json")
    };
    let mut object=serde_json::Map::new();
    object.insert("model".to_string(),Value::String(profile.model.trim().to_string()));
    object.insert("prompt".to_string(),Value::String(prompt.to_string()));
    object.insert("n".to_string(),Value::Number(1.into()));
    put_if_not_empty(&mut object,"size",if profile.image_size_override.trim().is_empty(){default_size}else{&profile.image_size_override});
    put_if_not_empty(&mut object,"quality",if profile.image_quality_override.trim().is_empty(){default_quality}else{&profile.image_quality_override});
    put_if_not_empty(&mut object,"output_format",if profile.image_output_format_override.trim().is_empty(){default_output_format}else{&profile.image_output_format_override});
    put_if_not_empty(&mut object,"response_format",if profile.image_response_format_override.trim().is_empty(){default_response_format}else{&profile.image_response_format_override});
    if let Value::Object(extra)=serde_json::from_str::<Value>(&profile.extra_params).unwrap_or(json!({})){for(k,v) in extra{object.insert(k,v);}}
    Value::Object(object)
}

fn image_base64(item:&Value)->Option<&str>{
    item.get("b64_json").and_then(Value::as_str)
        .or_else(||item.get("b64Json").and_then(Value::as_str))
        .or_else(||item.get("base64").and_then(Value::as_str))
        .or_else(||item.get("image_base64").and_then(Value::as_str))
}

fn image_extension(bytes:&[u8])->&'static str{
    if bytes.starts_with(&[0x89,b'P',b'N',b'G']) { "png" }
    else if bytes.starts_with(&[0xff,0xd8,0xff]) { "jpg" }
    else if bytes.starts_with(b"RIFF") && bytes.get(8..12)==Some(b"WEBP") { "webp" }
    else { "png" }
}

fn safe_file_component(value:&str)->String{
    let cleaned=value.chars().filter_map(|character|{
        if character.is_alphanumeric(){Some(character)}
        else if character.is_whitespace(){Some('_')}
        else{None}
    }).collect::<String>();
    let compact=cleaned.trim_matches('_').chars().take(18).collect::<String>();
    if compact.is_empty(){"星语".to_string()}else{compact}
}

fn image_file_stem(record_id:&str,mode:&str,question:&str)->String{
    let now=Utc::now();
    let date=now.format("%Y%m%d-%H%M%S");
    let month=now.format("%Y%m");
    let mode=safe_file_component(mode);
    let question=safe_file_component(question);
    let record=safe_file_component(record_id).chars().take(8).collect::<String>();
    format!("{month}/{date}_{mode}_{question}_{record}")
}

fn archive_image_bytes(storage:&Storage,file_stem:&str,bytes:&[u8])->Result<String,String>{
    if bytes.len()<32{return Err("生图服务返回的图片内容为空或不完整。".to_string())}
    let path=storage.write_local_image(&format!("{file_stem}.{}",image_extension(bytes)),bytes)?;
    Ok(path.to_string_lossy().to_string())
}

async fn archive_remote_image(storage:&Storage,url:&str,timeout_seconds:u64,file_stem:&str)->Result<String,String>{
    let client=Client::builder().timeout(std::time::Duration::from_secs(timeout_seconds)).build().map_err(|e|e.to_string())?;
    let response=client.get(url).send().await.map_err(|e|format!("无法下载生图服务返回的图片链接：{e}"))?;
    let status=response.status();
    if !status.is_success(){return Err(format!("生图服务返回的图片链接下载失败：HTTP {status}"))}
    let bytes=response.bytes().await.map_err(|e|format!("读取生成图片失败：{e}"))?;
    archive_image_bytes(storage,file_stem,&bytes)
}

fn is_safe_voice_short_name(value: &str) -> bool { !value.is_empty() && value.len() <= 120 && value.chars().all(|character| character.is_ascii_alphanumeric() || character == '-') }

const EDGE_TTS_ATTEMPTS: usize = 3;

fn edge_tts_client() -> Result<EdgeTtsClient, String> {
    EdgeTtsClient::builder()
        .connect_timeout(Duration::from_secs(20))
        .receive_timeout(Duration::from_secs(75))
        .ws_pool_size(0)
        .ws_warmup(false)
        .request_chunk_reuse(false)
        .build()
        .map_err(|error| format!("初始化 Edge 在线语音失败：{error}"))
}

fn concise_edge_tts_error(error: impl std::fmt::Display) -> String {
    let raw = error.to_string();
    raw.split('?').next().unwrap_or(&raw).trim_end_matches('(').trim().to_string()
}

async fn retry_pause(attempt: usize) {
    if attempt < EDGE_TTS_ATTEMPTS { tokio::time::sleep(Duration::from_millis(500 * attempt as u64)).await; }
}

pub async fn list_edge_tts_voices(storage: &Storage) -> Result<Vec<TtsVoice>, String> {
    let mut last_error = String::new();
    let mut result = None;
    for attempt in 1..=EDGE_TTS_ATTEMPTS {
        let client = edge_tts_client()?;
        match client.list_voices().await {
            Ok(voices) => { result = Some(voices); break; }
            Err(error) => { last_error = concise_edge_tts_error(error); retry_pause(attempt).await; }
        }
    }
    let mut voices = result.ok_or_else(|| format!("读取 Microsoft Edge 在线音色列表失败（已自动尝试 {EDGE_TTS_ATTEMPTS} 次）：{last_error}。请检查本机网络、代理或安全软件是否允许连接 Microsoft Edge 服务，并确认 Windows 系统时间已自动同步。"))?.into_iter().filter_map(|voice| {
        if !voice.locale.eq_ignore_ascii_case("zh-CN") || !is_safe_voice_short_name(&voice.short_name) { return None; }
        let display_name = voice.friendly_name.clone().unwrap_or_else(|| voice.name.clone());
        Some(TtsVoice { short_name:voice.short_name, display_name, local_name:voice.name, gender:voice.gender, locale:voice.locale, locale_name:"简体中文（中国）".to_string() })
    }).collect::<Vec<_>>();
    voices.sort_by(|left, right| left.local_name.cmp(&right.local_name).then(left.short_name.cmp(&right.short_name)));
    if voices.is_empty() { return Err("Microsoft Edge 在线语音未返回可用的简体中文（zh-CN）音色。请检查网络连接后重试。".to_string()); }
    storage.append_debug_log(&json!({"event":"edge-tts-voices","meta":{"timestamp":Storage::now(),"provider":"Microsoft Edge online speech","voiceCount":voices.len(),"attempts":EDGE_TTS_ATTEMPTS}}));
    Ok(voices)
}

pub async fn synthesize_edge_tts(storage: &Storage, voice_short_name: &str, text: &str) -> Result<TtsAudio, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() { return Err("没有可朗读的文字。".to_string()); }
    if trimmed.chars().count() > 4000 { return Err("单次语音朗读最多支持 4,000 个字符，请缩短本次内容。".to_string()); }
    if !is_safe_voice_short_name(voice_short_name) { return Err("所选 Microsoft Edge 在线音色名称无效。请重新读取音色列表后选择。".to_string()); }
    let started = Instant::now();
    let mut last_error = String::new();
    let mut result = None;
    for attempt in 1..=EDGE_TTS_ATTEMPTS {
        let client = edge_tts_client()?;
        match client.synthesize(trimmed, SpeakOptions { voice: voice_short_name.to_string(), ..Default::default() }).await {
            Ok(audio) => { result = Some(audio); break; }
            Err(error) => { last_error = concise_edge_tts_error(error); retry_pause(attempt).await; }
        }
    }
    let result = result.ok_or_else(|| format!("Microsoft Edge 在线语音合成失败（已自动尝试 {EDGE_TTS_ATTEMPTS} 次）：{last_error}。请检查本机网络、代理或安全软件是否允许 WebSocket 连接，并确认 Windows 系统时间已自动同步。"))?;
    if result.audio.len() < 64 { return Err("Microsoft Edge 在线语音返回的音频内容为空或不完整。".to_string()); }
    storage.append_debug_log(&json!({"event":"edge-tts-response","meta":{"timestamp":Storage::now(),"provider":"Microsoft Edge online speech","voice":voice_short_name,"elapsedMs":started.elapsed().as_millis(),"audioBytes":result.audio.len(),"attempts":EDGE_TTS_ATTEMPTS}}));
    Ok(TtsAudio { audio_base64:base64::engine::general_purpose::STANDARD.encode(result.audio), mime_type:"audio/mpeg".to_string(), file_extension:"mp3".to_string() })
}

pub async fn generate_image(storage:&Storage, profile_id:&str, prompt:&str, record_id:&str, mode:&str, question:&str)->Result<String,String>{
    let (profile,secret)=profile_for(storage,profile_id,"image")?;if profile.protocol!="openai-images"{return Err("当前版本仅支持生图 Profile 的 OpenAI Images 兼容协议。".to_string())}
    let timeout_seconds=profile.timeout_seconds.clamp(10,1800) as u64;
    let file_stem=image_file_stem(record_id,mode,question);
    let response=request_json(storage,&profile,&secret,image_request_body(&profile,prompt)).await?;let item=response.pointer("/data/0").ok_or_else(||"生图响应中没有 data[0]。请检查该 Profile 的服务地址、模型名称及图片输出参数。".to_string())?;
    if let Some(url)=item.get("url").and_then(Value::as_str){return archive_remote_image(storage,url,timeout_seconds,&file_stem).await}
    if let Some(encoded)=image_base64(item){let encoded=encoded.strip_prefix("data:image/png;base64,").or_else(||encoded.strip_prefix("data:image/jpeg;base64,")).or_else(||encoded.strip_prefix("data:image/webp;base64,")).unwrap_or(encoded);let bytes=base64::engine::general_purpose::STANDARD.decode(encoded).map_err(|e|format!("图片 Base64 无效：{e}"))?;return archive_image_bytes(storage,&file_stem,&bytes)}
    Err("生图响应中没有 url、b64_json 或兼容的 Base64 字段。请检查该 Profile 的响应格式覆写。".to_string())
}

#[cfg(test)]
mod image_profile_tests {
    use super::*;

    fn profile(model: &str) -> ModelProfile {
        ModelProfile { model: model.to_string(), ..Default::default() }
    }

    #[test]
    fn dall_e_three_uses_legal_landscape_defaults() {
        let body = image_request_body(&profile("dall-e-3"), "test");
        assert_eq!(body.get("size").and_then(Value::as_str), Some("1792x1024"));
        assert_eq!(body.get("quality").and_then(Value::as_str), Some("standard"));
        assert_eq!(body.get("response_format").and_then(Value::as_str), Some("b64_json"));
        assert_eq!(body.get("n").and_then(Value::as_i64), Some(1));
    }

    #[test]
    fn gpt_image_avoids_dalle_only_response_format() {
        let body = image_request_body(&profile("gpt-image-2"), "test");
        assert_eq!(body.get("size").and_then(Value::as_str), Some("1024x1024"));
        assert!(body.get("response_format").is_none());
        assert!(body.get("quality").is_none());
    }

    #[test]
    fn image_profile_overrides_take_priority() {
        let mut image_profile = profile("dall-e-3");
        image_profile.image_size_override = "1024x1024".to_string();
        image_profile.image_quality_override = "hd".to_string();
        image_profile.image_response_format_override = "url".to_string();
        let body = image_request_body(&image_profile, "test");
        assert_eq!(body.get("size").and_then(Value::as_str), Some("1024x1024"));
        assert_eq!(body.get("quality").and_then(Value::as_str), Some("hd"));
        assert_eq!(body.get("response_format").and_then(Value::as_str), Some("url"));
    }

    #[test]
    fn debug_content_redacts_nested_credentials() {
        let source = json!({"api_key":"top-secret","nested":{"token":"nested-secret","authorization":"Bearer secret","ok":"visible"},"items":[{"password":"hidden"}]});
        let redacted = redact_debug_value(&source);
        assert_eq!(redacted.pointer("/api_key").and_then(Value::as_str), Some("[REDACTED]"));
        assert_eq!(redacted.pointer("/nested/token").and_then(Value::as_str), Some("[REDACTED]"));
        assert_eq!(redacted.pointer("/nested/authorization").and_then(Value::as_str), Some("[REDACTED]"));
        assert_eq!(redacted.pointer("/nested/ok").and_then(Value::as_str), Some("visible"));
        assert_eq!(redacted.pointer("/items/0/password").and_then(Value::as_str), Some("[REDACTED]"));
    }

    #[test]
    fn debug_log_omits_non_json_response_body() {
        let logged = debug_raw_response("Bearer top-secret", true);
        assert_eq!(logged.as_str(), Some("[非 JSON 响应正文已省略，以免记录潜在敏感内容]"));
    }

    #[test]
    fn image_magic_numbers_choose_safe_extensions() {
        assert_eq!(image_extension(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a]), "png");
        assert_eq!(image_extension(&[0xff, 0xd8, 0xff, 0xe0]), "jpg");
        assert_eq!(image_extension(b"RIFF1234WEBPVP8 "), "webp");
    }

    #[test]
    fn image_file_stem_groups_by_month_and_keeps_safe_context() {
        let stem = image_file_stem("12345678-abcdef", "poster", "我是否应接受这份工作？");
        assert!(stem.starts_with(&format!("{}/", Utc::now().format("%Y%m"))));
        assert!(stem.contains("_poster_"));
        assert!(stem.contains("我是否应接受这份工作"));
        assert!(stem.ends_with("_12345678"));
        assert!(!stem.contains('？'));
    }

    #[test]
    fn edge_tts_errors_hide_query_parameters() {
        let message = concise_edge_tts_error("http error: request failed (https://example.invalid/voices/list?trustedclienttoken=private-value&Sec-MS-GEC=derived-value)");
        assert!(message.contains("https://example.invalid/voices/list"));
        assert!(!message.contains("trustedclienttoken"));
        assert!(!message.contains("derived-value"));
    }
}
