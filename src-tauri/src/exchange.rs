use std::{fs, io::{Cursor, Read, Write}, path::{Path, PathBuf}};

use chrono::Utc;
use serde_json::json;
use zip::{write::SimpleFileOptions, ZipArchive, ZipWriter};

use crate::{models::*, storage::Storage};

const FORMAT: &str = "star-dice-exchange";
const VERSION: i32 = 1;

fn is_newer(left: &str, right: &str) -> bool { left > right }
fn file_name_from_path(value: &str) -> Option<String> { Path::new(value).file_name().map(|x| x.to_string_lossy().to_string()) }
fn local_relative(storage: &Storage, value: &str) -> Option<String> { if storage.path_is_inside_images(value) { file_name_from_path(value).map(|n| format!("images/{n}")) } else { None } }

pub fn package_from_storage(storage: &Storage) -> Result<ExchangePackage, String> {
    let snapshot = storage.snapshot()?;
    let text_model = snapshot.profiles.iter().find(|p| p.id == snapshot.prefs.active_text_profile_id).map(|p| p.model.clone()).unwrap_or_default();
    let image_model = snapshot.profiles.iter().find(|p| p.id == snapshot.prefs.active_image_profile_id).map(|p| p.model.clone()).unwrap_or_default();
    let divination_records = snapshot.divination_records.into_iter().map(|mut r| { if let Some(relative) = local_relative(storage, &r.image_url) { r.image_url = relative; } r }).collect();
    let gallery_items = snapshot.gallery_items.into_iter().map(|mut r| { if let Some(relative) = local_relative(storage, &r.image_url) { r.image_url = relative; } r }).collect();
    Ok(ExchangePackage {
        format: FORMAT.to_string(), version: VERSION, source: "windows".to_string(), app_version: "v261018A".to_string(),
        exported_at: Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        profile: ExchangeProfile { user_id: "local".to_string(), user_label: Some("璇玑录本地档案".to_string()) },
        divination_records, training_records: snapshot.training_records,
        tags: snapshot.tags.into_iter().map(|t| ExchangeTag { name:t.name,color:t.color,meaning:t.meaning }).collect(),
        gallery_items, prefs: Some(ExchangePrefs { text_model, image_model })
    })
}

fn local_images_for_package(storage: &Storage, package: &ExchangePackage) -> Vec<(String, PathBuf)> {
    let mut entries = Vec::new();
    for path in package.divination_records.iter().map(|r| &r.image_url).chain(package.gallery_items.iter().map(|r| &r.image_url)) {
        if path.starts_with("images/") { let name = path.trim_start_matches("images/"); let local = storage.local_image_path(name); if local.exists() { entries.push((format!("images/{name}"), local)); } }
    }
    entries.sort_by(|a,b| a.0.cmp(&b.0)); entries.dedup_by(|a,b| a.0 == b.0); entries
}

