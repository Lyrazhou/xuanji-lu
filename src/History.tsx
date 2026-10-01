import { useEffect, useMemo, useRef, useState, type CSSProperties } from "react";
import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { convertFileSrc } from "@tauri-apps/api/core";
import { Download, FileDown, FileUp, Square, Trash2, Volume2 } from "lucide-react";
import type { AppSnapshot, DivinationRecord, Preferences, Tag, TrainingRecord, TtsAudio } from "./types";

const PAGE_SIZE = 10;
const dateText = (value: string) => value ? new Intl.DateTimeFormat("zh-CN", { dateStyle: "medium", timeStyle: "short" }).format(new Date(value)) : "—";
const safeFileName = (value: string) => value.replace(/[\\/:*?"<>|\s]/g, "").slice(0, 12) || "星语档案";
const localImage = (url: string) => url.startsWith("http") || url.startsWith("data:") ? url : convertFileSrc(url);
const tagStyle = (tag?: Tag): CSSProperties => ({ "--tag-color": tag?.color || "#8a8a8a" } as CSSProperties);
const SYSTEM_TAG_NAMES = ["自动", "手动", "图片"];
const trainingScore = (record: TrainingRecord) => record.aiScore || record.aiCompare?.score || 0;

function SafeImage({ src, alt, className }: { src: string; alt: string; className?: string }) {
  const [failed, setFailed] = useState(false);
  const [fallbackSrc, setFallbackSrc] = useState("");
  useEffect(() => { setFailed(false); setFallbackSrc(""); }, [src]);
  const recoverLocalImage = async () => {
    if (fallbackSrc || src.startsWith("http") || src.startsWith("data:")) return setFailed(true);
    try { setFallbackSrc(await invoke<string>("read_local_image_data_url", { sourcePath: src })); }
    catch { setFailed(true); }
  };
  return failed ? <div className={`${className ?? ""} image-unavailable`} role="status">本地图片不可读取</div> : <img className={className} src={fallbackSrc || localImage(src)} alt={alt} onError={() => void recoverLocalImage()} />;
}

function Tags({ tags, names }: { tags: Tag[]; names: string[] }) {
  return <div className="row wrap tags-indicator">{names.map((name) => { const tag = tags.find((item) => item.name === name); return <span className="tag" style={tagStyle(tag)} key={name}>{name}</span>; })}</div>;
}

function StarRating({ value, onChange }: { value: number; onChange?: (rating: number) => void }) {
  return <div className="star-rating">{[1, 2, 3, 4, 5].map((rating) => <button key={rating} className={`star ${rating <= value ? "filled" : ""}`} onClick={() => onChange?.(rating === value ? 0 : rating)} disabled={!onChange} aria-label={`${rating}星`}>★</button>)}</div>;
}

function Pagination({ current, total, onChange }: { current: number; total: number; onChange: (page: number) => void }) {
  if (total <= 1) return null;
  return <div className="pagination"><button className="ghost" disabled={current <= 1} onClick={() => onChange(current - 1)}>上一页</button><span className="mono muted">第 {current} / {total} 页</span><button className="ghost" disabled={current >= total} onClick={() => onChange(current + 1)}>下一页</button></div>;
}

function VoiceOutput({ snapshot, setPrefs, busy, setBusy, notice, text, fileStem, compact = false }: { snapshot: AppSnapshot; setPrefs: (prefs: Preferences) => Promise<void>; busy: string; setBusy: (value: string) => void; notice: (message: string, error?: boolean) => void; text: string; fileStem: string; compact?: boolean }) {
  const audioContextRef = useRef<AudioContext | null>(null);
  const sourceRef = useRef<AudioBufferSourceNode | null>(null);
  const [playing, setPlaying] = useState(false);
  const [prepared, setPrepared] = useState<{ key: string; audio: TtsAudio } | null>(null);
  const key = `${snapshot.prefs.ttsVoiceShortName}|${text}`;
  const stop = () => { try { sourceRef.current?.stop(); } catch { /* 已停止的音源无需再次处理 */ } sourceRef.current = null; setPlaying(false); };
  useEffect(() => () => { try { sourceRef.current?.stop(); } catch { /* 已停止的音源无需再次处理 */ } sourceRef.current = null; void audioContextRef.current?.close(); audioContextRef.current = null; }, []);
  const unlockAudio = async () => { if (!audioContextRef.current) audioContextRef.current = new AudioContext(); if (audioContextRef.current.state === "suspended") await audioContextRef.current.resume(); return audioContextRef.current; };
  const prepare = async () => {
    if (!snapshot.prefs.ttsVoiceShortName) { notice("请先在工作室偏好中读取并选择微软中文音色。", true); return null; }
    if (!text.trim()) { notice("没有可朗读的文字。", true); return null; }
    if (prepared?.key === key) return prepared.audio;
    setBusy("tts-synthesis");
    try { const audio = await invoke<TtsAudio>("synthesize_edge_tts", { voiceShortName: snapshot.prefs.ttsVoiceShortName, text }); setPrepared({ key, audio }); return audio; }
    catch (error) { notice(String(error), true); return null; }
    finally { setBusy(""); }
  };
  const toggle = async () => {
    if (playing) { stop(); return; }
    let audioContext: AudioContext;
    try { audioContext = await unlockAudio(); }
    catch { notice("当前系统无法初始化语音播放设备，请检查系统输出设备后重试。", true); return; }
    const audio = await prepare();
    if (!audio) return;
    try { const raw = atob(audio.audioBase64); const bytes = new Uint8Array(raw.length); for (let index = 0; index < raw.length; index += 1) bytes[index] = raw.charCodeAt(index); const buffer = new ArrayBuffer(bytes.length); new Uint8Array(buffer).set(bytes); const decoded = await audioContext.decodeAudioData(buffer); const source = audioContext.createBufferSource(); source.buffer = decoded; source.connect(audioContext.destination); source.onended = () => { if (sourceRef.current === source) { sourceRef.current = null; setPlaying(false); } }; sourceRef.current = source; source.start(0); setPlaying(true); }
    catch { stop(); notice("语音音频无法在应用内解码或播放，请检查系统音频设备后重试。", true); }
  };
  const exportAudio = async () => {
    const audio = await prepare();
    if (!audio) return;
    const name = `璇玑录_${safeFileName(fileStem)}_${new Date().toISOString().replace(/[-:.TZ]/g, "").slice(0, 14)}.${audio.fileExtension || "mp3"}`;
    const target = await save({ defaultPath: snapshot.prefs.lastAudioSaveDirectory ? `${snapshot.prefs.lastAudioSaveDirectory.replace(/[\\/]+$/, "")}\\${name}` : name, filters: [{ name: "MP3 音频", extensions: [audio.fileExtension || "mp3"] }] });
    if (!target) return;
    setBusy("tts-save");
    try { await invoke("save_tts_audio", { destinationPath: target, audioBase64: audio.audioBase64 }); const directory = target.replace(/[\\/][^\\/]*$/, ""); if (directory && directory !== snapshot.prefs.lastAudioSaveDirectory) await setPrefs({ ...snapshot.prefs, lastAudioSaveDirectory: directory }); notice("语音已另存到所选位置；音频不会写入历史档案。"); }
    catch (error) { notice(String(error), true); }
    finally { setBusy(""); }
  };
  return <div className={`voice-output ${compact ? "voice-output-compact" : ""}`} aria-label="语音输出"><button className={`icon-action voice-play ${playing ? "playing" : ""}`} title={playing ? "停止播放" : "播放语音"} aria-label={playing ? "停止播放语音" : "播放语音"} disabled={busy === "tts-synthesis"} onClick={() => void toggle()}>{playing ? <Square size={18} fill="currentColor" /> : <Volume2 size={22} />}</button>{!compact && <><button className="icon-action voice-save" title="另存语音" aria-label="另存语音" disabled={busy === "tts-save" || busy === "tts-synthesis"} onClick={() => void exportAudio()}><Download size={19} /></button><span className="muted small">{playing ? "朗读中，再按停止" : "语音输出"}</span></>}</div>;
}

export default function History({ snapshot, busy, setBusy, notice, reload, setPrefs, exportExchange, importExchange }: { snapshot: AppSnapshot; busy: string; setBusy: (value: string) => void; notice: (message: string, error?: boolean) => void; reload: () => Promise<void>; setPrefs: (prefs: Preferences) => Promise<void>; exportExchange: () => Promise<void>; importExchange: () => Promise<void> }) {
  const [tab, setTab] = useState<"div" | "training">("div");
  const [search, setSearch] = useState("");
  const [selectedTags, setSelectedTags] = useState<string[]>([]);
  const [page, setPage] = useState(1);
  const [viewingDivination, setViewingDivination] = useState<DivinationRecord | null>(null);
  const [viewingTraining, setViewingTraining] = useState<TrainingRecord | null>(null);

  const records = useMemo(() => snapshot.divinationRecords.filter((record) => {
    const matchesSearch = !search || `${record.question}${record.reviewNote}${record.summaryText}`.toLowerCase().includes(search.toLowerCase());
    const matchesTags = selectedTags.length === 0 || selectedTags.every((name) => record.tagNames.includes(name));
    return matchesSearch && matchesTags;
  }), [snapshot.divinationRecords, search, selectedTags]);
  const training = useMemo(() => snapshot.trainingRecords.filter((record) => !search || `${record.question}${record.userAnswer}${record.aiCompareText}`.toLowerCase().includes(search.toLowerCase())), [snapshot.trainingRecords, search]);
  const activeRows = tab === "div" ? records : training;
  const pageCount = Math.max(1, Math.ceil(activeRows.length / PAGE_SIZE));
  const pagedRows = activeRows.slice((page - 1) * PAGE_SIZE, page * PAGE_SIZE);
  useEffect(() => { if (page > pageCount) setPage(pageCount); }, [page, pageCount]);

  const selectTab = (next: "div" | "training") => { setTab(next); setPage(1); setViewingDivination(null); setViewingTraining(null); };
  const toggleFilter = (name: string) => { setSelectedTags((current) => current.includes(name) ? current.filter((item) => item !== name) : [...current, name]); setPage(1); };
  const remove = async (kind: "divination" | "training", id: string) => {
    if (!window.confirm("确定删除这条本地记录吗？此操作不会影响您已导出的档案包。")) return;
    if (!window.confirm("请再次确认：删除后无法从软件内恢复这条本地案例。确定永久删除吗？")) return;
    setBusy(id);
    try { await invoke("delete_record", { kind, id }); await reload(); if (viewingDivination?.id === id) setViewingDivination(null); if (viewingTraining?.id === id) setViewingTraining(null); notice("已删除本地记录。"); } catch (error) { notice(String(error), true); } finally { setBusy(""); }
  };
  const updateMeta = async (record: DivinationRecord, change: Partial<Pick<DivinationRecord, "rating" | "reviewNote" | "tagNames">>) => {
    setBusy(record.id);
    try {
      await invoke("update_divination_meta", { id: record.id, reviewNote: change.reviewNote ?? record.reviewNote, rating: change.rating ?? record.rating, tagNames: change.tagNames ?? record.tagNames });
      const updated = { ...record, ...change };
      setViewingDivination(updated);
      await reload();
    } catch (error) { notice(String(error), true); } finally { setBusy(""); }
  };
  const exportOne = async (record: DivinationRecord) => {
    const path = await save({ defaultPath: `占星骰子记录_${new Date().toISOString().slice(0, 10).replaceAll("-", "")}_${safeFileName(record.question)}.md`, filters: [{ name: "Markdown", extensions: ["md"] }] });
    if (!path) return;
    try { await invoke("export_divination_markdown", { id: record.id, path }); notice("已导出 Markdown 记录。"); } catch (error) { notice(String(error), true); }
  };
  const exportTraining = async (record: TrainingRecord) => {
    const path = await save({ defaultPath: `占星训练_${new Date().toISOString().slice(0, 10).replaceAll("-", "")}_${safeFileName(record.question)}.md`, filters: [{ name: "Markdown", extensions: ["md"] }] });
    if (!path) return;
    try { await invoke("export_training_markdown", { id: record.id, path }); notice("已导出 Markdown 训练案例。"); } catch (error) { notice(String(error), true); }
  };
  const exportBatch = async (format: "csv" | "html") => {
    const path = await save({ defaultPath: format === "csv" ? "占星骰子记录.csv" : "占星骰子记录.html", filters: [{ name: format.toUpperCase(), extensions: [format] }] });
    if (!path) return;
    try { await invoke(format === "csv" ? "export_divinations_csv" : "export_divinations_html", { ids: records.map((record) => record.id), path }); notice(`已导出 ${records.length} 条筛选记录。`); } catch (error) { notice(String(error), true); }
  };

  return <>
    <div className="row space-between"><div><div className="eyebrow">STAR ARCHIVE</div><h1>星语档案</h1><p className="lead">每一次占卜、每一场训练都存放在这里。通过关键词和标签寻找星语，打开独立查看窗完成复盘。</p></div><div className="row"><button className="ghost" onClick={importExchange}><FileUp size={14} /> 导入 SDE-1</button><button className="secondary" onClick={exportExchange}><FileDown size={14} /> 导出标准档案</button></div></div>
    <div className="panel panel-pad"><div className="tabs"><button className={`tab ${tab === "div" ? "active" : ""}`} onClick={() => selectTab("div")}>占卜记录 · {snapshot.divinationRecords.length}</button><button className={`tab ${tab === "training" ? "active" : ""}`} onClick={() => selectTab("training")}>训练场 · {snapshot.trainingRecords.length}</button></div>
      <div className="field"><label>关键词搜索</label><input value={search} onChange={(event) => { setSearch(event.target.value); setPage(1); }} placeholder="问题、复盘或解读内容" /></div>
      {tab === "div" && <div className="field" style={{ marginTop: 15 }}><label>标签过滤</label><div className="row wrap filter-tags">{snapshot.tags.length === 0 ? <span className="muted small">尚未建立标签；自动、手动和图片标签会在首次使用时自动加入。</span> : snapshot.tags.map((tag) => <button key={tag.id} className={`tag tag-toggle ${selectedTags.includes(tag.name) ? "active" : ""}`} style={tagStyle(tag)} onClick={() => toggleFilter(tag.name)}>{tag.name}</button>)}</div><span className="muted small">默认不选择任何标签，显示全部历史；选择多个标签时，仅显示同时具备全部所选标签的记录。</span></div>}
      {tab === "div" && <div className="row wrap" style={{ marginTop: 16 }}><button className="ghost" onClick={() => void exportBatch("csv")}><Download size={14} /> 导出 CSV</button><button className="ghost" onClick={() => void exportBatch("html")}><Download size={14} /> 导出 HTML</button><span className="muted small">当前筛选：{records.length} 条</span></div>}
      <div className="archive-list">{pagedRows.length === 0 ? <div className="empty"><div>{tab === "div" ? "还没有匹配的占卜记录。回到“星骰占问”，让第一段星语落进档案。" : "还没有匹配的训练案例。前往训练场，让星辰为你出一道题。"}</div></div> : tab === "div" ? (pagedRows as DivinationRecord[]).map((record) => <div className="record record-summary" key={record.id}><div className="record-summary-content"><div className="mono muted">{dateText(record.created)}</div><div className="row wrap record-question-row"><h3>{record.question}</h3><VoiceOutput compact snapshot={snapshot} setPrefs={setPrefs} busy={busy} setBusy={setBusy} notice={notice} text={`历史占问的问题是：${record.question}。${record.reading?.diceReadings.map((card) => `${card.diceLabel}，${card.faceName}。${card.meaning}`).join("。") || ""}。综合解读：${record.reading?.summary || record.summaryText || ""}`} fileStem={`历史占问_${record.question}`} /><Tags tags={snapshot.tags} names={record.tagNames} /></div><p>{record.dice.planet} · {record.dice.zodiac} · {record.dice.house} · {record.summaryText.slice(0, 103)}{record.summaryText.length > 103 ? "…" : ""}</p></div><div className="record-actions"><button className="record-view" onClick={() => setViewingDivination(record)}>查看</button><button className="ghost icon-button" onClick={() => void exportOne(record)} aria-label="导出记录"><Download size={14} /></button><button className="danger icon-button" disabled={busy === record.id} onClick={() => void remove("divination", record.id)} aria-label="删除记录"><Trash2 size={14} /></button></div></div>) : (pagedRows as TrainingRecord[]).map((record) => <div className="record record-summary" key={record.id}><div className="record-summary-content"><div className="mono muted">{dateText(record.created)}</div><div className="row wrap record-question-row"><h3>{record.question}</h3><VoiceOutput compact snapshot={snapshot} setPrefs={setPrefs} busy={busy} setBusy={setBusy} notice={notice} text={`历史训练点评。题目：${record.question}。三颗骰子：${record.diceText}。AI 评分：${trainingScore(record) ? `${trainingScore(record)} 分` : "未评分"}。${record.aiCompareText}`} fileStem={`历史训练点评_${record.question}`} /><span className={`training-score-badge ${trainingScore(record) ? "" : "unscored"}`}>AI 评分 · {trainingScore(record) ? `${trainingScore(record)} 分` : "未评分"}</span></div><p>{record.diceText} · {record.aiCompareText.slice(0, 103)}{record.aiCompareText.length > 103 ? "…" : ""}</p></div><div className="record-actions"><button className="record-view" onClick={() => setViewingTraining(record)}>查看</button><button className="ghost icon-button" onClick={() => void exportTraining(record)} aria-label="导出训练"><Download size={14} /></button><button className="danger icon-button" disabled={busy === record.id} onClick={() => void remove("training", record.id)} aria-label="删除训练"><Trash2 size={14} /></button></div></div>)}</div>
      <Pagination current={page} total={pageCount} onChange={setPage} />
    </div>
    {viewingDivination && <div className="modal-shade archive-modal-shade" onClick={() => setViewingDivination(null)}><div className="modal archive-modal" onClick={(event) => event.stopPropagation()}><div className="panel-title"><div><div className="eyebrow">DIVINATION RECORD</div><h2>{viewingDivination.question}</h2><div className="mono muted">{dateText(viewingDivination.created)}</div></div><button className="ghost" onClick={() => setViewingDivination(null)}>关闭</button></div><Tags tags={snapshot.tags} names={viewingDivination.tagNames} /><div className="record-detail"><div className="reading-card"><h3>三颗骰子</h3><p>{viewingDivination.dice.planet} · {viewingDivination.dice.zodiac} · {viewingDivination.dice.house}</p></div>{viewingDivination.reading?.diceReadings.map((card) => <div className="reading-card" key={card.diceLabel} style={{ marginTop: 10 }}><h3>{card.faceGlyph} {card.diceLabel} · {card.faceName}</h3><p>{card.meaning}</p></div>)}<div className="summary"><h3>综合解读</h3><p>{viewingDivination.reading?.summary || viewingDivination.summaryText}</p><VoiceOutput snapshot={snapshot} setPrefs={setPrefs} busy={busy} setBusy={setBusy} notice={notice} text={`历史占问的问题是：${viewingDivination.question}。${viewingDivination.reading?.diceReadings.map((card) => `${card.diceLabel}，${card.faceName}。${card.meaning}`).join("。") || ""}。综合解读：${viewingDivination.reading?.summary || viewingDivination.summaryText || ""}`} fileStem={`历史占问_${viewingDivination.question}`} /></div>{viewingDivination.imageUrl && <SafeImage className="modal-image" src={viewingDivination.imageUrl} alt="本条占卜关联的星语图" />}</div><div className="grid-two" style={{ marginTop: 16 }}><div className="field"><label>复盘验证（自动保存）</label><textarea defaultValue={viewingDivination.reviewNote} key={`${viewingDivination.id}-${viewingDivination.updated}`} onBlur={(event) => event.target.value !== viewingDivination.reviewNote && void updateMeta(viewingDivination, { reviewNote: event.target.value })} /></div><div className="stack"><div><label className="muted small">评分</label><StarRating value={viewingDivination.rating} onChange={(rating) => void updateMeta(viewingDivination, { rating })} /></div><div><label className="muted small">标签池</label><div className="row wrap filter-tags" style={{ marginTop: 7 }}>{snapshot.tags.filter((tag) => !SYSTEM_TAG_NAMES.includes(tag.name)).map((tag) => <button key={tag.id} className={`tag tag-toggle ${viewingDivination.tagNames.includes(tag.name) ? "active" : ""}`} style={tagStyle(tag)} onClick={() => void updateMeta(viewingDivination, { tagNames: viewingDivination.tagNames.includes(tag.name) ? viewingDivination.tagNames.filter((name) => name !== tag.name) : [...viewingDivination.tagNames, tag.name] })}>{tag.name}</button>)}</div></div></div></div><div className="row" style={{ marginTop: 16 }}><button className="secondary" onClick={() => void exportOne(viewingDivination)}><Download size={14} /> 导出本条</button><button className="ghost" onClick={() => setViewingDivination(null)}>关闭</button></div></div></div>}
    {viewingTraining && <div className="modal-shade archive-modal-shade" onClick={() => setViewingTraining(null)}><div className="modal archive-modal" onClick={(event) => event.stopPropagation()}><div className="panel-title"><div><div className="eyebrow">TRAINING RECORD</div><h2>{viewingTraining.question}</h2><div className="mono muted">{dateText(viewingTraining.created)}</div></div><button className="ghost" onClick={() => setViewingTraining(null)}>关闭</button></div><div className="reading-card"><h3>三颗骰子</h3><p>{viewingTraining.diceText}</p></div><div className={`training-score ${trainingScore(viewingTraining) ? "" : "unscored"}`} style={{ marginTop: 12 }}><span>AI 评分</span><strong>{trainingScore(viewingTraining) ? `${trainingScore(viewingTraining)} / 100` : "未评分"}</strong></div><div className="summary"><h3>你的解读</h3><p style={{ whiteSpace: "pre-wrap" }}>{viewingTraining.userAnswer}</p></div><div className="summary"><h3>参考解读与对照点评</h3><p style={{ whiteSpace: "pre-wrap" }}>{viewingTraining.aiCompareText}</p><VoiceOutput snapshot={snapshot} setPrefs={setPrefs} busy={busy} setBusy={setBusy} notice={notice} text={`历史训练点评。题目：${viewingTraining.question}。三颗骰子：${viewingTraining.diceText}。AI 评分：${trainingScore(viewingTraining) ? `${trainingScore(viewingTraining)} 分` : "未评分"}。${viewingTraining.aiCompareText}`} fileStem={`历史训练点评_${viewingTraining.question}`} /></div><div className="row" style={{ marginTop: 16 }}><button className="secondary" onClick={() => void exportTraining(viewingTraining)}><Download size={14} /> 导出本条</button><button className="ghost" onClick={() => setViewingTraining(null)}>关闭</button></div></div></div>}
  </>;
}
