use std::{fs, io::Write, path::{Path, PathBuf}};

use chrono::Utc;
use getrandom::u32 as secure_random_u32;
use keyring::Entry;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;
use uuid::Uuid;

use crate::models::*;

const LEGACY_APP_FOLDER: &str = "XuanjiLu";
const DATA_FOLDER: &str = "data";
const SECRET_SERVICE: &str = "璇玑录";
const DEFAULT_DICE_IMAGE_PROMPT: &str = "Create a premium 16:9 mystical astrology dice wallpaper. Deep indigo, obsidian black, muted blood-red lunar eclipse and antique gold starlight. Three premium jade and burnished-gold astrology dice inspired by {{dice}} float above an obsidian armillary sphere. Question atmosphere: {{question}}. Strictly no text, no letters, no numbers, no watermark.";
const DEFAULT_POSTER_IMAGE_PROMPT: &str = "Create a premium 16:9 cinematic mystical story poster based on this astrology dice divination. Question: {{question}}. Dice: {{dice}}. Star reading: {{reading}}. Blood-moon occultism, deep indigo, obsidian black, muted crimson, antique gold, elegant Chinese-inspired celestial symbolism. Strictly no text, no letters, no words, no typography, no watermark.";
const DEFAULT_CUSTOM_IMAGE_PROMPT: &str = "Create a premium 16:9 mystical image based on the following context. Question: {{question}}. Dice: {{dice}}. Star reading: {{reading}}. Use a restrained, cinematic blood-moon astrology style. Strictly no text, no letters, no numbers, no watermark.";
const SYSTEM_TAG_NAMES: [&str; 3] = ["自动", "手动", "图片"];
const PLANET_DICE: [&str; 12] = ["sun", "moon", "mercury", "venus", "mars", "jupiter", "saturn", "uranus", "neptune", "pluto", "north-node", "south-node"];
const ZODIAC_DICE: [&str; 12] = ["aries", "taurus", "gemini", "cancer", "leo", "virgo", "libra", "scorpio", "sagittarius", "capricorn", "aquarius", "pisces"];
const HOUSE_DICE: [&str; 12] = ["house-1", "house-2", "house-3", "house-4", "house-5", "house-6", "house-7", "house-8", "house-9", "house-10", "house-11", "house-12"];

pub struct Storage {
    pub base_dir: PathBuf,
    pub db_path: PathBuf,
}

impl Storage {
    pub fn open() -> Result<Self, String> {
        let legacy_root = Self::legacy_data_root()?;
        let root = Self::installation_data_root()?;
        Self::copy_legacy_data_if_needed(&legacy_root, &root)?;
        fs::create_dir_all(root.join("images")).map_err(|e| e.to_string())?;
        fs::create_dir_all(root.join("backups")).map_err(|e| e.to_string())?;
        let storage = Self { db_path: root.join("xuanji-lu.sqlite3"), base_dir: root };
        storage.migrate()?;
        storage.rewrite_legacy_image_paths(&legacy_root)?;
        Ok(storage)
    }

    fn legacy_data_root() -> Result<PathBuf, String> {
        dirs::data_local_dir()
            .or_else(dirs::data_dir)
            .ok_or_else(|| "无法确定旧版本机数据目录".to_string())
            .map(|path| path.join(LEGACY_APP_FOLDER))
    }

    fn installation_data_root() -> Result<PathBuf, String> {
        let executable = std::env::current_exe().map_err(|error| format!("无法确定软件所在目录：{error}"))?;
        let executable_dir = executable.parent().ok_or_else(|| "无法确定软件所在目录。".to_string())?;
        Ok(executable_dir.join(DATA_FOLDER))
    }

    fn copy_directory(source: &Path, destination: &Path) -> Result<(), String> {
        fs::create_dir_all(destination).map_err(|error| error.to_string())?;
        for entry in fs::read_dir(source).map_err(|error| error.to_string())? {
            let entry = entry.map_err(|error| error.to_string())?;
            let source_path = entry.path();
            let destination_path = destination.join(entry.file_name());
            if source_path.is_dir() { Self::copy_directory(&source_path, &destination_path)?; }
            else if source_path.is_file() { fs::copy(&source_path, &destination_path).map_err(|error| error.to_string())?; }
        }
        Ok(())
    }

    fn copy_legacy_data_if_needed(legacy_root: &Path, root: &Path) -> Result<(), String> {
        if root.join("xuanji-lu.sqlite3").exists() || !legacy_root.join("xuanji-lu.sqlite3").exists() { return Ok(()); }
        Self::copy_directory(legacy_root, root)
    }

