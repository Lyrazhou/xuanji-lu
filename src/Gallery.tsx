import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { convertFileSrc } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Download, Trash2 } from "lucide-react";
import type { AppSnapshot, GalleryItem, Preferences } from "./types";

const PAGE_SIZE = 10;
const localImage = (url: string) => url.startsWith("http") || url.startsWith("data:") ? url : convertFileSrc(url);
const safeFileName = (value: string) => value.replace(/[\\/:*?"<>|\s]/g, "").slice(0, 18) || "星语图片";
const extensionOf = (value: string) => {
  const match = value.split(/[?#]/)[0].match(/\.(png|jpe?g|webp)$/i);
  return (match?.[1] || "png").toLowerCase().replace("jpeg", "jpg");
};
const directoryOf = (path: string) => path.replace(/[\\/][^\\/]*$/, "");
const pathWithFile = (directory: string, file: string) => directory ? `${directory.replace(/[\\/]+$/, "")}\\${file}` : file;

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

export default function Gallery({ snapshot, busy, setBusy, notice, reload, setPrefs }: { snapshot: AppSnapshot; busy: string; setBusy: (x: string) => void; notice: (x: string, error?: boolean) => void; reload: () => Promise<void>; setPrefs: (prefs: Preferences) => Promise<void> }) {
  const [selected, setSelected] = useState<GalleryItem | null>(null);
  const [page, setPage] = useState(1);
  const pageCount = Math.max(1, Math.ceil(snapshot.galleryItems.length / PAGE_SIZE));
  const pageItems = snapshot.galleryItems.slice((page - 1) * PAGE_SIZE, page * PAGE_SIZE);
  useEffect(() => { if (page > pageCount) setPage(pageCount); }, [page, pageCount]);

  const remove = async (item: GalleryItem) => {
    if (!window.confirm("确定从本地星语图库删除这张图片记录吗？此操作不会删除原始图片文件。")) return;
    setBusy(item.id);
    try {
      await invoke("delete_record", { kind: "gallery", id: item.id });
      await reload();
      if (selected?.id === item.id) setSelected(null);
      notice("已从图库删除记录；原始图片文件仍保留在本地图片目录。");
    } catch (error) { notice(String(error), true); } finally { setBusy(""); }
  };

  const openOriginal = async (item: GalleryItem) => {
    try {
      if (item.imageUrl.startsWith("http")) await openUrl(item.imageUrl);
      else if (item.imageUrl.startsWith("data:")) return notice("此旧图片仍是内嵌数据，无法调用系统查看器。请先重新生成或另存为本地文件。", true);
      else await invoke("open_local_image", { sourcePath: item.imageUrl });
    } catch (error) { notice(`无法用系统默认图片查看器打开原图：${String(error)}`, true); }
  };

  const saveImage = async (item: GalleryItem) => {
    if (item.imageUrl.startsWith("http") || item.imageUrl.startsWith("data:")) return notice("此旧图片尚未归档为本地文件，无法直接另存。请重新生成该图片。", true);
    const extension = extensionOf(item.imageUrl);
    const defaultName = `璇玑录_${safeFileName(item.question)}_${item.created.slice(0, 10).replaceAll("-", "")}.${extension}`;
    const target = await save({
      defaultPath: pathWithFile(snapshot.prefs.lastImageSaveDirectory, defaultName),
      filters: [{ name: "图片", extensions: [extension] }]
    });
    if (!target) return;
    setBusy(`save-image-${item.id}`);
    try {
      await invoke("copy_local_image_to", { sourcePath: item.imageUrl, destinationPath: target });
      const directory = directoryOf(target);
      if (directory && directory !== snapshot.prefs.lastImageSaveDirectory) await setPrefs({ ...snapshot.prefs, lastImageSaveDirectory: directory });
      notice("图片已另存到所选位置；下次另存将默认打开此目录。");
    } catch (error) { notice(String(error), true); } finally { setBusy(""); }
  };

  return <>
    <div className="row space-between"><div><div className="eyebrow">STAR GALLERY</div><h1>星语图库</h1><p className="lead">每一次占卜都会落成一帧星语。在这里慢慢翻页、调用系统查看器查看原图，或另存到你选择的位置。</p></div></div>
    {snapshot.galleryItems.length === 0 ? <div className="empty"><div>图库仍是深空。完成一次占问后，可选择“骰子壁纸”或“星语海报”生成第一张本地星语图。</div></div> : <><div className="gallery">{pageItems.map((item) => <div className="gallery-item" key={item.id} onClick={() => setSelected(item)}><SafeImage src={item.imageUrl} alt={`星语图库：${item.question}`} /><div className="gallery-meta"><div className="gold">{item.mode === "dice" ? "骰子壁纸" : item.mode === "custom" ? "自定义图片输出" : "星语海报"}</div><div>{item.question}</div></div></div>)}</div>{pageCount > 1 && <div className="pagination"><button className="ghost" disabled={page <= 1} onClick={() => setPage(page - 1)}>上一页</button><span className="mono muted">第 {page} / {pageCount} 页 · 共 {snapshot.galleryItems.length} 张</span><button className="ghost" disabled={page >= pageCount} onClick={() => setPage(page + 1)}>下一页</button></div>}</>}
    {selected && <div className="modal-shade" onClick={() => setSelected(null)}><div className="modal gallery-modal" onClick={(event) => event.stopPropagation()}><div className="panel-title"><div><div className="eyebrow">STAR GALLERY</div><h2>{selected.question}</h2></div><button className="ghost" onClick={() => setSelected(null)}>关闭</button></div><SafeImage className="modal-image" src={selected.imageUrl} alt={`星语图库大图：${selected.question}`} /><p className="muted small" style={{ marginTop: 12 }}>{selected.diceText}</p><div className="row wrap"><button className="secondary" onClick={() => void openOriginal(selected)}>打开原图</button><button className="primary" disabled={busy === `save-image-${selected.id}`} onClick={() => void saveImage(selected)}><Download size={14} /> {busy === `save-image-${selected.id}` ? "另存中" : "另存图片"}</button><button className="danger" disabled={busy === selected.id} onClick={() => void remove(selected)}><Trash2 size={14} /> 删除记录</button></div></div></div>}
  </>;
}
