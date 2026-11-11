/** Annotation document and rendering for the Screenshot editor (pure; unit-tested). */

export type Tool = "select" | "arrow" | "rect" | "ellipse" | "line" | "highlight" | "blur" | "text" | "step" | "pen" | "crop";

interface Base {
  id: string;
  color: string;
  width: number;
}
export type Item =
  | (Base & { type: "arrow" | "rect" | "ellipse" | "line" | "highlight" | "blur"; x1: number; y1: number; x2: number; y2: number })
  | (Base & { type: "text"; x: number; y: number; text: string; size: number })
  | (Base & { type: "step"; x: number; y: number; n: number })
  | (Base & { type: "pen"; points: [number, number][] });

export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface Doc {
  version: 1;
  items: Item[];
  crop: Rect | null;
}

export const emptyDoc = (): Doc => ({ version: 1, items: [], crop: null });

export function parseDoc(json: string | null | undefined): Doc {
  if (!json) return emptyDoc();
  try {
    const d = JSON.parse(json) as Partial<Doc>;
    return { version: 1, items: Array.isArray(d.items) ? d.items : [], crop: d.crop ?? null };
  } catch {
    return emptyDoc();
  }
}

export function norm(x1: number, y1: number, x2: number, y2: number): Rect {
  return { x: Math.min(x1, x2), y: Math.min(y1, y2), w: Math.abs(x2 - x1), h: Math.abs(y2 - y1) };
}

/** Bounding box of an item, for hit-testing and moving. */
export function bounds(it: Item): Rect {
  switch (it.type) {
    case "text": {
      const w = Math.max(20, it.text.length * it.size * 0.6);
      return { x: it.x, y: it.y - it.size, w, h: it.size * 1.3 };
    }
    case "step":
      return { x: it.x - stepRadius(it.width), y: it.y - stepRadius(it.width), w: stepRadius(it.width) * 2, h: stepRadius(it.width) * 2 };
    case "pen": {
      const xs = it.points.map((p) => p[0]);
      const ys = it.points.map((p) => p[1]);
      return norm(Math.min(...xs), Math.min(...ys), Math.max(...xs), Math.max(...ys));
    }
    default:
      return norm(it.x1, it.y1, it.x2, it.y2);
  }
}

export const stepRadius = (w: number) => 10 + w * 2.5;

export function hit(items: Item[], x: number, y: number, pad = 6): Item | null {
  for (let i = items.length - 1; i >= 0; i--) {
    const b = bounds(items[i]);
    if (x >= b.x - pad && x <= b.x + b.w + pad && y >= b.y - pad && y <= b.y + b.h + pad) return items[i];
  }
  return null;
}

export function move(it: Item, dx: number, dy: number): Item {
  switch (it.type) {
    case "text":
    case "step":
      return { ...it, x: it.x + dx, y: it.y + dy };
    case "pen":
      return { ...it, points: it.points.map(([x, y]) => [x + dx, y + dy] as [number, number]) };
    default:
      return { ...it, x1: it.x1 + dx, y1: it.y1 + dy, x2: it.x2 + dx, y2: it.y2 + dy };
  }
}

export const nextStep = (items: Item[]) => items.reduce((n, it) => (it.type === "step" ? Math.max(n, it.n) : n), 0) + 1;

function arrow(ctx: CanvasRenderingContext2D, x1: number, y1: number, x2: number, y2: number, w: number) {
  const head = Math.max(12, w * 4);
  const ang = Math.atan2(y2 - y1, x2 - x1);
  ctx.beginPath();
  ctx.moveTo(x1, y1);
  ctx.lineTo(x2 - Math.cos(ang) * head * 0.8, y2 - Math.sin(ang) * head * 0.8);
  ctx.stroke();
  ctx.beginPath();
  ctx.moveTo(x2, y2);
  ctx.lineTo(x2 - head * Math.cos(ang - Math.PI / 7), y2 - head * Math.sin(ang - Math.PI / 7));
  ctx.lineTo(x2 - head * Math.cos(ang + Math.PI / 7), y2 - head * Math.sin(ang + Math.PI / 7));
  ctx.closePath();
  ctx.fill();
}

