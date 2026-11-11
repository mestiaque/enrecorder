import { ArrowUpRight, Check, Circle, Copy, Crop, Eraser, Hash, Highlighter, Minus, MousePointer2, Pencil, Redo2, Save, ScanText, Square, Trash2, Type, Undo2, X } from "lucide-react";
import { Dialog } from "../ui/overlays";
import { useCallback, useEffect, useRef, useState, type PointerEvent, type ReactNode } from "react";
import { api, errorMessage } from "../../lib/api";
import { cn } from "../../lib/format";
import type { Screenshot } from "../../lib/types";
import { toast } from "../../store/ui";
import { Button, IconButton, Select } from "../ui/primitives";
import { bounds, drawItem, emptyDoc, hit, move, nextStep, norm, parseDoc, renderExport, type Doc, type Item, type Tool } from "./annotate";

const TOOLS: { id: Tool; label: string; icon: ReactNode; key: string }[] = [
  { id: "select", label: "Select and move", icon: <MousePointer2 className="h-4 w-4" />, key: "v" },
  { id: "arrow", label: "Arrow", icon: <ArrowUpRight className="h-4 w-4" />, key: "a" },
  { id: "rect", label: "Rectangle", icon: <Square className="h-4 w-4" />, key: "r" },
  { id: "ellipse", label: "Ellipse", icon: <Circle className="h-4 w-4" />, key: "e" },
  { id: "line", label: "Line", icon: <Minus className="h-4 w-4" />, key: "l" },
  { id: "pen", label: "Pen", icon: <Pencil className="h-4 w-4" />, key: "p" },
  { id: "highlight", label: "Highlight", icon: <Highlighter className="h-4 w-4" />, key: "h" },
  { id: "blur", label: "Blur (pixelate) — hide passwords and personal data", icon: <Eraser className="h-4 w-4" />, key: "b" },
  { id: "text", label: "Text", icon: <Type className="h-4 w-4" />, key: "t" },
  { id: "step", label: "Numbered step", icon: <Hash className="h-4 w-4" />, key: "n" },
  { id: "crop", label: "Crop", icon: <Crop className="h-4 w-4" />, key: "c" },
];
const COLORS = ["#ef4444", "#f97316", "#eab308", "#22c55e", "#3b82f6", "#a855f7", "#ffffff", "#111111"];
const WIDTHS = [2, 4, 8];
const uid = () => Math.random().toString(36).slice(2, 10);

