mod ai;
mod exchange;
mod models;
mod storage;

use std::{fs, path::Path};

use base64::{engine::general_purpose::STANDARD, Engine};
use models::*;
use storage::Storage;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

pub struct AppState { storage: Storage }

fn state_error<T>(result: Result<T, String>) -> Result<T, String> { result }

#[tauri::command]
fn load_snapshot(state: State<'_, AppState>) -> Result<AppSnapshot, String> { state.storage.snapshot() }

#[tauri::command]
fn save_divination(state: State<'_, AppState>, record: DivinationRecord) -> Result<DivinationRecord, String> { state.storage.save_divination(record) }

#[tauri::command]
fn update_divination_meta(state: State<'_, AppState>, id: String, review_note: String, rating: i32, tag_names: Vec<String>) -> Result<(), String> { state.storage.update_divination_meta(&id, &review_note, rating, tag_names) }

#[tauri::command]
fn save_training(state: State<'_, AppState>, record: TrainingRecord) -> Result<TrainingRecord, String> { state.storage.save_training(record) }

#[tauri::command]
fn delete_record(state: State<'_, AppState>, kind: String, id: String) -> Result<(), String> { state.storage.delete_record(&kind, &id) }

#[tauri::command]
fn save_tag(state: State<'_, AppState>, tag: Tag) -> Result<Tag, String> { state.storage.save_tag(tag) }

#[tauri::command]
fn delete_tag(state: State<'_, AppState>, id: String) -> Result<(), String> { state.storage.delete_tag(&id) }

#[tauri::command]
fn save_profile(state: State<'_, AppState>, profile: ModelProfileInput) -> Result<ModelProfile, String> { state.storage.save_profile(profile) }

#[tauri::command]
fn delete_profile(state: State<'_, AppState>, id: String) -> Result<(), String> { state.storage.delete_profile(&id) }

#[tauri::command]
fn save_prefs(state: State<'_, AppState>, prefs: Preferences) -> Result<Preferences, String> { state.storage.save_prefs(prefs) }

#[tauri::command]
async fn interpret_divination(state: State<'_, AppState>, profile_id: String, question: String, dice_context: String) -> Result<AiResult, String> { ai::interpret_divination(&state.storage, &profile_id, &question, &dice_context).await }

#[tauri::command]
fn roll_local_dice(state: State<'_, AppState>) -> Result<DiceResult, String> { state.storage.roll_local_dice() }

#[tauri::command]
async fn generate_training_question(state: State<'_, AppState>, profile_id: String) -> Result<serde_json::Value, String> { ai::generate_training_question(&state.storage, &profile_id).await }

#[tauri::command]
async fn grade_training(state: State<'_, AppState>, profile_id: String, question: String, dice_context: String, answer: String) -> Result<TrainingComparison, String> { ai::grade_training(&state.storage, &profile_id, &question, &dice_context, &answer).await }

#[tauri::command]
async fn list_edge_tts_voices(state: State<'_, AppState>) -> Result<Vec<TtsVoice>, String> { ai::list_edge_tts_voices(&state.storage).await }

#[tauri::command]
async fn synthesize_edge_tts(state: State<'_, AppState>, voice_short_name: String, text: String) -> Result<TtsAudio, String> { ai::synthesize_edge_tts(&state.storage, &voice_short_name, &text).await }

#[tauri::command]
fn save_tts_audio(destination_path: String, audio_base64: String) -> Result<(), String> {
    let bytes = STANDARD.decode(audio_base64).map_err(|error| format!("语音音频数据无效：{error}"))?;
    if bytes.len() < 64 || bytes.len() > 40 * 1024 * 1024 { return Err("语音音频文件大小无效。".to_string()); }
    let destination = Path::new(&destination_path);
    if let Some(parent) = destination.parent() { fs::create_dir_all(parent).map_err(|error| error.to_string())?; }
    fs::write(destination, bytes).map_err(|error| format!("另存语音失败：{error}"))
}

#[tauri::command]
async fn generate_image(state: State<'_, AppState>, profile_id: String, prompt: String, record_id: String, mode: String, question: String) -> Result<String, String> { ai::generate_image(&state.storage, &profile_id, &prompt, &record_id, &mode, &question).await }

#[tauri::command]
fn attach_generated_image(state: State<'_, AppState>, record_id: String, image_url: String, mode: String, dice_text: String) -> Result<(), String> {
    let record = state.storage.list_divinations()?.into_iter().find(|r| r.id == record_id).ok_or_else(|| "未找到关联占卜记录。".to_string())?;
    state.storage.attach_divination_image(&record_id, &image_url)?;
    let now = Storage::now();
    state.storage.save_gallery(GalleryItem { id:String::new(), uid:String::new(), created:now.clone(), updated:now, image_url, mode, question:record.question, dice_text })?;
    Ok(())
}

#[tauri::command]
fn copy_local_image_to(state: State<'_, AppState>, source_path: String, destination_path: String) -> Result<(), String> {
    if !state.storage.path_is_inside_images(&source_path) { return Err("只能另存已归档在本地图库目录中的图片。".to_string()); }
    let source = Path::new(&source_path);
    if !source.is_file() { return Err("本地图库图片文件不存在。".to_string()); }
    let destination = Path::new(&destination_path);
    if let Some(parent) = destination.parent() { fs::create_dir_all(parent).map_err(|e| e.to_string())?; }
    fs::copy(source, destination).map_err(|e| format!("另存图片失败：{e}"))?;
    Ok(())
}

fn local_image_data_url(bytes: &[u8]) -> Result<String, String> {
    let mime = if bytes.starts_with(&[0x89, b'P', b'N', b'G']) { "image/png" }
        else if bytes.starts_with(&[0xff, 0xd8, 0xff]) { "image/jpeg" }
        else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") { "image/webp" }
        else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") { "image/gif" }
        else if bytes.starts_with(b"BM") { "image/bmp" }
        else if bytes.get(4..12) == Some(b"ftypavif") || bytes.get(4..12) == Some(b"ftypavis") { "image/avif" }
        else { return Err("本地图库文件格式无法被应用内查看器识别。".to_string()) };
    Ok(format!("data:{mime};base64,{}", STANDARD.encode(bytes)))
}

#[tauri::command]
fn read_local_image_data_url(state: State<'_, AppState>, source_path: String) -> Result<String, String> {
    let source = state.storage.verified_local_image_path(&source_path)?;
    let bytes = fs::read(&source).map_err(|error| format!("读取本地图库图片失败：{error}"))?;
    local_image_data_url(&bytes)
}

#[tauri::command]
fn open_local_image(app: AppHandle, state: State<'_, AppState>, source_path: String) -> Result<(), String> {
    let source = state.storage.verified_local_image_path(&source_path)?;
    app.opener().open_path(source.to_string_lossy().to_string(), None::<&str>).map_err(|error| format!("无法调用系统默认图片查看器：{error}"))
}

#[tauri::command]
fn import_exchange(state: State<'_, AppState>, path: String) -> Result<MergeSummary, String> { exchange::import_package(&state.storage, Path::new(&path)) }

#[tauri::command]
fn export_exchange(state: State<'_, AppState>, path: String, zip_output: bool) -> Result<String, String> { exchange::export_package(&state.storage, Path::new(&path), zip_output) }

#[tauri::command]
fn export_divination_markdown(state: State<'_, AppState>, id: String, path: String) -> Result<(), String> {
    let record = state.storage.list_divinations()?.into_iter().find(|r|r.id==id).ok_or_else(||"未找到占卜记录。".to_string())?;
    fs::write(path, exchange::divination_markdown(&record)).map_err(|e|e.to_string())
}

#[tauri::command]
fn export_training_markdown(state: State<'_, AppState>, id: String, path: String) -> Result<(), String> {
    let record = state.storage.list_training()?.into_iter().find(|r|r.id==id).ok_or_else(||"未找到训练案例。".to_string())?;
    fs::write(path, exchange::training_markdown(&record)).map_err(|e|e.to_string())
}

#[tauri::command]
fn export_divinations_csv(state: State<'_, AppState>, ids: Vec<String>, path: String) -> Result<(), String> {
    let records=state.storage.list_divinations()?.into_iter().filter(|r|ids.is_empty()||ids.contains(&r.id)).collect::<Vec<_>>();
    fs::write(path, exchange::records_csv(&records)).map_err(|e|e.to_string())
}

#[tauri::command]
fn export_divinations_html(state: State<'_, AppState>, ids: Vec<String>, path: String) -> Result<(), String> {
    let records=state.storage.list_divinations()?.into_iter().filter(|r|ids.is_empty()||ids.contains(&r.id)).collect::<Vec<_>>();
    fs::write(path, exchange::html_export(&records)).map_err(|e|e.to_string())
}

#[tauri::command]
fn backup_database(state: State<'_, AppState>) -> Result<String, String> { Ok(state.storage.backup_database("manual")?.to_string_lossy().to_string()) }

#[tauri::command]
fn initialize_archive_keep_assets(state: State<'_, AppState>) -> Result<(), String> { state.storage.initialize_archive(false, false) }

#[tauri::command]
fn initialize_archive_delete_images(state: State<'_, AppState>) -> Result<(), String> { state.storage.initialize_archive(true, false) }

#[tauri::command]
fn initialize_archive_delete_profiles(state: State<'_, AppState>) -> Result<(), String> { state.storage.initialize_archive(false, true) }

#[tauri::command]
fn initialize_archive_delete_all(state: State<'_, AppState>) -> Result<(), String> { state.storage.initialize_archive(true, true) }

pub fn run() {
    let storage = Storage::open().expect("无法初始化《璇玑录》本地数据目录");
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(AppState { storage })
        .invoke_handler(tauri::generate_handler![
            load_snapshot, save_divination, update_divination_meta, save_training, delete_record,
            save_tag, delete_tag, save_profile, delete_profile, save_prefs, interpret_divination,
            roll_local_dice, generate_training_question, grade_training, list_edge_tts_voices, synthesize_edge_tts, save_tts_audio, generate_image, attach_generated_image, copy_local_image_to, read_local_image_data_url, open_local_image,
            import_exchange, export_exchange, export_divination_markdown, export_training_markdown, export_divinations_csv,
            export_divinations_html, backup_database, initialize_archive_keep_assets, initialize_archive_delete_images,
            initialize_archive_delete_profiles, initialize_archive_delete_all
        ])
        .run(tauri::generate_context!())
        .expect("运行《璇玑录》时发生致命错误");
}

#[cfg(test)]
mod tests {
    use super::models::ExchangePackage;

    #[test]
    fn local_image_data_url_detects_supported_formats() {
        assert!(super::local_image_data_url(&[0x89, b'P', b'N', b'G', 1]).unwrap().starts_with("data:image/png;base64,"));
        assert!(super::local_image_data_url(&[0xff, 0xd8, 0xff, 0xe0, 1]).unwrap().starts_with("data:image/jpeg;base64,"));
        assert!(super::local_image_data_url(b"RIFF1234WEBPVP8 ").unwrap().starts_with("data:image/webp;base64,"));
        assert!(super::local_image_data_url(b"not-an-image").is_err());
    }

    #[test]
    fn parses_user_sde1_sample() {
        let raw = include_str!("../fixtures/sample-star-dice-exchange.json");
        let package: ExchangePackage = serde_json::from_str(raw).expect("用户提供的 SDE-1 样例应可解析");
        assert_eq!(package.format, "star-dice-exchange");
        assert_eq!(package.version, 1);
        assert_eq!(package.divination_records.len(), 2);
        assert_eq!(package.training_records.len(), 1);
        assert_eq!(package.gallery_items.len(), 1);
        assert_eq!(package.tags.len(), 2);
        assert_eq!(package.divination_records[0].dice.planet, "moon");
        assert_eq!(package.training_records[0].dice.as_ref().unwrap().house, "house-6");
    }
}