    fn rewrite_legacy_image_paths(&self, legacy_root: &Path) -> Result<(), String> {
        if !self.db_path.exists() { return Ok(()); }
        let old_prefix = legacy_root.to_string_lossy().to_string();
        let new_prefix = self.base_dir.to_string_lossy().to_string();
        if old_prefix == new_prefix { return Ok(()); }
        let match_prefix = format!("{old_prefix}%");
        let connection = self.conn()?;
        for table in ["divination_records", "gallery_items"] {
            connection.execute(&format!("UPDATE {table} SET image_url=REPLACE(image_url, ?1, ?2) WHERE image_url LIKE ?3"), params![old_prefix, new_prefix, match_prefix]).map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    fn conn(&self) -> Result<Connection, String> {
        Connection::open(&self.db_path).map_err(|e| e.to_string())
    }

    fn migrate(&self) -> Result<(), String> {
        let conn = self.conn()?;
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS divination_records (
               id TEXT PRIMARY KEY, uid TEXT NOT NULL UNIQUE, created TEXT NOT NULL, updated TEXT NOT NULL,
               question TEXT NOT NULL, dice_json TEXT NOT NULL, reading_json TEXT, summary_text TEXT NOT NULL DEFAULT '',
               image_url TEXT NOT NULL DEFAULT '', review_note TEXT NOT NULL DEFAULT '', rating INTEGER NOT NULL DEFAULT 0,
               tag_names_json TEXT NOT NULL DEFAULT '[]'
             );
             CREATE TABLE IF NOT EXISTS training_records (
               id TEXT PRIMARY KEY, uid TEXT NOT NULL UNIQUE, created TEXT NOT NULL, updated TEXT NOT NULL,
               question TEXT NOT NULL, dice_json TEXT, dice_text TEXT NOT NULL DEFAULT '', user_answer TEXT NOT NULL DEFAULT '',
               ai_compare_text TEXT NOT NULL DEFAULT '', ai_compare_json TEXT, ai_score INTEGER NOT NULL DEFAULT 0
             );
             CREATE TABLE IF NOT EXISTS gallery_items (
               id TEXT PRIMARY KEY, uid TEXT NOT NULL UNIQUE, created TEXT NOT NULL, updated TEXT NOT NULL,
               image_url TEXT NOT NULL, mode TEXT NOT NULL, question TEXT NOT NULL, dice_text TEXT NOT NULL DEFAULT ''
             );
             CREATE TABLE IF NOT EXISTS tags (
               id TEXT PRIMARY KEY, name TEXT NOT NULL UNIQUE, color TEXT NOT NULL, meaning TEXT NOT NULL DEFAULT '',
               created TEXT NOT NULL, updated TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS model_profiles (
               id TEXT PRIMARY KEY, name TEXT NOT NULL, capability TEXT NOT NULL, protocol TEXT NOT NULL,
               endpoint TEXT NOT NULL, model TEXT NOT NULL, auth_type TEXT NOT NULL DEFAULT 'bearer',
               extra_headers TEXT NOT NULL DEFAULT '{}', extra_params TEXT NOT NULL DEFAULT '{}', timeout_seconds INTEGER NOT NULL DEFAULT 300,
               image_size_override TEXT NOT NULL DEFAULT '', image_quality_override TEXT NOT NULL DEFAULT '',
               image_output_format_override TEXT NOT NULL DEFAULT '', image_response_format_override TEXT NOT NULL DEFAULT '',
               created TEXT NOT NULL, updated TEXT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_divination_created ON divination_records(created DESC);
             CREATE INDEX IF NOT EXISTS idx_training_created ON training_records(created DESC);
             CREATE INDEX IF NOT EXISTS idx_gallery_created ON gallery_items(created DESC);"
        ).map_err(|e| e.to_string())?;
        let mut columns = conn.prepare("PRAGMA table_info(model_profiles)").map_err(|e| e.to_string())?;
        let existing_columns = columns.query_map([], |row| row.get::<_, String>(1)).map_err(|e| e.to_string())?.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
        for (name, sql) in [
            ("timeout_seconds", "ALTER TABLE model_profiles ADD COLUMN timeout_seconds INTEGER NOT NULL DEFAULT 300"),
            ("image_size_override", "ALTER TABLE model_profiles ADD COLUMN image_size_override TEXT NOT NULL DEFAULT ''"),
            ("image_quality_override", "ALTER TABLE model_profiles ADD COLUMN image_quality_override TEXT NOT NULL DEFAULT ''"),
            ("image_output_format_override", "ALTER TABLE model_profiles ADD COLUMN image_output_format_override TEXT NOT NULL DEFAULT ''"),
            ("image_response_format_override", "ALTER TABLE model_profiles ADD COLUMN image_response_format_override TEXT NOT NULL DEFAULT ''"),
        ] { if !existing_columns.iter().any(|column| column == name) { conn.execute(sql, []).map_err(|e| e.to_string())?; } }
        let mut training_columns = conn.prepare("PRAGMA table_info(training_records)").map_err(|e| e.to_string())?;
        let existing_training_columns = training_columns.query_map([], |row| row.get::<_, String>(1)).map_err(|e| e.to_string())?.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
        if !existing_training_columns.iter().any(|column| column == "ai_score") { conn.execute("ALTER TABLE training_records ADD COLUMN ai_score INTEGER NOT NULL DEFAULT 0", []).map_err(|e| e.to_string())?; }
        for (name,color,meaning) in [("自动","#c8a86b","自动掷骰"),("手动","#b87898","手动指定骰面"),("图片","#7db6d8","已生成星语图片")] {
            let now=Self::now();
            conn.execute("INSERT OR IGNORE INTO tags(id,name,color,meaning,created,updated) VALUES(?1,?2,?3,?4,?5,?5)",params![Uuid::new_v4().to_string(),name,color,meaning,now]).map_err(|e|e.to_string())?;
        }
        self.reconcile_image_tags()?;
        Ok(())
    }

    pub fn now() -> String { Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true) }

    fn secure_index(length: usize) -> Result<usize, String> {
        if length == 0 || length > u32::MAX as usize { return Err("骰子面数量无效。".to_string()); }
        let limit = u32::MAX - (u32::MAX % length as u32);
        loop {
            let value = secure_random_u32().map_err(|error| format!("无法获取操作系统随机熵：{error}"))?;
            if value < limit { return Ok((value % length as u32) as usize); }
        }
    }

    pub fn roll_local_dice(&self) -> Result<DiceResult, String> {
        Ok(DiceResult {
            planet: PLANET_DICE[Self::secure_index(PLANET_DICE.len())?].to_string(),
            zodiac: ZODIAC_DICE[Self::secure_index(ZODIAC_DICE.len())?].to_string(),
            house: HOUSE_DICE[Self::secure_index(HOUSE_DICE.len())?].to_string(),
        })
    }

    pub fn images_dir(&self) -> PathBuf { self.base_dir.join("images") }
    pub fn backup_dir(&self) -> PathBuf { self.base_dir.join("backups") }

    pub fn append_debug_log(&self, event:&Value) {
        let enabled = self.prefs().map(|prefs| prefs.debug_logging_enabled).unwrap_or(false);
        if !enabled { return; }
        let result = (|| -> Result<(), String> {
            let exe_dir = std::env::current_exe().map_err(|e| e.to_string())?.parent().map(Path::to_path_buf).ok_or_else(|| "无法确定软件所在目录。".to_string())?;
            let log_dir = exe_dir.join("logs");
            fs::create_dir_all(&log_dir).map_err(|e| e.to_string())?;
            let log_path = log_dir.join("xuanji-llm-debug.jsonl");
            if log_path.metadata().map(|meta| meta.len() > 5 * 1024 * 1024).unwrap_or(false) { fs::write(&log_path, "").map_err(|e| e.to_string())?; }
            let mut file = fs::OpenOptions::new().create(true).append(true).open(&log_path).map_err(|e| e.to_string())?;
            writeln!(file, "{}", serde_json::to_string(event).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
            Ok(())
        })();
        let _ = result;
    }

    pub fn backup_database(&self, label: &str) -> Result<PathBuf, String> {
        let stamp = Utc::now().format("%Y%m%d-%H%M%S");
        let safe = label.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_').collect::<String>();
        let output = self.backup_dir().join(format!("xuanji-lu-{}-{}.sqlite3", stamp, safe));
        if self.db_path.exists() { fs::copy(&self.db_path, &output).map_err(|e| e.to_string())?; }
        Ok(output)
    }

    fn value_or_empty(value: Option<String>) -> String { value.unwrap_or_default() }
    fn parse_json<T: serde::de::DeserializeOwned + Default>(raw: &str) -> T { serde_json::from_str(raw).unwrap_or_default() }

    pub fn snapshot(&self) -> Result<AppSnapshot, String> {
        self.repair_missing_divination_images()?;
        self.reconcile_image_tags()?;
        Ok(AppSnapshot {
            divination_records: self.list_divinations()?,
            training_records: self.list_training()?,
            gallery_items: self.list_gallery()?,
            tags: self.list_tags()?,
            profiles: self.list_profiles()?,
            prefs: self.prefs()?,
            data_directory: self.base_dir.to_string_lossy().to_string(),
        })
    }

    pub fn list_divinations(&self) -> Result<Vec<DivinationRecord>, String> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare("SELECT id, uid, created, updated, question, dice_json, reading_json, summary_text, image_url, review_note, rating, tag_names_json FROM divination_records ORDER BY created DESC LIMIT 500").map_err(|e|e.to_string())?;
        let rows = stmt.query_map([], |r| Ok(DivinationRecord {
            id: r.get(0)?, uid: r.get(1)?, created: r.get(2)?, updated: r.get(3)?, question: r.get(4)?,
            dice: Self::parse_json(&r.get::<_, String>(5)?),
            reading: r.get::<_, Option<String>>(6)?.and_then(|x| serde_json::from_str(&x).ok()),
            summary_text: r.get(7)?, image_url: r.get(8)?, review_note: r.get(9)?, rating: r.get(10)?,
            tag_names: Self::parse_json(&r.get::<_, String>(11)?),
        })).map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
    }

    pub fn list_training(&self) -> Result<Vec<TrainingRecord>, String> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare("SELECT id, uid, created, updated, question, dice_json, dice_text, user_answer, ai_compare_text, ai_compare_json, ai_score FROM training_records ORDER BY created DESC LIMIT 500").map_err(|e|e.to_string())?;
        let rows = stmt.query_map([], |r| Ok(TrainingRecord {
            id: r.get(0)?, uid: r.get(1)?, created: r.get(2)?, updated: r.get(3)?, question: r.get(4)?,
            dice: r.get::<_, Option<String>>(5)?.and_then(|x| serde_json::from_str(&x).ok()), dice_text: r.get(6)?,
            user_answer: r.get(7)?, ai_compare_text: r.get(8)?, ai_compare: r.get::<_, Option<String>>(9)?.and_then(|x| serde_json::from_str(&x).ok()), ai_score: r.get(10)?,
        })).map_err(|e|e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e|e.to_string())
    }

    pub fn list_gallery(&self) -> Result<Vec<GalleryItem>, String> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare("SELECT id, uid, created, updated, image_url, mode, question, dice_text FROM gallery_items ORDER BY created DESC LIMIT 50").map_err(|e|e.to_string())?;
        let rows = stmt.query_map([], |r| Ok(GalleryItem { id:r.get(0)?, uid:r.get(1)?, created:r.get(2)?, updated:r.get(3)?, image_url:r.get(4)?, mode:r.get(5)?, question:r.get(6)?, dice_text:r.get(7)? })).map_err(|e|e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e|e.to_string())
    }

