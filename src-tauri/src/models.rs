use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DiceResult {
    pub planet: String,
    pub zodiac: String,
    pub house: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DiceReadingCard {
    pub dice_label: String,
    pub face_name: String,
    pub face_glyph: String,
    pub meaning: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DivinationReading {
    pub dice_readings: Vec<DiceReadingCard>,
    pub summary: String,
    pub image_prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct DivinationRecord {
    pub id: String,
    pub uid: String,
    pub created: String,
    pub updated: String,
    pub question: String,
    pub dice: DiceResult,
    pub reading: Option<DivinationReading>,
    pub summary_text: String,
    pub image_url: String,
    pub review_note: String,
    pub rating: i32,
    pub tag_names: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ComparisonRow {
    pub dice_label: String,
    pub user_reading: String,
    pub ref_reading: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TrainingComparison {
    pub rows: Vec<ComparisonRow>,
    pub highlights: String,
    pub improvements: String,
    pub overall_comment: String,
    #[serde(default)]
    pub score: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct TrainingRecord {
    pub id: String,
    pub uid: String,
    pub created: String,
    pub updated: String,
    pub question: String,
    pub dice: Option<DiceResult>,
    pub dice_text: String,
    pub user_answer: String,
    pub ai_compare_text: String,
    pub ai_compare: Option<TrainingComparison>,
    #[serde(default)]
    pub ai_score: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct GalleryItem {
    pub id: String,
    pub uid: String,
    pub created: String,
    pub updated: String,
    pub image_url: String,
    pub mode: String,
    pub question: String,
    pub dice_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Tag {
    pub id: String,
    pub name: String,
    pub color: String,
    pub meaning: String,
    pub created: String,
    pub updated: String,
}

fn default_timeout_seconds() -> i64 { 300 }

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ModelProfile {
    pub id: String,
    pub name: String,
    pub capability: String,
    pub protocol: String,
    pub endpoint: String,
    pub model: String,
    pub auth_type: String,
    pub extra_headers: String,
    pub extra_params: String,
    #[serde(default = "default_timeout_seconds")]
    pub timeout_seconds: i64,
    #[serde(default)]
    pub image_size_override: String,
    #[serde(default)]
    pub image_quality_override: String,
    #[serde(default)]
    pub image_output_format_override: String,
    #[serde(default)]
    pub image_response_format_override: String,
    pub has_secret: bool,
    pub created: String,
    pub updated: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ModelProfileInput {
    pub id: Option<String>,
    pub name: String,
    pub capability: String,
    pub protocol: String,
    pub endpoint: String,
    pub model: String,
    pub auth_type: String,
    pub extra_headers: String,
    pub extra_params: String,
    #[serde(default = "default_timeout_seconds")]
    pub timeout_seconds: i64,
    #[serde(default)]
    pub image_size_override: String,
    #[serde(default)]
    pub image_quality_override: String,
    #[serde(default)]
    pub image_output_format_override: String,
    #[serde(default)]
    pub image_response_format_override: String,
    pub api_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Preferences {
    pub active_text_profile_id: String,
    pub active_image_profile_id: String,
    #[serde(default)]
    pub tts_voice_short_name: String,
    pub image_dice_prompt: String,
    pub image_poster_prompt: String,
    pub image_custom_prompt: String,
    #[serde(default)]
    pub debug_logging_enabled: bool,
    #[serde(default)]
    pub debug_log_content: bool,
    #[serde(default)]
    pub last_image_save_directory: String,
    #[serde(default)]
    pub last_audio_save_directory: String,
    #[serde(default)]
    pub training_topic_scopes: Vec<String>,
    #[serde(default)]
    pub training_custom_scope: String,
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AppSnapshot {
    pub divination_records: Vec<DivinationRecord>,
    pub training_records: Vec<TrainingRecord>,
    pub gallery_items: Vec<GalleryItem>,
    pub tags: Vec<Tag>,
    pub profiles: Vec<ModelProfile>,
    pub prefs: Preferences,
    pub data_directory: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ExchangePackage {
    pub format: String,
    pub version: i32,
    pub source: String,
    pub app_version: String,
    pub exported_at: String,
    pub profile: ExchangeProfile,
    #[serde(default)]
    pub divination_records: Vec<DivinationRecord>,
    #[serde(default)]
    pub training_records: Vec<TrainingRecord>,
    #[serde(default)]
    pub tags: Vec<ExchangeTag>,
    #[serde(default)]
    pub gallery_items: Vec<GalleryItem>,
    pub prefs: Option<ExchangePrefs>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ExchangeProfile {
    pub user_id: String,
    pub user_label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ExchangeTag {
    pub name: String,
    pub color: String,
    pub meaning: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExchangePrefs {
    pub text_model: String,
    pub image_model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MergeSummary {
    pub inserted: i32,
    pub updated: i32,
    pub skipped: i32,
    pub tags_created: i32,
    pub images_copied: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AiResult {
    pub reading: DivinationReading,
    pub raw_text: String,
}
