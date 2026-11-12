/**
 * Quick capture overlay (Flameshot / ShareX style). The frozen desktop fills the screen; drag to
 * choose an area, adjust it with the handles, draw on it in place, then Enter copies it, Ctrl+S
 * saves it to the library (and copies), "Studio" opens it in the full editor, Esc cancels.
 * Everything is drawn in image pixels with the shared annotation code, so what you see is what
 * is saved, and a saved capture stays editable in the Screenshot Studio.
 */
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  ArrowUpRight,
  Check,
  Circle,
  Copy,
  Droplets,
  Hash,
  Highlighter,
  Minus,
  Move,
  Pencil,
  PenLine,
  Redo2,
  Save,
  Square,
  Type,
  Undo2,
  X,
} from "lucide-react";
import { useCallback, useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { api, errorMessage } from "../lib/api";
import { defaultFinish, drawItem, nextStep, norm, renderExport, type Item, type Rect } from "../components/screenshot/annotate";

type DrawTool = "move" | "arrow" | "rect" | "ellipse" | "line" | "pen" | "highlight" | "blur" | "text" | "step";
type Handle = "nw" | "n" | "ne" | "e" | "se" | "s" | "sw" | "w";
type Drag =
  | { kind: "select"; x0: number; y0: number }
  | { kind: "move"; x0: number; y0: number; r: Rect }
  | { kind: "resize"; h: Handle; r: Rect }
  | { kind: "draw"; item: Item };

const COLORS = ["#ef4444", "#f59e0b", "#22c55e", "#3b82f6", "#ffffff", "#111111"];
const WIDTHS = [2, 4, 8];
const TEXT_SIZE: Record<number, number> = { 2: 20, 4: 28, 8: 44 };
const TOOLS: { id: DrawTool; icon: ReactNode; label: string; key: string }[] = [
  { id: "move", icon: <Move className="h-4 w-4" />, label: "Move / resize the area", key: "m" },
  { id: "arrow", icon: <ArrowUpRight className="h-4 w-4" />, label: "Arrow", key: "a" },
  { id: "rect", icon: <Square className="h-4 w-4" />, label: "Rectangle", key: "r" },
  { id: "ellipse", icon: <Circle className="h-4 w-4" />, label: "Ellipse", key: "e" },
  { id: "line", icon: <Minus className="h-4 w-4" />, label: "Line", key: "l" },
  { id: "pen", icon: <Pencil className="h-4 w-4" />, label: "Pen", key: "p" },
  { id: "highlight", icon: <Highlighter className="h-4 w-4" />, label: "Highlight", key: "h" },
  { id: "blur", icon: <Droplets className="h-4 w-4" />, label: "Blur (pixelate)", key: "b" },
  { id: "text", icon: <Type className="h-4 w-4" />, label: "Text", key: "t" },
  { id: "step", icon: <Hash className="h-4 w-4" />, label: "Numbered step", key: "n" },
];

const uid = () => Math.random().toString(36).slice(2, 10);
const clamp = (v: number, lo: number, hi: number) => Math.max(lo, Math.min(hi, v));

export function QuickCaptureView() {
  const canvas = useRef<HTMLCanvasElement>(null);
  const [img, setImg] = useState<HTMLImageElement | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [region, setRegion] = useState<Rect | null>(null);
  const [editing, setEditing] = useState(false);
  const [tool, setTool] = useState<DrawTool>("arrow");
  const [color, setColor] = useState(COLORS[0]);
  const [width, setWidth] = useState(4);
  const [items, setItems] = useState<Item[]>([]);
  const [redo, setRedo] = useState<Item[][]>([]);
  const [past, setPast] = useState<Item[][]>([]);
  const [text, setText] = useState<{ x: number; y: number; value: string } | null>(null);
  const [busy, setBusy] = useState(false);
  const [mouse, setMouse] = useState<{ x: number; y: number } | null>(null);
  const [view, setView] = useState({ w: window.innerWidth, h: window.innerHeight });
  const drag = useRef<Drag | null>(null);
  const [live, setLive] = useState<Item | null>(null);
  const bar = useRef<HTMLDivElement>(null);
  const [barSize, setBarSize] = useState({ w: 880, h: 44 });
  useLayoutEffect(() => {
    const b = bar.current;
    if (b && (b.offsetWidth !== barSize.w || b.offsetHeight !== barSize.h)) setBarSize({ w: b.offsetWidth, h: b.offsetHeight });
  });

  // Load the frozen desktop, then show the window (no flash of an empty overlay).
  useEffect(() => {
    let url = "";
    invoke<ArrayBuffer>("quick_capture_image")
      .then((buf) => {
        url = URL.createObjectURL(new Blob([buf], { type: "image/png" }));
        const i = new Image();
        i.onload = () => {
          setImg(i);
          const w = getCurrentWindow();
          void w.show().then(() => w.setFocus());
        };
        i.onerror = () => setError("The capture could not be read.");
        i.src = url;
      })
      .catch((e) => {
        setError(errorMessage(e));
        void getCurrentWindow().show();
      });
    const onResize = () => setView({ w: window.innerWidth, h: window.innerHeight });
    window.addEventListener("resize", onResize);
    return () => {
      window.removeEventListener("resize", onResize);
      if (url) URL.revokeObjectURL(url);
    };
  }, []);

  // Image ↔ screen: the image is fitted into the window (exact on a single screen).
  const fit = img ? Math.min(view.w / img.naturalWidth, view.h / img.naturalHeight) : 1;
  const ox = img ? (view.w - img.naturalWidth * fit) / 2 : 0;
  const oy = img ? (view.h - img.naturalHeight * fit) / 2 : 0;
  const toImg = useCallback(
    (cx: number, cy: number) => (img ? { x: clamp((cx - ox) / fit, 0, img.naturalWidth), y: clamp((cy - oy) / fit, 0, img.naturalHeight) } : { x: 0, y: 0 }),
    [img, fit, ox, oy],
  );
  const toScreen = (r: Rect) => ({ x: ox + r.x * fit, y: oy + r.y * fit, w: r.w * fit, h: r.h * fit });

  // ------------------------------------------------------------------ drawing
  useEffect(() => {
    const c = canvas.current;
    const ctx = c?.getContext("2d");
    if (!c || !ctx || !img) return;
    const dpr = window.devicePixelRatio || 1;
    c.width = Math.round(view.w * dpr);
    c.height = Math.round(view.h * dpr);
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.fillStyle = "#000";
    ctx.fillRect(0, 0, c.width, c.height);
    ctx.setTransform(dpr * fit, 0, 0, dpr * fit, dpr * ox, dpr * oy);
    ctx.imageSmoothingEnabled = fit < 1;
    ctx.drawImage(img, 0, 0);
    const W = img.naturalWidth;
    const H = img.naturalHeight;
    ctx.fillStyle = "rgba(0,0,0,0.5)";
    if (region && region.w > 0 && region.h > 0) {
      const r = region;
      ctx.save();
      ctx.beginPath();
      ctx.rect(r.x, r.y, r.w, r.h);
      ctx.clip();
      for (const it of items) drawItem(ctx, img, it);
      if (live) drawItem(ctx, img, live);
      ctx.restore();
      ctx.fillRect(0, 0, W, r.y);
      ctx.fillRect(0, r.y + r.h, W, H - r.y - r.h);
      ctx.fillRect(0, r.y, r.x, r.h);
      ctx.fillRect(r.x + r.w, r.y, W - r.x - r.w, r.h);
    } else {
      ctx.fillRect(0, 0, W, H);
    }
    // Screen-space decorations: border, handles, size label, magnifier.
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    if (region) {
      const s = toScreen(region);
      ctx.strokeStyle = "#6366f1";
      ctx.lineWidth = 1.5;
      ctx.strokeRect(s.x, s.y, s.w, s.h);
      if (editing && tool === "move") {
        ctx.fillStyle = "#fff";
        ctx.strokeStyle = "#6366f1";
        for (const [hx, hy] of handlePoints(s)) {
          ctx.beginPath();
          ctx.rect(hx - 4, hy - 4, 8, 8);
          ctx.fill();
          ctx.stroke();
        }
      }
      const label = `${Math.round(region.w)} × ${Math.round(region.h)}`;
      ctx.font = "600 12px Inter, Ubuntu, sans-serif";
      const lw = ctx.measureText(label).width + 12;
      const ly = s.y > 26 ? s.y - 24 : s.y + 4;
      ctx.fillStyle = "rgba(17,17,17,0.85)";
      ctx.fillRect(s.x, ly, lw, 20);
      ctx.fillStyle = "#fff";
      ctx.fillText(label, s.x + 6, ly + 14);
    }
    if (!editing && mouse) {
      // Magnifier: 15×15 image pixels at 8×, with the pixel under the pointer marked.
      const p = toImg(mouse.x, mouse.y);
      const z = 8;
      const n = 15;
      const size = n * z;
      let mx = mouse.x + 24;
      let my = mouse.y + 24;
      if (mx + size > view.w) mx = mouse.x - 24 - size;
      if (my + size + 22 > view.h) my = mouse.y - 24 - size - 22;
      ctx.save();
      ctx.imageSmoothingEnabled = false;
      ctx.fillStyle = "#000";
      ctx.fillRect(mx, my, size, size);
      ctx.drawImage(img, Math.floor(p.x) - 7, Math.floor(p.y) - 7, n, n, mx, my, size, size);
      ctx.strokeStyle = "#6366f1";
      ctx.lineWidth = 2;
      ctx.strokeRect(mx, my, size, size);
      ctx.strokeStyle = "rgba(255,255,255,0.9)";
      ctx.lineWidth = 1;
      ctx.strokeRect(mx + 7 * z, my + 7 * z, z, z);
      ctx.fillStyle = "rgba(17,17,17,0.85)";
      ctx.fillRect(mx, my + size, size, 22);
      ctx.fillStyle = "#fff";
      ctx.font = "11px Inter, Ubuntu, sans-serif";
      ctx.fillText(`${Math.floor(p.x)}, ${Math.floor(p.y)}`, mx + 6, my + size + 15);
      ctx.restore();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [img, region, items, live, editing, tool, mouse, view, fit, ox, oy]);

  // ------------------------------------------------------------------ actions
  const commit = (next: Item[]) => {
    setPast((p) => [...p.slice(-50), items]);
    setRedo([]);
    setItems(next);
  };
  const undo = () => {
    if (!past.length) return;
    setRedo((r) => [...r, items]);
    setItems(past[past.length - 1]);
    setPast(past.slice(0, -1));
  };
  const redoIt = () => {
    if (!redo.length) return;
    setPast((p) => [...p, items]);
    setItems(redo[redo.length - 1]);
    setRedo(redo.slice(0, -1));
  };
  const cancel = () => void api.quickCaptureCancel();
  const finish = async (action: "copy" | "save" | "edit") => {
    if (!img || !region || region.w < 2 || region.h < 2 || busy) return;
    setBusy(true);
    setError(null);
    try {
      const doc = { version: 1 as const, items, crop: region, finish: defaultFinish() };
      const png = renderExport(img, doc).toDataURL("image/png");
      await api.quickCaptureFinish(action, png, JSON.stringify(doc), action === "save");
    } catch (e) {
      setError(errorMessage(e));
      setBusy(false);
    }
  };
  const commitText = () => {
    if (text && text.value.trim()) commit([...items, { id: uid(), type: "text", color, width, x: text.x, y: text.y + TEXT_SIZE[width], text: text.value, size: TEXT_SIZE[width] }]);
    setText(null);
  };

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (text || (e.target as HTMLElement)?.tagName === "INPUT") return; // the text box handles its own keys
      const k = e.key.toLowerCase();
      if (k === "escape") return cancel();
      if (!editing) return;
      if (k === "enter" || ((e.ctrlKey || e.metaKey) && k === "c")) return void finish("copy");
      if ((e.ctrlKey || e.metaKey) && k === "s") {
        e.preventDefault();
        return void finish("save");
      }
      if ((e.ctrlKey || e.metaKey) && (k === "y" || (k === "z" && e.shiftKey))) return redoIt();
      if ((e.ctrlKey || e.metaKey) && k === "z") return undo();
      if (!e.ctrlKey && !e.altKey && !e.metaKey) {
        const t = TOOLS.find((x) => x.key === k);
        if (t) setTool(t.id);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  // ------------------------------------------------------------------ pointer
  const handleAt = (cx: number, cy: number): Handle | null => {
    if (!region || !editing || tool !== "move") return null;
    const names: Handle[] = ["nw", "n", "ne", "e", "se", "s", "sw", "w"];
    const pts = handlePoints(toScreen(region));
    const i = pts.findIndex(([hx, hy]) => Math.abs(cx - hx) <= 7 && Math.abs(cy - hy) <= 7);
    return i >= 0 ? names[i] : null;
  };
  const inside = (p: { x: number; y: number }) => !!region && p.x >= region.x && p.x <= region.x + region.w && p.y >= region.y && p.y <= region.y + region.h;

  const onDown = (e: React.MouseEvent) => {
    // Keep the focus where it is (the text box) — a click on the canvas must not steal it.
    e.preventDefault();
    if (!img || busy) return;
    if (e.button === 2) {
      if (!editing) cancel();
      return;
    }
    if (e.button !== 0 || text) return;
    const p = toImg(e.clientX, e.clientY);
    if (!editing) {
      drag.current = { kind: "select", x0: p.x, y0: p.y };
      setRegion({ x: p.x, y: p.y, w: 0, h: 0 });
      return;
    }
    const h = handleAt(e.clientX, e.clientY);
    if (h && region) {
      drag.current = { kind: "resize", h, r: region };
      return;
    }
    if (tool === "move") {
      if (inside(p) && region) drag.current = { kind: "move", x0: p.x, y0: p.y, r: region };
      else if (items.length === 0) {
        // Start over with a new area.
        setEditing(false);
        drag.current = { kind: "select", x0: p.x, y0: p.y };
        setRegion({ x: p.x, y: p.y, w: 0, h: 0 });
      }
      return;
    }
    if (tool === "text") {
      setText({ x: p.x, y: p.y, value: "" });
      return;
    }
    if (tool === "step") {
      commit([...items, { id: uid(), type: "step", color, width, x: p.x, y: p.y, n: nextStep(items) }]);
      return;
    }
    const item: Item = tool === "pen" ? { id: uid(), type: "pen", color, width, points: [[p.x, p.y]] } : { id: uid(), type: tool, color, width: tool === "highlight" ? 1 : width, x1: p.x, y1: p.y, x2: p.x, y2: p.y };
    drag.current = { kind: "draw", item };
    setLive(item);
  };

  const onMove = (e: React.MouseEvent) => {
    setMouse({ x: e.clientX, y: e.clientY });
    const d = drag.current;
    if (!d || !img) return;
    const p = toImg(e.clientX, e.clientY);
    const W = img.naturalWidth;
    const H = img.naturalHeight;
    if (d.kind === "select") setRegion(norm(d.x0, d.y0, p.x, p.y));
    else if (d.kind === "move") setRegion({ ...d.r, x: clamp(d.r.x + p.x - d.x0, 0, W - d.r.w), y: clamp(d.r.y + p.y - d.y0, 0, H - d.r.h) });
    else if (d.kind === "resize") {
      let { x, y, w, h } = d.r;
      let x2 = x + w;
      let y2 = y + h;
      if (d.h.includes("w")) x = p.x;
      if (d.h.includes("e")) x2 = p.x;
      if (d.h.includes("n")) y = p.y;
      if (d.h.includes("s")) y2 = p.y;
      setRegion(norm(x, y, x2, y2));
    } else if (d.kind === "draw") {
      const it = d.item;
      const next: Item = it.type === "pen" ? { ...it, points: [...it.points, [p.x, p.y]] } : it.type === "text" || it.type === "step" ? it : { ...it, x2: p.x, y2: p.y };
      d.item = next;
      setLive(next);
    }
  };

  const onUp = () => {
    const d = drag.current;
    drag.current = null;
    if (!d) return;
    if (d.kind === "select") {
      if (region && region.w >= 4 && region.h >= 4) setEditing(true);
      else setRegion(null);
    } else if (d.kind === "draw") {
      setLive(null);
      const it = d.item;
      const tiny = it.type === "pen" ? it.points.length < 2 : it.type !== "text" && it.type !== "step" && Math.abs(it.x2 - it.x1) < 3 && Math.abs(it.y2 - it.y1) < 3;
      if (!tiny) commit([...items, it]);
    }
  };

  const cursor = (() => {
    if (!editing) return "crosshair";
    if (mouse) {
      const h = handleAt(mouse.x, mouse.y);
      if (h) return h === "n" || h === "s" ? "ns-resize" : h === "e" || h === "w" ? "ew-resize" : h === "nw" || h === "se" ? "nwse-resize" : "nesw-resize";
      if (tool === "move") return inside(toImg(mouse.x, mouse.y)) ? "move" : items.length ? "default" : "crosshair";
    }
    return tool === "text" ? "text" : "crosshair";
  })();

  // Toolbar under the area, or above it, or inside it when neither fits.
  const s = region ? toScreen(region) : null;
  const barH = barSize.h;
  const barW = barSize.w;
  const barTop = s ? (s.y + s.h + barH + 12 <= view.h ? s.y + s.h + 8 : s.y - barH - 8 >= 0 ? s.y - barH - 8 : s.y + s.h - barH - 8) : 0;
  const barLeft = s ? clamp(s.x + s.w / 2 - barW / 2, 8, view.w - barW - 8) : 0;

  return (
    <div className="fixed inset-0 select-none overflow-hidden" style={{ cursor }} onContextMenu={(e) => e.preventDefault()}>
      <canvas ref={canvas} className="absolute inset-0" style={{ width: view.w, height: view.h }} onMouseDown={onDown} onMouseMove={onMove} onMouseUp={onUp} onMouseLeave={() => setMouse(null)} />
      {img && !region && (
        <div className="pointer-events-none absolute left-1/2 top-6 -translate-x-1/2 rounded-full bg-[#18181b]/90 px-4 py-2 text-[13px] text-white shadow-lg">
          Drag to choose an area · Esc to cancel
        </div>
      )}
      {!img && !error && <div className="absolute inset-0 flex items-center justify-center text-[13px] text-white/70">Loading the capture…</div>}
      {text && (
        <input
          ref={(el) => {
            if (el && document.activeElement !== el) setTimeout(() => el.focus(), 0);
          }}
          value={text.value}
          onChange={(e) => setText({ ...text, value: e.target.value })}
          onKeyDown={(e) => {
            e.stopPropagation();
            if (e.key === "Enter") commitText();
            if (e.key === "Escape") setText(null);
          }}
          onBlur={commitText}
          placeholder="Type, then Enter"
          className="absolute rounded border border-dashed border-white/70 bg-black/20 px-1 font-semibold outline-none placeholder:text-white/50"
          style={{ left: ox + text.x * fit, top: oy + text.y * fit, fontSize: TEXT_SIZE[width] * fit, color, minWidth: 160 }}
        />
      )}
      {editing && s && (
        <div
          ref={bar}
          className="absolute flex min-h-11 flex-wrap items-center gap-0.5 rounded-xl border border-white/15 bg-[#18181b]/95 px-1.5 py-1 text-white shadow-2xl"
          style={{ top: barTop, left: barLeft, maxWidth: view.w - 16 }}
          onMouseDown={(e) => e.stopPropagation()}
        >
          {TOOLS.map((t) => (
            <Btn key={t.id} active={tool === t.id} title={`${t.label} (${t.key.toUpperCase()})`} onClick={() => setTool(t.id)}>
              {t.icon}
            </Btn>
          ))}
          <Sep />
          {COLORS.map((c) => (
            <button key={c} title={c} onClick={() => setColor(c)} className="mx-0.5 h-5 w-5 shrink-0 rounded-full border border-white/30" style={{ background: c, boxShadow: color === c ? `0 0 0 2px #18181b, 0 0 0 4px ${c}` : undefined }} />
          ))}
          <Sep />
          {WIDTHS.map((w) => (
            <Btn key={w} active={width === w} title={w === 2 ? "Thin" : w === 4 ? "Medium" : "Thick"} onClick={() => setWidth(w)}>
              <span className="rounded-full bg-white" style={{ width: w + 3, height: w + 3 }} />
            </Btn>
          ))}
          <Sep />
          <Btn title="Undo (Ctrl+Z)" disabled={!past.length} onClick={undo}>
            <Undo2 className="h-4 w-4" />
          </Btn>
          <Btn title="Redo (Ctrl+Y)" disabled={!redo.length} onClick={redoIt}>
            <Redo2 className="h-4 w-4" />
          </Btn>
          <Sep />
          <Btn title="Open in the Screenshot Studio (saves it to the library)" disabled={busy} onClick={() => void finish("edit")}>
            <PenLine className="h-4 w-4" />
          </Btn>
          <Btn title="Save to the library and copy (Ctrl+S)" disabled={busy} onClick={() => void finish("save")}>
            <Save className="h-4 w-4" />
          </Btn>
          <button
            title="Copy to the clipboard (Enter)"
            disabled={busy}
            onClick={() => void finish("copy")}
            className="ml-1 flex h-8 shrink-0 items-center gap-1.5 rounded-lg bg-indigo-500 px-3 text-[13px] font-medium text-white hover:bg-indigo-400 disabled:opacity-50"
          >
            {busy ? <Check className="h-4 w-4" /> : <Copy className="h-4 w-4" />}
            Copy
          </button>
          <Btn title="Cancel (Esc)" onClick={cancel}>
            <X className="h-4 w-4" />
          </Btn>
        </div>
      )}
      {error && (
        <div className="absolute left-1/2 top-6 flex -translate-x-1/2 items-center gap-3 rounded-lg bg-red-600/95 px-4 py-2 text-[13px] text-white shadow-lg">
          {error}
          <button className="rounded bg-white/20 px-2 py-0.5" onClick={cancel}>
            Close
          </button>
        </div>
      )}
    </div>
  );
}

function handlePoints(s: { x: number; y: number; w: number; h: number }): [number, number][] {
  const { x, y, w, h } = s;
  return [
    [x, y],
    [x + w / 2, y],
    [x + w, y],
    [x + w, y + h / 2],
    [x + w, y + h],
    [x + w / 2, y + h],
    [x, y + h],
    [x, y + h / 2],
  ];
}

function Btn({ active, title, disabled, onClick, children }: { active?: boolean; title: string; disabled?: boolean; onClick: () => void; children: ReactNode }) {
  return (
    <button
      title={title}
      disabled={disabled}
      onClick={onClick}
      className={`flex h-8 w-8 shrink-0 items-center justify-center rounded-lg transition-colors disabled:opacity-30 ${active ? "bg-indigo-500/30 text-indigo-200" : "text-white/80 hover:bg-white/10 hover:text-white"}`}
    >
      {children}
    </button>
  );
}

const Sep = () => <span className="mx-1 h-5 w-px shrink-0 bg-white/15" />;