pub fn export_package(storage: &Storage, destination: &Path, zip_output: bool) -> Result<String, String> {
    let package = package_from_storage(storage)?;
    let json_bytes = serde_json::to_vec_pretty(&package).map_err(|e|e.to_string())?;
    let images = local_images_for_package(storage, &package);
    if zip_output {
        let file = fs::File::create(destination).map_err(|e|e.to_string())?;
        let mut writer = ZipWriter::new(file);
        let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        writer.start_file("星骰档案导出.json", options).map_err(|e|e.to_string())?;
        writer.write_all(&json_bytes).map_err(|e|e.to_string())?;
        for (entry, path) in images { writer.start_file(entry, options).map_err(|e|e.to_string())?; writer.write_all(&fs::read(path).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?; }
        writer.finish().map_err(|e|e.to_string())?;
    } else {
        fs::write(destination, json_bytes).map_err(|e|e.to_string())?;
        if !images.is_empty() { let root = destination.parent().unwrap_or(Path::new(".")).join("images"); fs::create_dir_all(&root).map_err(|e|e.to_string())?; for (entry,path) in images { let name = entry.trim_start_matches("images/"); fs::copy(path, root.join(name)).map_err(|e|e.to_string())?; } }
    }
    Ok(format!("已导出 {} 条占卜记录、{} 条训练记录和 {} 张图库图片。", package.divination_records.len(), package.training_records.len(), package.gallery_items.len()))
}

fn read_json_from_zip(path: &Path) -> Result<(ExchangePackage, Vec<(String, Vec<u8>)>), String> {
    let file = fs::File::open(path).map_err(|e|e.to_string())?;
    let mut archive = ZipArchive::new(file).map_err(|e|format!("无法读取 ZIP 档案包：{e}"))?;
    let mut package_bytes = Vec::new();
    let mut images = Vec::new();
    for i in 0..archive.len() { let mut entry = archive.by_index(i).map_err(|e|e.to_string())?; let name=entry.name().replace('\\',"/"); if name.ends_with(".json") && package_bytes.is_empty(){entry.read_to_end(&mut package_bytes).map_err(|e|e.to_string())?;} else if name.starts_with("images/") && !name.contains("..") { let mut data=Vec::new();entry.read_to_end(&mut data).map_err(|e|e.to_string())?;images.push((name,data)); } }
    if package_bytes.is_empty(){return Err("ZIP 包内未找到 SDE-1 JSON 文件。".to_string())}
    let package:ExchangePackage=serde_json::from_slice(&package_bytes).map_err(|e|format!("JSON 格式无效：{e}"))?;
    Ok((package,images))
}

fn read_package(path: &Path) -> Result<(ExchangePackage, Vec<(String, Vec<u8>)>), String> {
    if path.extension().map(|x|x.to_string_lossy().eq_ignore_ascii_case("zip")).unwrap_or(false) { return read_json_from_zip(path); }
    let raw=fs::read(path).map_err(|e|e.to_string())?;
    let package=serde_json::from_slice(&raw).map_err(|e|format!("JSON 格式无效：{e}"))?;
    let mut images=Vec::new();
    let root=path.parent().unwrap_or(Path::new(".")).join("images");
    if root.exists() { for entry in fs::read_dir(root).map_err(|e|e.to_string())? { let item=entry.map_err(|e|e.to_string())?;if item.file_type().map_err(|e|e.to_string())?.is_file(){images.push((format!("images/{}",item.file_name().to_string_lossy()),fs::read(item.path()).map_err(|e|e.to_string())?));} } }
    Ok((package,images))
}

fn validate(package:&ExchangePackage)->Result<(),String>{if package.format!=FORMAT{return Err("不支持的档案格式：仅可导入 star-dice-exchange。".to_string())}if package.version>VERSION||package.version<1{return Err(format!("不支持的 SDE-1 版本：{}。",package.version))}Ok(())}

fn mapped_image(storage:&Storage, value:&str, image_map:&std::collections::HashMap<String,String>)->String{if value.starts_with("images/"){image_map.get(value).cloned().unwrap_or_else(||value.to_string())}else{value.to_string()}}

pub fn import_package(storage:&Storage, source:&Path)->Result<MergeSummary,String>{
    let (mut package, images)=read_package(source)?;validate(&package)?;let _=storage.backup_database("pre-import")?;let mut summary=MergeSummary::default();let mut image_map=std::collections::HashMap::new();
    for (relative,data) in images { if !relative.starts_with("images/")||relative.contains("..") {continue;} let name=file_name_from_path(&relative).unwrap_or_else(||format!("{}.bin",uuid::Uuid::new_v4()));let target=storage.local_image_path(&name);if !target.exists(){storage.write_local_image(&name,&data)?;summary.images_copied+=1;}image_map.insert(relative,target.to_string_lossy().to_string()); }
    let known_tags=storage.list_tags()?;
    for tag in package.tags.drain(..) { if !known_tags.iter().any(|x|x.name==tag.name) {storage.save_tag(Tag{id:String::new(),name:tag.name,color:tag.color,meaning:tag.meaning,created:String::new(),updated:String::new()})?;summary.tags_created+=1;} }
    let mut local=storage.snapshot()?;
    for mut incoming in package.divination_records { if incoming.updated.is_empty() { incoming.updated = incoming.created.clone(); } storage.set_tag_names_for_exchange(&incoming.tag_names)?;incoming.image_url=mapped_image(storage,&incoming.image_url,&image_map);match local.divination_records.iter().find(|x|x.uid==incoming.uid){None=>{storage.save_divination(incoming)?;summary.inserted+=1;},Some(existing) if is_newer(&incoming.updated,&existing.updated)=>{storage.save_divination(incoming)?;summary.updated+=1;},_=>summary.skipped+=1};local.divination_records=storage.list_divinations()?; }
    for mut incoming in package.training_records { if incoming.updated.is_empty() { incoming.updated = incoming.created.clone(); } match local.training_records.iter().find(|x|x.uid==incoming.uid){None=>{storage.save_training(incoming)?;summary.inserted+=1;},Some(existing) if is_newer(&incoming.updated,&existing.updated)=>{storage.save_training(incoming)?;summary.updated+=1;},_=>summary.skipped+=1};local.training_records=storage.list_training()?;}
    for mut incoming in package.gallery_items { if incoming.updated.is_empty() { incoming.updated = incoming.created.clone(); } incoming.image_url=mapped_image(storage,&incoming.image_url,&image_map);match local.gallery_items.iter().find(|x|x.uid==incoming.uid){None=>{storage.save_gallery(incoming)?;summary.inserted+=1;},Some(existing) if is_newer(&incoming.updated,&existing.updated)=>{storage.save_gallery(incoming)?;summary.updated+=1;},_=>summary.skipped+=1};local.gallery_items=storage.list_gallery()?;}
    Ok(summary)
}

pub fn divination_markdown(record:&DivinationRecord)->String{let mut output=format!("# 占星骰子 · 占卜记录\n\n- 时间：{}\n- 问题：{}\n\n## 三颗骰子\n- 行星骰：{}\n- 星座骰：{}\n- 宫骰：{}\n",record.created,record.question,record.dice.planet,record.dice.zodiac,record.dice.house);if let Some(reading)=&record.reading{output.push_str("\n## 逐骰解读\n");for card in &reading.dice_readings{output.push_str(&format!("\n### {} · {}\n{}\n",card.dice_label,card.face_name,card.meaning));}output.push_str(&format!("\n## 综合解读\n{}\n",reading.summary));}if !record.review_note.trim().is_empty(){output.push_str(&format!("\n## 复盘验证\n{}\n",record.review_note));}let rating=if record.rating==0{"未评分".to_string()}else{format!("{}（{} 星）","★".repeat(record.rating as usize)+&"☆".repeat((5-record.rating)as usize),record.rating)};output.push_str(&format!("\n## 评分与标记\n- 星级：{}\n- 标记：{}\n",rating,if record.tag_names.is_empty(){"未挂标记".to_string()}else{record.tag_names.join("、")}));if !record.image_url.is_empty(){output.push_str(&format!("- 意境图：{}\n",record.image_url));}output}

pub fn records_csv(records:&[DivinationRecord])->String{let mut out="\u{FEFF}时间,问题,行星骰,星座骰,宫骰,解读摘要,复盘备注,星级,标记\r\n".to_string();for r in records{let esc=|v:&str|format!("\"{}\"",v.replace('"',"\"\""));out.push_str(&[esc(&r.created),esc(&r.question),esc(&r.dice.planet),esc(&r.dice.zodiac),esc(&r.dice.house),esc(&r.summary_text),esc(&r.review_note),esc(&if r.rating==0{"".to_string()}else{format!("{}星",r.rating)}),esc(&r.tag_names.join("、"))].join(","));out.push_str("\r\n");}out}

pub fn html_export(records:&[DivinationRecord])->String{let cards=records.iter().map(|r|format!("<article class='card'><time>{}</time><h2>{}</h2><p class='dice'>{} · {} · {}</p><h3>综合解读</h3><p>{}</p>{}</article>",r.created,r.question,r.dice.planet,r.dice.zodiac,r.dice.house,r.summary_text,if r.review_note.is_empty(){"".to_string()}else{format!("<h3>复盘验证</h3><p>{}</p>",r.review_note)})).collect::<Vec<_>>().join("\n");format!("<!doctype html><html lang='zh-CN'><head><meta charset='utf-8'><title>占星骰子 · 星语档案</title><style>body{{background:#080706;color:#fff8ea;font-family:system-ui;padding:40px;line-height:1.7}}h1,h2{{color:#c6a15b}}.card{{background:#100d09;border:1px solid #3a3020;border-radius:16px;padding:24px;margin:16px 0}}time,.dice{{color:#9a8c70}}@media print{{body{{background:#fff;color:#111}}.card{{background:#fff;border-color:#ccc}}h1,h2{{color:#5d4015}}}}</style></head><body><h1>占星骰子 · 星语档案</h1>{cards}</body></html>")}


pub fn training_markdown(record: &TrainingRecord) -> String {
    let score = if record.ai_score > 0 { record.ai_score } else { record.ai_compare.as_ref().map(|comparison| comparison.score).unwrap_or(0) };
    let score_text = if score > 0 { format!("{} / 100", score) } else { "未评分".to_string() };
    format!(
        "# 占星骰子 · 训练案例\n\n- 时间：{}\n- 问题：{}\n- AI 评分：{}\n\n## 三颗骰子\n{}\n\n## 你的解读\n{}\n\n## 参考解读与对照点评\n{}\n",
        record.created,
        record.question,
        score_text,
        record.dice_text,
        record.user_answer,
        record.ai_compare_text
    )
}