/** Pixelates a region of the base image (not of earlier annotations). */
function pixelate(ctx: CanvasRenderingContext2D, img: CanvasImageSource, r: Rect, block: number) {
  if (r.w < 2 || r.h < 2) return;
  const sw = Math.max(1, Math.round(r.w / block));
  const sh = Math.max(1, Math.round(r.h / block));
  const tmp = document.createElement("canvas");
  tmp.width = sw;
  tmp.height = sh;
  const t = tmp.getContext("2d");
  if (!t) return;
  t.imageSmoothingEnabled = true;
  t.drawImage(img, r.x, r.y, r.w, r.h, 0, 0, sw, sh);
  ctx.save();
  ctx.imageSmoothingEnabled = false;
  ctx.drawImage(tmp, 0, 0, sw, sh, r.x, r.y, r.w, r.h);
  ctx.restore();
}

export function drawItem(ctx: CanvasRenderingContext2D, img: CanvasImageSource, it: Item) {
  ctx.save();
  ctx.strokeStyle = it.color;
  ctx.fillStyle = it.color;
  ctx.lineWidth = it.width;
  ctx.lineCap = "round";
  ctx.lineJoin = "round";
  switch (it.type) {
    case "arrow":
      arrow(ctx, it.x1, it.y1, it.x2, it.y2, it.width);
      break;
    case "line":
      ctx.beginPath();
      ctx.moveTo(it.x1, it.y1);
      ctx.lineTo(it.x2, it.y2);
      ctx.stroke();
      break;
    case "rect": {
      const r = norm(it.x1, it.y1, it.x2, it.y2);
      ctx.strokeRect(r.x, r.y, r.w, r.h);
      break;
    }
    case "ellipse": {
      const r = norm(it.x1, it.y1, it.x2, it.y2);
      ctx.beginPath();
      ctx.ellipse(r.x + r.w / 2, r.y + r.h / 2, r.w / 2, r.h / 2, 0, 0, Math.PI * 2);
      ctx.stroke();
      break;
    }
    case "highlight": {
      const r = norm(it.x1, it.y1, it.x2, it.y2);
      ctx.globalAlpha = 0.35;
      ctx.fillRect(r.x, r.y, r.w, r.h);
      break;
    }
    case "blur":
      pixelate(ctx, img, norm(it.x1, it.y1, it.x2, it.y2), Math.max(6, it.width * 3));
      break;
    case "text":
      ctx.font = `600 ${it.size}px Inter, "Segoe UI", Ubuntu, sans-serif`;
      ctx.textBaseline = "alphabetic";
      ctx.lineWidth = Math.max(2, it.size / 8);
      ctx.strokeStyle = "rgba(0,0,0,0.55)";
      ctx.strokeText(it.text, it.x, it.y);
      ctx.fillText(it.text, it.x, it.y);
      break;
    case "step": {
      const rad = stepRadius(it.width);
      ctx.beginPath();
      ctx.arc(it.x, it.y, rad, 0, Math.PI * 2);
      ctx.fill();
      ctx.fillStyle = "#fff";
      ctx.font = `700 ${Math.round(rad * 1.1)}px Inter, "Segoe UI", Ubuntu, sans-serif`;
      ctx.textAlign = "center";
      ctx.textBaseline = "middle";
      ctx.fillText(String(it.n), it.x, it.y + 1);
      break;
    }
    case "pen":
      ctx.beginPath();
      it.points.forEach(([x, y], i) => (i ? ctx.lineTo(x, y) : ctx.moveTo(x, y)));
      ctx.stroke();
      break;
  }
  ctx.restore();
}

/** The final image: base, annotations, then the crop. */
export function renderExport(img: HTMLImageElement, doc: Doc): HTMLCanvasElement {
  const full = document.createElement("canvas");
  full.width = img.naturalWidth;
  full.height = img.naturalHeight;
  const ctx = full.getContext("2d")!;
  ctx.drawImage(img, 0, 0);
  for (const it of doc.items) drawItem(ctx, img, it);
  const c = doc.crop && doc.crop.w > 4 && doc.crop.h > 4 ? doc.crop : null;
  if (!c) return full;
  const out = document.createElement("canvas");
  out.width = Math.round(c.w);
  out.height = Math.round(c.h);
  out.getContext("2d")!.drawImage(full, c.x, c.y, c.w, c.h, 0, 0, c.w, c.h);
  return out;
}