    pub fn list_tags(&self) -> Result<Vec<Tag>, String> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare("SELECT id, name, color, meaning, created, updated FROM tags ORDER BY created ASC").map_err(|e|e.to_string())?;
        let rows = stmt.query_map([], |r| Ok(Tag { id:r.get(0)?, name:r.get(1)?, color:r.get(2)?, meaning:r.get(3)?, created:r.get(4)?, updated:r.get(5)? })).map_err(|e|e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e|e.to_string())
    }

    pub fn list_profiles(&self) -> Result<Vec<ModelProfile>, String> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare("SELECT id, name, capability, protocol, endpoint, model, auth_type, extra_headers, extra_params, timeout_seconds, image_size_override, image_quality_override, image_output_format_override, image_response_format_override, created, updated FROM model_profiles ORDER BY created ASC").map_err(|e|e.to_string())?;
        let rows = stmt.query_map([], |r| {
            let id: String = r.get(0)?;
            Ok(ModelProfile { id: id.clone(), name:r.get(1)?, capability:r.get(2)?, protocol:r.get(3)?, endpoint:r.get(4)?, model:r.get(5)?, auth_type:r.get(6)?, extra_headers:r.get(7)?, extra_params:r.get(8)?, timeout_seconds:r.get(9)?, image_size_override:r.get(10)?, image_quality_override:r.get(11)?, image_output_format_override:r.get(12)?, image_response_format_override:r.get(13)?, has_secret: Self::has_profile_secret(&id), created:r.get(14)?, updated:r.get(15)? })
        }).map_err(|e|e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e|e.to_string())
    }

    fn has_profile_secret(id: &str) -> bool {
        Entry::new(SECRET_SERVICE, id).and_then(|e| e.get_password()).map(|v| !v.is_empty()).unwrap_or(false)
    }

    pub fn profile_secret(&self, id: &str) -> Result<String, String> {
        Entry::new(SECRET_SERVICE, id).map_err(|e|e.to_string())?.get_password().map_err(|_| "当前模型 Profile 未保存密钥，请在设置中补充。".to_string())
    }

    pub fn profile(&self, id: &str) -> Result<ModelProfile, String> {
        self.list_profiles()?.into_iter().find(|x| x.id == id).ok_or_else(|| "未找到模型 Profile。".to_string())
    }

    pub fn save_profile(&self, input: ModelProfileInput) -> Result<ModelProfile, String> {
        if input.name.trim().is_empty() || input.endpoint.trim().is_empty() || input.model.trim().is_empty() { return Err("Profile 名称、服务地址与模型名称均不能为空。".to_string()); }
        if input.capability != "text" && input.capability != "image" { return Err("Profile 能力必须是 text 或 image。".to_string()); }
        if !(10..=1800).contains(&input.timeout_seconds) { return Err("Profile 超时必须介于 10 至 1800 秒之间。".to_string()); }
        let id = input.id.unwrap_or_else(|| Uuid::new_v4().to_string());
        let now = Self::now();
        let conn = self.conn()?;
        let existing_created: Option<String> = conn.query_row("SELECT created FROM model_profiles WHERE id=?1", [&id], |r| r.get(0)).optional().map_err(|e|e.to_string())?;
        conn.execute("INSERT INTO model_profiles (id,name,capability,protocol,endpoint,model,auth_type,extra_headers,extra_params,timeout_seconds,image_size_override,image_quality_override,image_output_format_override,image_response_format_override,created,updated) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16) ON CONFLICT(id) DO UPDATE SET name=excluded.name,capability=excluded.capability,protocol=excluded.protocol,endpoint=excluded.endpoint,model=excluded.model,auth_type=excluded.auth_type,extra_headers=excluded.extra_headers,extra_params=excluded.extra_params,timeout_seconds=excluded.timeout_seconds,image_size_override=excluded.image_size_override,image_quality_override=excluded.image_quality_override,image_output_format_override=excluded.image_output_format_override,image_response_format_override=excluded.image_response_format_override,updated=excluded.updated", params![&id,input.name.trim(),input.capability,input.protocol,input.endpoint.trim(),input.model.trim(),input.auth_type,input.extra_headers,input.extra_params,input.timeout_seconds,input.image_size_override.trim(),input.image_quality_override.trim(),input.image_output_format_override.trim(),input.image_response_format_override.trim(),existing_created.unwrap_or_else(||now.clone()),now]).map_err(|e|e.to_string())?;
        if let Some(key) = input.api_key { if !key.trim().is_empty() { Entry::new(SECRET_SERVICE, &id).map_err(|e|e.to_string())?.set_password(key.trim()).map_err(|_| "无法写入系统安全凭据。请确认 Windows 凭据管理器可用。".to_string())?; } }
        self.profile(&id)
    }

    pub fn delete_profile(&self, id: &str) -> Result<(), String> {
        let conn = self.conn()?;
        conn.execute("DELETE FROM model_profiles WHERE id=?1", [id]).map_err(|e|e.to_string())?;
        let _ = Entry::new(SECRET_SERVICE, id).and_then(|e| e.delete_credential());
        let prefs = self.prefs()?;
        if prefs.active_text_profile_id == id { self.set_pref("active_text_profile_id", "")?; }
        if prefs.active_image_profile_id == id { self.set_pref("active_image_profile_id", "")?; }
        Ok(())
    }

    pub fn prefs(&self) -> Result<Preferences, String> { Ok(Preferences { active_text_profile_id:self.get_pref("active_text_profile_id")?, active_image_profile_id:self.get_pref("active_image_profile_id")?, tts_voice_short_name:self.get_pref("tts_voice_short_name")?, image_dice_prompt:self.get_pref_or_default("image_dice_prompt", DEFAULT_DICE_IMAGE_PROMPT)?, image_poster_prompt:self.get_pref_or_default("image_poster_prompt", DEFAULT_POSTER_IMAGE_PROMPT)?, image_custom_prompt:self.get_pref_or_default("image_custom_prompt", DEFAULT_CUSTOM_IMAGE_PROMPT)?, debug_logging_enabled:self.get_pref("debug_logging_enabled")? == "true", debug_log_content:self.get_pref("debug_log_content")? == "true", last_image_save_directory:self.get_pref("last_image_save_directory")?, last_audio_save_directory:self.get_pref("last_audio_save_directory")?, training_topic_scopes:Self::parse_json(&self.get_pref("training_topic_scopes")?), training_custom_scope:self.get_pref("training_custom_scope")? }) }
    fn get_pref_or_default(&self, key:&str, default:&str) -> Result<String,String> { let value=self.get_pref(key)?; Ok(if value.trim().is_empty(){default.to_string()}else{value}) }
    fn get_pref(&self, key:&str) -> Result<String,String> { self.conn()?.query_row("SELECT value FROM meta WHERE key=?1", [key], |r|r.get(0)).optional().map_err(|e|e.to_string()).map(|v|v.unwrap_or_default()) }
    fn set_pref(&self,key:&str,value:&str)->Result<(),String>{self.conn()?.execute("INSERT INTO meta(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",params![key,value]).map_err(|e|e.to_string())?;Ok(())}
    pub fn save_prefs(&self,prefs:Preferences)->Result<Preferences,String>{ if prefs.training_custom_scope.chars().count()>15{return Err("训练场“其它”范围不得超过 15 个字。".to_string())} self.set_pref("active_text_profile_id",&prefs.active_text_profile_id)?;self.set_pref("active_image_profile_id",&prefs.active_image_profile_id)?;self.set_pref("tts_voice_short_name",&prefs.tts_voice_short_name)?;self.set_pref("image_dice_prompt",&prefs.image_dice_prompt)?;self.set_pref("image_poster_prompt",&prefs.image_poster_prompt)?;self.set_pref("image_custom_prompt",&prefs.image_custom_prompt)?;self.set_pref("debug_logging_enabled",if prefs.debug_logging_enabled{"true"}else{"false"})?;self.set_pref("debug_log_content",if prefs.debug_log_content{"true"}else{"false"})?;self.set_pref("last_image_save_directory",&prefs.last_image_save_directory)?;self.set_pref("last_audio_save_directory",&prefs.last_audio_save_directory)?;self.set_pref("training_topic_scopes",&serde_json::to_string(&prefs.training_topic_scopes).map_err(|e|e.to_string())?)?;self.set_pref("training_custom_scope",prefs.training_custom_scope.trim())?;self.prefs() }

    fn ensure_tag_names(&self, names:&[String]) -> Result<(),String> { for raw_name in names { let name=raw_name.trim(); if name.is_empty(){continue;} let conn=self.conn()?;let count:i32=conn.query_row("SELECT COUNT(*) FROM tags WHERE name=?1",[name],|r|r.get(0)).map_err(|e|e.to_string())?; if count==0 { let now=Self::now();let (color,meaning)=match name {"自动"=>("#c8a86b","自动掷骰"),"手动"=>("#b87898","手动指定骰面"),"图片"=>("#7db6d8","已生成星语图片"),_=>("#8a8a8a","从交换档案自动创建的标记")};conn.execute("INSERT INTO tags(id,name,color,meaning,created,updated) VALUES(?1,?2,?3,?4,?5,?5)",params![Uuid::new_v4().to_string(),name,color,meaning,now]).map_err(|e|e.to_string())?; } } Ok(()) }

    fn is_system_tag_name(name:&str)->bool{SYSTEM_TAG_NAMES.contains(&name.trim())}
    pub fn save_tag(&self, mut tag:Tag)->Result<Tag,String>{if tag.name.trim().is_empty(){return Err("标记名称不能为空。".to_string())} if Self::is_system_tag_name(&tag.name){return Err("“自动”、“手动”和“图片”是系统标记，不能手动编辑。".to_string())} if !tag.id.is_empty(){let existing_name:Option<String>=self.conn()?.query_row("SELECT name FROM tags WHERE id=?1",[&tag.id],|r|r.get(0)).optional().map_err(|e|e.to_string())?;if existing_name.as_deref().map(Self::is_system_tag_name).unwrap_or(false){return Err("系统标记不能编辑。".to_string())}} if tag.name.chars().count()>20||tag.meaning.chars().count()>100{return Err("标记名称不得超过20字，含义不得超过100字。".to_string())} let now=Self::now();if tag.id.is_empty(){tag.id=Uuid::new_v4().to_string();tag.created=now.clone();}tag.updated=now;let conn=self.conn()?;conn.execute("INSERT INTO tags(id,name,color,meaning,created,updated) VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(id) DO UPDATE SET name=excluded.name,color=excluded.color,meaning=excluded.meaning,updated=excluded.updated",params![tag.id,tag.name.trim(),tag.color,tag.meaning,tag.created,tag.updated]).map_err(|e|e.to_string())?;Ok(tag)}
    pub fn delete_tag(&self,id:&str)->Result<(),String>{let name:Option<String>=self.conn()?.query_row("SELECT name FROM tags WHERE id=?1",[id],|r|r.get(0)).optional().map_err(|e|e.to_string())?;if name.as_deref().map(Self::is_system_tag_name).unwrap_or(false){return Err("系统标记不能删除。".to_string())}self.conn()?.execute("DELETE FROM tags WHERE id=?1",[id]).map_err(|e|e.to_string())?;Ok(())}

    pub fn save_divination(&self, mut r:DivinationRecord)->Result<DivinationRecord,String>{
        if r.question.trim().is_empty(){return Err("请先写下想问的问题。".to_string())};let now=Self::now();if r.id.is_empty(){r.id=Uuid::new_v4().to_string();}if r.created.is_empty(){r.created=now.clone();}if r.updated.is_empty(){r.updated=now.clone();}if r.uid.is_empty(){r.uid=format!("div|{}|{}|{}+{}+{}",r.created,r.question,r.dice.planet,r.dice.zodiac,r.dice.house)};if r.summary_text.is_empty(){r.summary_text=r.reading.as_ref().map(|x|x.summary.clone()).unwrap_or_default()}self.ensure_tag_names(&r.tag_names)?;let conn=self.conn()?;conn.execute("INSERT INTO divination_records(id,uid,created,updated,question,dice_json,reading_json,summary_text,image_url,review_note,rating,tag_names_json) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12) ON CONFLICT(uid) DO UPDATE SET updated=excluded.updated,question=excluded.question,dice_json=excluded.dice_json,reading_json=excluded.reading_json,summary_text=excluded.summary_text,image_url=excluded.image_url,review_note=excluded.review_note,rating=excluded.rating,tag_names_json=excluded.tag_names_json",params![r.id,r.uid,r.created,r.updated,r.question,serde_json::to_string(&r.dice).unwrap(),r.reading.as_ref().map(|x|serde_json::to_string(x).unwrap()),r.summary_text,r.image_url,r.review_note,r.rating,serde_json::to_string(&r.tag_names).unwrap()]).map_err(|e|e.to_string())?;Ok(r)
    }

    pub fn update_divination_meta(&self,id:&str,review_note:&str,rating:i32,tag_names:Vec<String>)->Result<(),String>{if !(0..=5).contains(&rating){return Err("评分必须在 0 至 5 星之间。".to_string())}let current:Option<String>=self.conn()?.query_row("SELECT tag_names_json FROM divination_records WHERE id=?1",[id],|r|r.get(0)).optional().map_err(|e|e.to_string())?;let mut final_names=Self::parse_json::<Vec<String>>(&current.unwrap_or_default()).into_iter().filter(|name|Self::is_system_tag_name(name)).collect::<Vec<_>>();for name in tag_names{if !Self::is_system_tag_name(&name)&&!final_names.contains(&name){final_names.push(name)}}self.ensure_tag_names(&final_names)?;self.conn()?.execute("UPDATE divination_records SET review_note=?1,rating=?2,tag_names_json=?3,updated=?4 WHERE id=?5",params![review_note,rating,serde_json::to_string(&final_names).unwrap(),Self::now(),id]).map_err(|e|e.to_string())?;Ok(())}

    pub fn attach_divination_image(&self,id:&str,image_url:&str)->Result<(),String>{
        if image_url.trim().is_empty(){return Err("图片路径不能为空。".to_string())}
        let conn=self.conn()?;
        let raw_tags:Option<String>=conn.query_row("SELECT tag_names_json FROM divination_records WHERE id=?1",[id],|row|row.get(0)).optional().map_err(|e|e.to_string())?;
        let mut tags=Self::parse_json::<Vec<String>>(&raw_tags.ok_or_else(||"未找到关联占卜记录。".to_string())?);
        if !tags.iter().any(|name|name=="图片"){tags.push("图片".to_string());}
        conn.execute("UPDATE divination_records SET image_url=?1,tag_names_json=?2,updated=?3 WHERE id=?4",params![image_url,serde_json::to_string(&tags).unwrap(),Self::now(),id]).map_err(|e|e.to_string())?;
        Ok(())
    }

    fn reconcile_image_tags(&self)->Result<(),String>{
        let conn=self.conn()?;
        let mut stmt=conn.prepare("SELECT id,tag_names_json FROM divination_records WHERE TRIM(image_url)<>''").map_err(|e|e.to_string())?;
        let rows=stmt.query_map([],|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?))).map_err(|e|e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?;
        drop(stmt);
        for (id,raw_tags) in rows { let mut tags=Self::parse_json::<Vec<String>>(&raw_tags); if !tags.iter().any(|name|name=="图片"){tags.push("图片".to_string());conn.execute("UPDATE divination_records SET tag_names_json=?1,updated=?2 WHERE id=?3",params![serde_json::to_string(&tags).unwrap(),Self::now(),id]).map_err(|e|e.to_string())?;} }
        Ok(())
    }

    fn repair_missing_divination_images(&self)->Result<(),String>{
        self.conn()?.execute("UPDATE divination_records SET image_url=(SELECT gallery_items.image_url FROM gallery_items WHERE gallery_items.question=divination_records.question ORDER BY gallery_items.created DESC LIMIT 1),updated=?1 WHERE image_url='' AND EXISTS(SELECT 1 FROM gallery_items WHERE gallery_items.question=divination_records.question)",params![Self::now()]).map_err(|e|e.to_string())?;
        Ok(())
    }

    pub fn save_training(&self,mut r:TrainingRecord)->Result<TrainingRecord,String>{if r.question.trim().is_empty(){return Err("训练问题不能为空。".to_string())}if r.ai_score==0{r.ai_score=r.ai_compare.as_ref().map(|comparison|comparison.score).unwrap_or(0)}if !(0..=100).contains(&r.ai_score){return Err("AI 评分必须在 0 至 100 分之间。".to_string())}let now=Self::now();if r.id.is_empty(){r.id=Uuid::new_v4().to_string()}if r.created.is_empty(){r.created=now.clone()}if r.updated.is_empty(){r.updated=now.clone()}if r.uid.is_empty(){let dice=r.dice.as_ref().map(|d|format!("{}+{}+{}",d.planet,d.zodiac,d.house)).unwrap_or_else(||r.dice_text.clone());r.uid=format!("trn|{}|{}|{}",r.created,r.question,dice)}let conn=self.conn()?;conn.execute("INSERT INTO training_records(id,uid,created,updated,question,dice_json,dice_text,user_answer,ai_compare_text,ai_compare_json,ai_score) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11) ON CONFLICT(uid) DO UPDATE SET updated=excluded.updated,question=excluded.question,dice_json=excluded.dice_json,dice_text=excluded.dice_text,user_answer=excluded.user_answer,ai_compare_text=excluded.ai_compare_text,ai_compare_json=excluded.ai_compare_json,ai_score=excluded.ai_score",params![r.id,r.uid,r.created,r.updated,r.question,r.dice.as_ref().map(|x|serde_json::to_string(x).unwrap()),r.dice_text,r.user_answer,r.ai_compare_text,r.ai_compare.as_ref().map(|x|serde_json::to_string(x).unwrap()),r.ai_score]).map_err(|e|e.to_string())?;Ok(r)}

    pub fn save_gallery(&self,mut r:GalleryItem)->Result<GalleryItem,String>{let now=Self::now();if r.id.is_empty(){r.id=Uuid::new_v4().to_string()}if r.created.is_empty(){r.created=now.clone()}if r.updated.is_empty(){r.updated=now.clone()}if r.uid.is_empty(){r.uid=format!("img|{}|{:x}",r.created,md5::compute(&r.image_url))}let conn=self.conn()?;conn.execute("INSERT INTO gallery_items(id,uid,created,updated,image_url,mode,question,dice_text) VALUES(?1,?2,?3,?4,?5,?6,?7,?8) ON CONFLICT(uid) DO UPDATE SET updated=excluded.updated,image_url=excluded.image_url,mode=excluded.mode,question=excluded.question,dice_text=excluded.dice_text",params![r.id,r.uid,r.created,r.updated,r.image_url,r.mode,r.question,r.dice_text]).map_err(|e|e.to_string())?;conn.execute("DELETE FROM gallery_items WHERE id IN (SELECT id FROM gallery_items ORDER BY created DESC LIMIT -1 OFFSET 50)",[]).map_err(|e|e.to_string())?;Ok(r)}

    pub fn delete_record(&self,kind:&str,id:&str)->Result<(),String>{let table=match kind{"divination"=>"divination_records","training"=>"training_records","gallery"=>"gallery_items",_=>return Err("未知记录类型。".to_string())};self.conn()?.execute(&format!("DELETE FROM {} WHERE id=?1",table),[id]).map_err(|e|e.to_string())?;Ok(())}

    pub fn initialize_archive(&self, delete_images: bool, delete_profiles: bool) -> Result<(), String> {
        if delete_images {
            let images = self.images_dir();
            if images.exists() { fs::remove_dir_all(&images).map_err(|error| format!("无法删除本地图库文件：{error}"))?; }
            fs::create_dir_all(&images).map_err(|error| error.to_string())?;
        }
        let profile_ids = if delete_profiles { self.list_profiles()?.into_iter().map(|profile| profile.id).collect::<Vec<_>>() } else { Vec::new() };
        let mut connection = self.conn()?;
        let transaction = connection.transaction().map_err(|error| error.to_string())?;
        for table in ["divination_records", "training_records", "gallery_items", "tags"] {
            transaction.execute(&format!("DELETE FROM {table}"), []).map_err(|error| error.to_string())?;
        }
        if delete_profiles {
            transaction.execute("DELETE FROM model_profiles", []).map_err(|error| error.to_string())?;
            transaction.execute("DELETE FROM meta WHERE key IN ('active_text_profile_id','active_image_profile_id','tts_voice_short_name')", []).map_err(|error| error.to_string())?;
        }
        transaction.commit().map_err(|error| error.to_string())?;
        for profile_id in profile_ids { let _ = Entry::new(SECRET_SERVICE, &profile_id).and_then(|entry| entry.delete_credential()); }
        self.migrate()?;
        Ok(())
    }

    pub fn database_exists(&self)->bool{self.db_path.exists()}
    pub fn local_image_path(&self, name:&str)->PathBuf{self.images_dir().join(name)}
    pub fn set_tag_names_for_exchange(&self,names:&[String])->Result<(),String>{self.ensure_tag_names(names)}
    pub fn raw_connection(&self)->Result<Connection,String>{self.conn()}
    pub fn read_local_image(&self,path:&Path)->Result<Vec<u8>,String>{fs::read(path).map_err(|e|e.to_string())}
    pub fn verified_local_image_path(&self, raw_path: &str) -> Result<PathBuf, String> {
        let image_root = fs::canonicalize(self.images_dir()).map_err(|_| "本地图库目录不存在或不可读取。".to_string())?;
        let candidate = fs::canonicalize(Path::new(raw_path)).map_err(|_| "原图文件不存在或不可读取。".to_string())?;
        if !candidate.is_file() || !candidate.starts_with(&image_root) { return Err("只能打开软件本地图库目录中的图片。".to_string()); }
        Ok(candidate)
    }
    pub fn write_local_image(&self,name:&str,data:&[u8])->Result<PathBuf,String>{let path=self.local_image_path(name);if !path.starts_with(self.images_dir()){return Err("图片归档路径无效。".to_string())}if let Some(parent)=path.parent(){fs::create_dir_all(parent).map_err(|e|e.to_string())?;}fs::write(&path,data).map_err(|e|e.to_string())?;Ok(path)}
    pub fn path_is_inside_images(&self,path:&str)->bool{Path::new(path).starts_with(self.images_dir())}
    pub fn parse_object(raw:&str)->Value{serde_json::from_str(raw).unwrap_or(Value::Object(Default::default()))}
}

