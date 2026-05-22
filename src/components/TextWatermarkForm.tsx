import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import type {
  FontStatus,
  LogoBackground,
  TextWatermark,
} from "../types";
import { useWehiStore } from "../store";

const QUICK_POSITIONS: { label: string; x: number; y: number }[] = [
  { label: "↖", x: 0.05, y: 0.05 },
  { label: "↑", x: 0.5, y: 0.05 },
  { label: "↗", x: 0.95, y: 0.05 },
  { label: "←", x: 0.05, y: 0.5 },
  { label: "•", x: 0.5, y: 0.5 },
  { label: "→", x: 0.95, y: 0.5 },
  { label: "↙", x: 0.05, y: 0.95 },
  { label: "↓", x: 0.5, y: 0.95 },
  { label: "↘", x: 0.95, y: 0.95 },
];

function rgbToHex([r, g, b]: [number, number, number, number]): string {
  const h = (n: number) => n.toString(16).padStart(2, "0");
  return `#${h(r)}${h(g)}${h(b)}`;
}
function hexToRgb(hex: string): [number, number, number] {
  return [
    parseInt(hex.slice(1, 3), 16),
    parseInt(hex.slice(3, 5), 16),
    parseInt(hex.slice(5, 7), 16),
  ];
}

const DEFAULT_TEXT: TextWatermark = {
  text: "© {filename}",
  font_size_pct: 0.04,
  color_rgba: [255, 255, 255, 255],
  position: { x: 0.5, y: 0.95 },
  opacity: 0.9,
  background: null,
};

const DEFAULT_BG: LogoBackground = {
  color_rgba: [0, 0, 0, 180],
  padding_pct: 0.2,
};

function TextEditor({
  cfg,
  index,
  total,
  onChange,
  onRemove,
  onMove,
}: {
  cfg: TextWatermark;
  index: number;
  total: number;
  onChange: (next: TextWatermark) => void;
  onRemove: () => void;
  onMove: (delta: -1 | 1) => void;
}) {
  function set(next: Partial<TextWatermark>) {
    onChange({ ...cfg, ...next });
  }

  function setBg(next: Partial<LogoBackground> | null) {
    if (next === null) {
      set({ background: null });
    } else if (cfg.background) {
      set({ background: { ...cfg.background, ...next } });
    } else {
      set({ background: { ...DEFAULT_BG, ...next } });
    }
  }

  return (
    <div className="border border-neutral-200 rounded p-2 space-y-2 bg-white">
      <div className="flex items-center gap-1">
        <div className="text-[11px] font-medium text-neutral-700 flex-1">
          Texto {index + 1} de {total}
        </div>
        <button
          type="button"
          onClick={() => onMove(-1)}
          disabled={index === 0}
          className="text-[10px] px-1 py-0.5 border rounded disabled:opacity-30"
        >
          ↑
        </button>
        <button
          type="button"
          onClick={() => onMove(1)}
          disabled={index === total - 1}
          className="text-[10px] px-1 py-0.5 border rounded disabled:opacity-30"
        >
          ↓
        </button>
        <button
          type="button"
          onClick={onRemove}
          className="text-[10px] px-1 py-0.5 text-red-600 hover:bg-red-50 rounded"
        >
          ✕
        </button>
      </div>

      <label className="text-[10px] text-neutral-600 block">
        Texto
        <input
          value={cfg.text}
          onChange={(e) => set({ text: e.target.value })}
          placeholder="© {filename} 2025"
          className="mt-0.5 w-full border border-neutral-300 rounded px-2 py-1 text-xs"
        />
        <span className="text-[9px] text-neutral-400">
          Tokens: <code className="bg-neutral-100 px-0.5">{"{filename}"}</code>{" "}
          <code className="bg-neutral-100 px-0.5">{"{date}"}</code>
        </span>
      </label>

      <div>
        <div className="text-[10px] text-neutral-600 mb-1">Posición</div>
        <div className="grid grid-cols-3 gap-1 max-w-[120px]">
          {QUICK_POSITIONS.map((q, i) => {
            const activa =
              Math.abs(cfg.position.x - q.x) < 0.01 &&
              Math.abs(cfg.position.y - q.y) < 0.01;
            return (
              <button
                key={i}
                type="button"
                onClick={() => set({ position: { x: q.x, y: q.y } })}
                className={`aspect-square text-[10px] rounded border ${
                  activa
                    ? "bg-neutral-900 text-white border-neutral-900"
                    : "bg-white text-neutral-600 border-neutral-300 hover:bg-neutral-100"
                }`}
              >
                {q.label}
              </button>
            );
          })}
        </div>
      </div>

      <label className="text-[10px] text-neutral-600 block">
        Tamaño: {(cfg.font_size_pct * 100).toFixed(1)}% del ancho
        <input
          type="range"
          min={0.5}
          max={20}
          step={0.1}
          value={Number((cfg.font_size_pct * 100).toFixed(1))}
          onChange={(e) =>
            set({ font_size_pct: Number(e.target.value) / 100 })
          }
          className="w-full"
        />
      </label>

      <div className="grid grid-cols-2 gap-2">
        <label className="text-[10px] text-neutral-600 flex items-center gap-1">
          Color
          <input
            type="color"
            value={rgbToHex(cfg.color_rgba)}
            onChange={(e) => {
              const [r, g, b] = hexToRgb(e.target.value);
              set({ color_rgba: [r, g, b, cfg.color_rgba[3]] });
            }}
            className="w-7 h-5 border border-neutral-300 rounded"
          />
        </label>
        <label className="text-[10px] text-neutral-600 block">
          Opacidad: {(cfg.opacity * 100).toFixed(0)}%
          <input
            type="range"
            min={0}
            max={100}
            step={1}
            value={Math.round(cfg.opacity * 100)}
            onChange={(e) => set({ opacity: Number(e.target.value) / 100 })}
            className="w-full"
          />
        </label>
      </div>

      <div className="border-t border-neutral-100 pt-1 space-y-1">
        <label className="text-[10px] text-neutral-700 flex items-center gap-2">
          <input
            type="checkbox"
            checked={!!cfg.background}
            onChange={(e) => (e.target.checked ? setBg({}) : setBg(null))}
          />
          Fondo detrás del texto
        </label>
        {cfg.background && (
          <>
            <label className="text-[10px] text-neutral-600 flex items-center gap-2">
              Color fondo
              <input
                type="color"
                value={rgbToHex(cfg.background.color_rgba)}
                onChange={(e) => {
                  const [r, g, b] = hexToRgb(e.target.value);
                  setBg({
                    color_rgba: [r, g, b, cfg.background!.color_rgba[3]],
                  });
                }}
                className="w-7 h-5 border border-neutral-300 rounded"
              />
            </label>
            <label className="text-[10px] text-neutral-600 block">
              Opacidad fondo:{" "}
              {Math.round((cfg.background.color_rgba[3] / 255) * 100)}%
              <input
                type="range"
                min={0}
                max={255}
                step={1}
                value={cfg.background.color_rgba[3]}
                onChange={(e) => {
                  const [r, g, b] = cfg.background!.color_rgba;
                  setBg({ color_rgba: [r, g, b, Number(e.target.value)] });
                }}
                className="w-full"
              />
            </label>
            <label className="text-[10px] text-neutral-600 block">
              Margen: {(cfg.background.padding_pct * 100).toFixed(0)}%
              <input
                type="range"
                min={0}
                max={100}
                step={5}
                value={Math.round(cfg.background.padding_pct * 100)}
                onChange={(e) =>
                  setBg({ padding_pct: Number(e.target.value) / 100 })
                }
                className="w-full"
              />
            </label>
          </>
        )}
      </div>
    </div>
  );
}

