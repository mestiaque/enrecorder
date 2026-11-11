/** Full-screen transparent overlay: drag to mark the area to record. */
import { useEffect, useRef, useState } from "react";
import { api } from "../lib/api";

type Rect = { x: number; y: number; w: number; h: number };

const params = new URLSearchParams(location.search);
const ORIGIN = { x: Number(params.get("ox") ?? 0), y: Number(params.get("oy") ?? 0) };

export function RegionView() {
  const [rect, setRect] = useState<Rect | null>(null);
  const [dragging, setDragging] = useState(false);
  const start = useRef<{ x: number; y: number } | null>(null);
  const cancel = () => void api.regionSelected(null);
  const confirm = (r: Rect | null = rect) => {
    if (!r || r.w < 16 || r.h < 16) return;
    // CSS pixels → physical desktop pixels.
    const s = window.devicePixelRatio || 1;
    void api.regionSelected({ type: "region", x: Math.round(ORIGIN.x + r.x * s), y: Math.round(ORIGIN.y + r.y * s), width: Math.round(r.w * s), height: Math.round(r.h * s) });
  };
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") cancel();
      if (e.key === "Enter") confirm();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });
  const s = window.devicePixelRatio || 1;
  return (
    <div
      className="fixed inset-0 cursor-crosshair select-none"
      style={{ background: rect ? "transparent" : "rgba(0,0,0,0.35)" }}
      onMouseDown={(e) => {
        if ((e.target as HTMLElement).closest("button")) return;
        start.current = { x: e.clientX, y: e.clientY };
        setRect({ x: e.clientX, y: e.clientY, w: 0, h: 0 });
        setDragging(true);
      }}
      onMouseMove={(e) => {
        if (!dragging || !start.current) return;
        const a = start.current;
        setRect({ x: Math.min(a.x, e.clientX), y: Math.min(a.y, e.clientY), w: Math.abs(e.clientX - a.x), h: Math.abs(e.clientY - a.y) });
      }}
      onMouseUp={() => setDragging(false)}
      onContextMenu={(e) => {
        e.preventDefault();
        cancel();
      }}
    >
      {!rect && (
        <div className="pointer-events-none absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2 rounded-xl bg-black/75 px-5 py-3 text-center text-[14px] text-white">
          Drag to mark the area to record
          <div className="mt-1 text-[12px] text-white/60">Enter confirms · Esc or right-click cancels</div>
        </div>
      )}
      {rect && (
        <>
          {/* Dim everything outside the selection with one huge shadow. */}
          <div className="pointer-events-none absolute border-2 border-[#6366f1]" style={{ left: rect.x, top: rect.y, width: rect.w, height: rect.h, boxShadow: "0 0 0 9999px rgba(0,0,0,0.45)" }} />
          <div className="pointer-events-none absolute rounded bg-black/80 px-1.5 py-0.5 font-mono text-[11px] text-white" style={{ left: rect.x, top: Math.max(0, rect.y - 22) }}>
            {Math.round(rect.w * s)} × {Math.round(rect.h * s)}
          </div>
          {!dragging && rect.w >= 16 && rect.h >= 16 && (
            <div className="absolute flex gap-2" style={{ left: rect.x, top: Math.min(window.innerHeight - 44, rect.y + rect.h + 8) }}>
              <button className="h-8 rounded-md bg-[#6366f1] px-3 text-[13px] font-medium text-white hover:brightness-110" onClick={() => confirm()}>
                Record this area
              </button>
              <button className="h-8 rounded-md bg-black/75 px-3 text-[13px] text-white hover:bg-black/90" onClick={() => setRect(null)}>
                Redraw
              </button>
              <button className="h-8 rounded-md bg-black/75 px-3 text-[13px] text-white hover:bg-black/90" onClick={cancel}>
                Cancel
              </button>
            </div>
          )}
        </>
      )}
    </div>
  );
}