export function ScreenshotEditor({ shot, startTool = "arrow", onClose }: { shot: Screenshot; startTool?: Tool; onClose: (saved?: Screenshot) => void }) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [img, setImg] = useState<HTMLImageElement | null>(null);
  const [doc, setDoc] = useState<Doc>(() => parseDoc(shot.annotations));
  const [past, setPast] = useState<Doc[]>([]);
  const [future, setFuture] = useState<Doc[]>([]);
  const [tool, setTool] = useState<Tool>(startTool);
  const [color, setColor] = useState(COLORS[0]);
  const [width, setWidth] = useState(4);
  const [selected, setSelected] = useState<string | null>(null);
  const [draft, setDraft] = useState<Item | null>(null);
  const [cropDraft, setCropDraft] = useState<{ x1: number; y1: number; x2: number; y2: number } | null>(null);
  const [textAt, setTextAt] = useState<{ x: number; y: number; sx: number; sy: number } | null>(null);
  const [format, setFormat] = useState<Screenshot["format"]>(shot.format);
  const [busy, setBusy] = useState(false);
  const [ocr, setOcr] = useState<{ text: string; busy: boolean } | null>(null);
  const drag = useRef<{ x: number; y: number; id: string } | null>(null);

  useEffect(() => {
    let alive = true;
    api
      .screenshotImage(shot.id)
      .then((b64) => {
        const i = new Image();
        i.onload = () => alive && setImg(i);
        i.src = `data:image/png;base64,${b64}`;
      })
      .catch((e) => toast({ kind: "error", title: "Could not open the screenshot", detail: errorMessage(e) }));
    return () => {
      alive = false;
    };
  }, [shot.id]);

  const commit = useCallback((next: Doc) => {
    setPast((p) => [...p.slice(-99), doc]);
    setFuture([]);
    setDoc(next);
  }, [doc]);
  const undo = () => past.length && (setFuture((f) => [doc, ...f]), setDoc(past[past.length - 1]), setPast((p) => p.slice(0, -1)));
  const redo = () => future.length && (setPast((p) => [...p, doc]), setDoc(future[0]), setFuture((f) => f.slice(1)));

  // Draw: base image, items, the item being drawn, the crop shade, the selection.
  useEffect(() => {
    const c = canvasRef.current;
    if (!c || !img) return;
    c.width = img.naturalWidth;
    c.height = img.naturalHeight;
    const ctx = c.getContext("2d")!;
    ctx.drawImage(img, 0, 0);
    for (const it of doc.items) drawItem(ctx, img, it);
    if (draft) drawItem(ctx, img, draft);
    const crop = cropDraft ? norm(cropDraft.x1, cropDraft.y1, cropDraft.x2, cropDraft.y2) : doc.crop;
    if (crop && crop.w > 2 && crop.h > 2) {
      ctx.save();
      ctx.fillStyle = "rgba(0,0,0,0.55)";
      ctx.beginPath();
      ctx.rect(0, 0, c.width, c.height);
      ctx.rect(crop.x, crop.y, crop.w, crop.h);
      ctx.fill("evenodd");
      ctx.strokeStyle = "#fff";
      ctx.setLineDash([8, 6]);
      ctx.lineWidth = Math.max(1, c.width / 800);
      ctx.strokeRect(crop.x, crop.y, crop.w, crop.h);
      ctx.restore();
    }
    const sel = doc.items.find((i) => i.id === selected);
    if (sel) {
      const b = bounds(sel);
      ctx.save();
      ctx.strokeStyle = "#3b82f6";
      ctx.setLineDash([6, 4]);
      ctx.lineWidth = Math.max(1.5, c.width / 900);
      ctx.strokeRect(b.x - 6, b.y - 6, b.w + 12, b.h + 12);
      ctx.restore();
    }
  }, [img, doc, draft, cropDraft, selected]);

  const pos = (e: PointerEvent) => {
    const c = canvasRef.current!;
    const r = c.getBoundingClientRect();
    return { x: ((e.clientX - r.left) * c.width) / r.width, y: ((e.clientY - r.top) * c.height) / r.height, sx: e.clientX - r.left, sy: e.clientY - r.top };
  };

  const down = (e: PointerEvent) => {
    if (!img || textAt) return;
    (e.target as HTMLElement).setPointerCapture(e.pointerId);
    const p = pos(e);
    const base = { id: uid(), color, width };
    switch (tool) {
      case "select": {
        const h = hit(doc.items, p.x, p.y, 8 * (canvasRef.current!.width / canvasRef.current!.getBoundingClientRect().width));
        setSelected(h?.id ?? null);
        if (h) {
          setPast((pp) => [...pp.slice(-99), doc]);
          setFuture([]);
          drag.current = { x: p.x, y: p.y, id: h.id };
        }
        break;
      }
      case "text":
        setTextAt(p);
        break;
      case "step":
        commit({ ...doc, items: [...doc.items, { ...base, type: "step", x: p.x, y: p.y, n: nextStep(doc.items) }] });
        break;
      case "pen":
        setDraft({ ...base, type: "pen", points: [[p.x, p.y]] });
        break;
      case "crop":
        setCropDraft({ x1: p.x, y1: p.y, x2: p.x, y2: p.y });
        break;
      default:
        setDraft({ ...base, type: tool, x1: p.x, y1: p.y, x2: p.x, y2: p.y, width: tool === "highlight" ? width : width });
    }
  };

  const moveP = (e: PointerEvent) => {
    const p = pos(e);
    if (drag.current) {
      const { x, y, id } = drag.current;
      drag.current = { x: p.x, y: p.y, id };
      setDoc((d) => ({ ...d, items: d.items.map((it) => (it.id === id ? move(it, p.x - x, p.y - y) : it)) }));
    } else if (cropDraft) setCropDraft({ ...cropDraft, x2: p.x, y2: p.y });
    else if (draft?.type === "pen") setDraft({ ...draft, points: [...draft.points, [p.x, p.y]] });
    else if (draft && "x2" in draft) setDraft({ ...draft, x2: p.x, y2: p.y });
  };

  const up = () => {
    drag.current = null;
    if (cropDraft) {
      const r = norm(cropDraft.x1, cropDraft.y1, cropDraft.x2, cropDraft.y2);
      setCropDraft(null);
      if (r.w > 8 && r.h > 8) commit({ ...doc, crop: r });
    }
    if (draft) {
      const tiny = draft.type === "pen" ? draft.points.length < 2 : "x2" in draft && Math.hypot(draft.x2 - draft.x1, draft.y2 - draft.y1) < 4;
      if (!tiny) commit({ ...doc, items: [...doc.items, draft] });
      setDraft(null);
    }
  };

  const addText = (text: string) => {
    if (textAt && text.trim()) commit({ ...doc, items: [...doc.items, { id: uid(), type: "text", color, width, x: textAt.x, y: textAt.y, text: text.trim(), size: Math.max(18, Math.round((img?.naturalWidth ?? 1920) / 60)) * (width / 4 + 0.5) }] });
    setTextAt(null);
  };

  const removeSelected = useCallback(() => {
    if (!selected) return;
    commit({ ...doc, items: doc.items.filter((i) => i.id !== selected) });
    setSelected(null);
  }, [selected, doc, commit]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (textAt || (e.target as HTMLElement).closest("input,select,textarea")) return;
      const k = e.key.toLowerCase();
      if ((e.ctrlKey || e.metaKey) && k === "z") {
        e.preventDefault();
        if (e.shiftKey) redo();
        else undo();
      } else if ((e.ctrlKey || e.metaKey) && k === "y") {
        e.preventDefault();
        redo();
      } else if ((e.ctrlKey || e.metaKey) && k === "s") {
        e.preventDefault();
        void save();
      } else if ((e.ctrlKey || e.metaKey) && k === "c") {
        e.preventDefault();
        void copy();
      } else if (k === "delete" || k === "backspace") removeSelected();
      else if (k === "escape") onClose();
      else if (!e.ctrlKey && !e.metaKey) {
        const t = TOOLS.find((x) => x.key === k);
        if (t) setTool(t.id);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  const flattened = () => (img ? renderExport(img, doc).toDataURL("image/png") : null);

  async function save() {
    const png = flattened();
    if (!png || busy) return;
    setBusy(true);
    try {
      const saved = await api.saveScreenshot(shot.id, png, format, 90, JSON.stringify(doc));
      toast({ kind: "success", title: "Screenshot saved", detail: saved.path });
      onClose(saved);
    } catch (e) {
      toast({ kind: "error", title: "Could not save", detail: errorMessage(e) });
    } finally {
      setBusy(false);
    }
  }

  /** Text in the current image (crop applied, blurred areas stay unreadable), via Tesseract. */
  async function readText() {
    const png = flattened();
    if (!png) return;
    setOcr({ text: "", busy: true });
    try {
      const st = await api.ocrStatus();
      const langs = ["eng", "ben"].filter((l) => st.languages.includes(l)).join("+") || "eng";
      const text = await api.ocrScreenshot(png, langs);
      setOcr({ text: text || "(no text found)", busy: false });
    } catch (e) {
      setOcr(null);
      toast({ kind: "error", title: "Could not read the text", detail: errorMessage(e) }, 8000);
    }
  }

  async function copy() {
    const png = flattened();
    if (!png) return;
    try {
      await api.copyScreenshot(png);
      toast({ kind: "success", title: "Copied to the clipboard" });
    } catch (e) {
      toast({ kind: "error", title: "Could not copy", detail: errorMessage(e) });
    }
  }

  return (
    <div className="fixed inset-0 z-[60] flex flex-col bg-bg animate-fade-in" role="dialog" aria-label="Screenshot editor">
      <header className="flex h-12 shrink-0 items-center gap-1 border-b border-line bg-panel px-2">
        <div className="flex items-center gap-0.5 rounded-md border border-line bg-bg p-0.5">
          {TOOLS.map((t) => (
            <IconButton key={t.id} label={t.label} shortcut={t.key.toUpperCase()} active={tool === t.id} onClick={() => setTool(t.id)} className="h-7 w-7">
              {t.icon}
            </IconButton>
          ))}
        </div>
        <div className="mx-2 flex items-center gap-1">
          {COLORS.map((c) => (
            <button key={c} aria-label={`Colour ${c}`} title={c} onClick={() => setColor(c)} className={cn("h-5 w-5 rounded-full border border-line", color === c && "ring-2 ring-accent ring-offset-1 ring-offset-panel")} style={{ background: c }} />
          ))}
        </div>
        <div className="flex items-center gap-0.5">
          {WIDTHS.map((w) => (
            <IconButton key={w} label={`Thickness ${w}`} active={width === w} onClick={() => setWidth(w)} className="h-7 w-7">
              <span className="block rounded-full bg-current" style={{ width: w + 4, height: w + 4 }} />
            </IconButton>
          ))}
        </div>
        <div className="mx-1 h-5 w-px bg-line" />
        <IconButton label="Undo" shortcut="Ctrl+Z" disabled={!past.length} onClick={undo}>
          <Undo2 className="h-4 w-4" />
        </IconButton>
        <IconButton label="Redo" shortcut="Ctrl+Y" disabled={!future.length} onClick={redo}>
          <Redo2 className="h-4 w-4" />
        </IconButton>
        <IconButton label="Delete the selected item" shortcut="Del" disabled={!selected} onClick={removeSelected}>
          <Trash2 className="h-4 w-4" />
        </IconButton>
        {doc.crop && (
          <Button size="sm" variant="ghost" onClick={() => commit({ ...doc, crop: null })}>
            Remove crop
          </Button>
        )}
        <span className="flex-1" />
        <Button size="sm" variant="ghost" onClick={() => commit(emptyDoc())} disabled={!doc.items.length && !doc.crop}>
          Reset
        </Button>
        <Button size="sm" variant="ghost" onClick={() => void readText()} disabled={!img} title="Read the text in the image (OCR, on this computer)">
          <ScanText className="h-3.5 w-3.5" />
          Text
        </Button>
        <Select value={format} onChange={(v) => setFormat(v)} options={[{ value: "png", label: "PNG" }, { value: "jpeg", label: "JPEG" }, { value: "webp", label: "WebP" }]} />
        <Button size="sm" onClick={() => void copy()} disabled={!img}>
          <Copy className="h-3.5 w-3.5" />
          Copy
        </Button>
        <Button size="sm" variant="primary" onClick={() => void save()} disabled={!img || busy}>
          <Save className="h-3.5 w-3.5" />
          {busy ? "Saving…" : "Save"}
        </Button>
        <IconButton label="Close" shortcut="Esc" onClick={() => onClose()}>
          <X className="h-4 w-4" />
        </IconButton>
      </header>
      <div className="relative flex min-h-0 flex-1 items-center justify-center overflow-hidden p-6" style={{ background: "repeating-conic-gradient(rgb(var(--raised)) 0 25%, rgb(var(--bg)) 0 50%) 0 0/20px 20px" }}>
        {!img && <div className="text-[13px] text-muted">Opening…</div>}
        <div className="relative max-h-full max-w-full">
          <canvas
            ref={canvasRef}
            onPointerDown={down}
            onPointerMove={moveP}
            onPointerUp={up}
            className={cn("block max-h-[calc(100vh-7rem)] max-w-full rounded shadow-pop", tool === "select" ? "cursor-default" : "cursor-crosshair", !img && "hidden")}
          />
          {textAt && (
            <input
              autoFocus
              placeholder="Type, then Enter"
              className="absolute z-10 h-8 min-w-[180px] rounded border border-accent bg-panel px-2 text-[14px] text-fg outline-none"
              style={{ left: textAt.sx, top: textAt.sy - 32 }}
              onKeyDown={(e) => {
                e.stopPropagation();
                if (e.key === "Enter") addText(e.currentTarget.value);
                if (e.key === "Escape") setTextAt(null);
              }}
              onBlur={(e) => addText(e.currentTarget.value)}
            />
          )}
        </div>
      </div>
      {ocr && (
        <Dialog
          open
          onOpenChange={(o) => !o && setOcr(null)}
          title="Text in the screenshot"
          description="Recognised on this computer with Tesseract. Check it before using it."
          footer={
            <>
              <Button onClick={() => setOcr(null)}>Close</Button>
              <Button variant="primary" disabled={ocr.busy || !ocr.text} onClick={() => void api.copyText(ocr.text).then(() => toast({ kind: "success", title: "Text copied" }), () => toast({ kind: "error", title: "Could not copy" }))}>
                <Copy className="h-3.5 w-3.5" />
                Copy text
              </Button>
            </>
          }
        >
          {ocr.busy ? <div className="py-6 text-center text-[13px] text-muted">Reading…</div> : <textarea readOnly value={ocr.text} className="h-64 w-full resize-none rounded-md border border-line bg-bg p-2 font-mono text-[12px] outline-none" />}
        </Dialog>
      )}
      <footer className="flex h-7 shrink-0 items-center gap-4 border-t border-line bg-panel px-3 text-[11px] text-muted">
        <span>{img ? `${img.naturalWidth}×${img.naturalHeight}` : "…"}</span>
        {doc.crop && <span>Crop {Math.round(doc.crop.w)}×{Math.round(doc.crop.h)}</span>}
        <span>{doc.items.length} annotation{doc.items.length === 1 ? "" : "s"}</span>
        <span className="flex-1" />
        <span className="flex items-center gap-1">
          <Check className="h-3 w-3" />
          The original is kept — annotations stay editable after saving
        </span>
      </footer>
    </div>
  );
}