export function TextWatermarkForm() {
  const texts = useWehiStore((s) => s.preset.text_watermarks);
  const patchPreset = useWehiStore((s) => s.patchPreset);
  const [font, setFont] = useState<FontStatus | null>(null);

  useEffect(() => {
    invoke<FontStatus>("check_font")
      .then(setFont)
      .catch(() => setFont({ available: false, path: null }));
  }, []);

  function commit(next: TextWatermark[]) {
    patchPreset({ text_watermarks: next });
  }

  function add() {
    commit([...texts, { ...DEFAULT_TEXT }]);
  }
  function removeAt(i: number) {
    commit(texts.filter((_, idx) => idx !== i));
  }
  function updateAt(i: number, next: TextWatermark) {
    commit(texts.map((t, idx) => (idx === i ? next : t)));
  }
  function moveAt(i: number, delta: -1 | 1) {
    const j = i + delta;
    if (j < 0 || j >= texts.length) return;
    const copy = [...texts];
    [copy[i], copy[j]] = [copy[j], copy[i]];
    commit(copy);
  }

  return (
    <section className="space-y-2">
      <div className="flex items-center justify-between">
        <h3 className="text-xs font-semibold text-neutral-700">Texto</h3>
        <button
          type="button"
          onClick={add}
          disabled={font !== null && !font.available}
          className="text-[11px] px-2 py-1 bg-neutral-900 text-white rounded hover:bg-neutral-700 disabled:opacity-50"
          title={
            font && !font.available
              ? "No se encontró fuente del sistema"
              : ""
          }
        >
          + Añadir texto
        </button>
      </div>
      {font && !font.available && (
        <div className="text-[10px] text-amber-700 bg-amber-50 border border-amber-200 rounded p-2">
          No se encontró ninguna fuente TTF del sistema. Las marcas de
          agua por texto no funcionarán hasta instalar una (en macOS
          suele venir incluida).
        </div>
      )}
      {texts.length === 0 ? (
        <div className="text-[11px] text-neutral-500 italic">
          Sin texto. Pulsa "+ Añadir texto" para agregar firma, fecha o lo que necesites.
        </div>
      ) : (
        <div className="space-y-2">
          {texts.map((cfg, i) => (
            <TextEditor
              key={i}
              cfg={cfg}
              index={i}
              total={texts.length}
              onChange={(next) => updateAt(i, next)}
              onRemove={() => removeAt(i)}
              onMove={(d) => moveAt(i, d)}
            />
          ))}
        </div>
      )}
    </section>
  );
}