#[cfg(test)]
mod storage_tests {
    use super::*;

    fn temporary_storage() -> Storage {
        let root = std::env::temp_dir().join(format!("xuanji-lu-storage-test-{}", Uuid::new_v4()));
        fs::create_dir_all(root.join("images")).unwrap();
        fs::create_dir_all(root.join("backups")).unwrap();
        let storage = Storage { db_path: root.join("test.sqlite3"), base_dir: root };
        storage.migrate().unwrap();
        storage
    }

    #[test]
    fn new_generation_image_is_linked_by_divination_id_and_debug_is_off_by_default() {
        let storage = temporary_storage();
        let saved = storage.save_divination(DivinationRecord { question: "测试新图关联".to_string(), tag_names: vec!["自动".to_string()], ..Default::default() }).unwrap();
        storage.attach_divination_image(&saved.id, "C:/images/exact.png").unwrap();
        let record = storage.list_divinations().unwrap().into_iter().find(|record| record.id == saved.id).unwrap();
        assert_eq!(record.image_url, "C:/images/exact.png");
        assert_eq!(record.tag_names, vec!["自动", "图片"]);
        let mut prefs = storage.prefs().unwrap();
        assert!(!prefs.debug_logging_enabled);
        assert!(!prefs.debug_log_content);
        assert!(storage.list_tags().unwrap().iter().any(|tag| tag.name == "自动" && tag.color == "#c8a86b"));
        prefs.last_image_save_directory = "C:/Users/example/Pictures".to_string();
        prefs.tts_voice_short_name = "zh-CN-XiaoxiaoNeural".to_string();
        let saved_prefs = storage.save_prefs(prefs).unwrap();
        assert_eq!(saved_prefs.last_image_save_directory, "C:/Users/example/Pictures");
        assert_eq!(storage.prefs().unwrap().tts_voice_short_name, "zh-CN-XiaoxiaoNeural");
        let image = storage.write_local_image("202608/fixture.png", &[0x89, b'P', b'N', b'G', 1]).unwrap();
        assert!(image.exists());
        let root = storage.base_dir.clone();
        drop(storage);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn verified_local_image_path_allows_only_gallery_files() {
        let storage = temporary_storage();
        let image = storage.write_local_image("202608/allowed.png", &[0x89, b'P', b'N', b'G']).unwrap();
        assert_eq!(storage.verified_local_image_path(&image.to_string_lossy()).unwrap(), fs::canonicalize(&image).unwrap());
        assert!(storage.verified_local_image_path(&storage.db_path.to_string_lossy()).is_err());
        let root = storage.base_dir.clone();
        drop(storage);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn initialize_archive_respects_image_and_profile_options() {
        let storage = temporary_storage();
        storage.save_divination(DivinationRecord { question: "初始化占卜".to_string(), ..Default::default() }).unwrap();
        storage.save_training(TrainingRecord { question: "初始化训练".to_string(), ..Default::default() }).unwrap();
        let image = storage.write_local_image("202608/reset.png", &[0x89, b'P', b'N', b'G', 1]).unwrap();
        storage.save_gallery(GalleryItem { question: "初始化占卜".to_string(), image_url: image.to_string_lossy().to_string(), ..Default::default() }).unwrap();
        storage.save_profile(ModelProfileInput { id: None, name: "初始化 Profile".to_string(), capability: "text".to_string(), protocol: "openai-chat".to_string(), endpoint: "https://example.invalid/v1/chat/completions".to_string(), model: "test-model".to_string(), auth_type: "none".to_string(), extra_headers: "{}".to_string(), extra_params: "{}".to_string(), timeout_seconds: 300, image_size_override: String::new(), image_quality_override: String::new(), image_output_format_override: String::new(), image_response_format_override: String::new(), api_key: None }).unwrap();
        storage.initialize_archive(true, true).unwrap();
        assert!(storage.list_divinations().unwrap().is_empty());
        assert!(storage.list_training().unwrap().is_empty());
        assert!(storage.list_gallery().unwrap().is_empty());
        assert!(storage.list_profiles().unwrap().is_empty());
        assert!(!image.exists());
        assert_eq!(storage.list_tags().unwrap().len(), 3);
        let root = storage.base_dir.clone();
        drop(storage);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn legacy_data_copy_and_image_path_rewrite_preserve_archive() {
        let root = std::env::temp_dir().join(format!("xuanji-lu-migration-test-{}", Uuid::new_v4()));
        let legacy_root = root.join("legacy");
        let target_root = root.join("install-data");
        fs::create_dir_all(legacy_root.join("images/202608")).unwrap();
        fs::create_dir_all(legacy_root.join("backups")).unwrap();
        let legacy = Storage { db_path: legacy_root.join("xuanji-lu.sqlite3"), base_dir: legacy_root.clone() };
        legacy.migrate().unwrap();
        let image_path = legacy.write_local_image("202608/legacy.png", &[0x89, b'P', b'N', b'G', 1]).unwrap();
        let saved = legacy.save_divination(DivinationRecord { question: "迁移案例".to_string(), image_url: image_path.to_string_lossy().to_string(), ..Default::default() }).unwrap();
        legacy.save_gallery(GalleryItem { question: saved.question.clone(), image_url: image_path.to_string_lossy().to_string(), ..Default::default() }).unwrap();
        Storage::copy_legacy_data_if_needed(&legacy_root, &target_root).unwrap();
        let migrated = Storage { db_path: target_root.join("xuanji-lu.sqlite3"), base_dir: target_root.clone() };
        migrated.migrate().unwrap();
        migrated.rewrite_legacy_image_paths(&legacy_root).unwrap();
        let expected_image = target_root.join("images/202608/legacy.png").to_string_lossy().to_string();
        assert!(Path::new(&expected_image).exists());
        assert_eq!(migrated.list_divinations().unwrap()[0].image_url, expected_image);
        assert_eq!(migrated.list_gallery().unwrap()[0].image_url, expected_image);
        drop(migrated);
        drop(legacy);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn historical_repair_uses_the_newest_gallery_image_for_the_same_question() {
        let storage = temporary_storage();
        let saved = storage.save_divination(DivinationRecord { question: "测试旧图修复".to_string(), ..Default::default() }).unwrap();
        storage.save_gallery(GalleryItem { question: saved.question.clone(), image_url: "C:/images/older.png".to_string(), created: "2026-08-25T00:00:00.000Z".to_string(), ..Default::default() }).unwrap();
        storage.save_gallery(GalleryItem { question: saved.question.clone(), image_url: "C:/images/newer.png".to_string(), created: "2026-08-25T00:01:00.000Z".to_string(), ..Default::default() }).unwrap();
        let repaired = storage.snapshot().unwrap().divination_records.into_iter().find(|record| record.id == saved.id).unwrap();
        assert_eq!(repaired.image_url, "C:/images/newer.png");
        let root = storage.base_dir.clone();
        drop(storage);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn training_score_and_topic_scopes_round_trip_through_storage() {
        let storage = temporary_storage();
        let mut prefs = storage.prefs().unwrap();
        prefs.training_topic_scopes = vec!["恋爱与亲密关系".to_string(), "其它".to_string()];
        prefs.training_custom_scope = "宠物陪伴".to_string();
        let saved_prefs = storage.save_prefs(prefs).unwrap();
        assert_eq!(saved_prefs.training_topic_scopes, vec!["恋爱与亲密关系", "其它"]);
        assert_eq!(saved_prefs.training_custom_scope, "宠物陪伴");
        let comparison = TrainingComparison { rows: Vec::new(), highlights: "能够连结骰面。".to_string(), improvements: "可补充行动建议。".to_string(), overall_comment: "继续练习。".to_string(), score: 86 };
        let saved = storage.save_training(TrainingRecord { question: "测试评分保存".to_string(), ai_compare: Some(comparison), ..Default::default() }).unwrap();
        assert_eq!(saved.ai_score, 86);
        assert_eq!(storage.list_training().unwrap()[0].ai_score, 86);
        let root = storage.base_dir.clone();
        drop(storage);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_secure_dice_rolls_only_valid_faces() {
        let storage = temporary_storage();
        for _ in 0..64 {
            let dice = storage.roll_local_dice().unwrap();
            assert!(PLANET_DICE.contains(&dice.planet.as_str()));
            assert!(ZODIAC_DICE.contains(&dice.zodiac.as_str()));
            assert!(HOUSE_DICE.contains(&dice.house.as_str()));
        }
        let root = storage.base_dir.clone();
        drop(storage);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn system_tags_cannot_be_edited_deleted_or_removed_from_records() {
        let storage = temporary_storage();
        let automatic = storage.list_tags().unwrap().into_iter().find(|tag| tag.name == "自动").unwrap();
        assert!(storage.delete_tag(&automatic.id).is_err());
        assert!(storage.save_tag(Tag { id: String::new(), name: "自动".to_string(), color: "#000000".to_string(), meaning: String::new(), created: String::new(), updated: String::new() }).is_err());
        let saved = storage.save_divination(DivinationRecord { question: "系统标签保护".to_string(), tag_names: vec!["自动".to_string()], ..Default::default() }).unwrap();
        storage.update_divination_meta(&saved.id, "", 0, vec!["复盘".to_string()]).unwrap();
        let updated = storage.list_divinations().unwrap().into_iter().find(|record| record.id == saved.id).unwrap();
        assert!(updated.tag_names.contains(&"自动".to_string()));
        assert!(updated.tag_names.contains(&"复盘".to_string()));
        let root = storage.base_dir.clone();
        drop(storage);
        fs::remove_dir_all(root).unwrap();
    }
}
