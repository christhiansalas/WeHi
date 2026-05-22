import type { ResizeConfig, ResizeMode } from "../types";
import { useWehiStore } from "../store";

const MODOS: { value: ResizeMode; label: string }[] = [
  { value: "MaxDimension", label: "Caber en caja" },
  { value: "Exact", label: "Exacto" },
  { value: "Percentage", label: "Porcentaje" },
];

export function ResizeConfigForm() {
  const resize = useWehiStore((s) => s.preset.resize);
  const patchPreset = useWehiStore((s) => s.patchPreset);

  const set = (next: Partial<ResizeConfig>) =>
    patchPreset({ resize: { ...resize, ...next } });

  return (
    <section className="space-y-3">
      <h3 className="text-xs font-semibold text-neutral-700">
        Redimensionado
      </h3>

      <div>
        <label className="text-[11px] text-neutral-600 mb-1 block">Modo</label>
        <div className="flex gap-1">
          {MODOS.map((m) => (
            <button
              key={m.value}
              type="button"
              onClick={() => set({ mode: m.value })}
              className={`flex-1 text-[11px] py-1 rounded border ${
                resize.mode === m.value
                  ? "bg-neutral-900 text-white border-neutral-900"
                  : "bg-white text-neutral-700 border-neutral-300"
              }`}
            >
              {m.label}
            </button>
          ))}
        </div>
      </div>

      {(resize.mode === "MaxDimension" || resize.mode === "Exact") && (
        <div className="grid grid-cols-2 gap-2">
          <label className="text-[11px] text-neutral-600">
            Ancho (px)
            <input
              type="number"
              min={1}
              value={resize.width ?? ""}
              onChange={(e) =>
                set({
                  width: e.target.value === "" ? null : Number(e.target.value),
                })
              }
              className="mt-0.5 w-full border border-neutral-300 rounded px-2 py-1 text-xs"
            />
          </label>
          <label className="text-[11px] text-neutral-600">
            Alto (px)
            <input
              type="number"
              min={1}
              value={resize.height ?? ""}
              onChange={(e) =>
                set({
                  height: e.target.value === "" ? null : Number(e.target.value),
                })
              }
              className="mt-0.5 w-full border border-neutral-300 rounded px-2 py-1 text-xs"
            />
          </label>
        </div>
      )}

      {resize.mode === "Percentage" && (
        <label className="text-[11px] text-neutral-600 block">
          Porcentaje: {((resize.percentage ?? 1) * 100).toFixed(0)}%
          <input
            type="range"
            min={5}
            max={400}
            step={1}
            value={Math.round((resize.percentage ?? 1) * 100)}
            onChange={(e) => set({ percentage: Number(e.target.value) / 100 })}
            className="w-full"
          />
        </label>
      )}

      <div className="space-y-1">
        <label className="flex items-center gap-2 text-[11px] text-neutral-700">
          <input
            type="checkbox"
            checked={resize.keep_aspect_ratio}
            onChange={(e) => set({ keep_aspect_ratio: e.target.checked })}
          />
          Mantener proporción
        </label>
        <label className="flex items-center gap-2 text-[11px] text-neutral-700">
          <input
            type="checkbox"
            checked={resize.allow_upscale}
            onChange={(e) => set({ allow_upscale: e.target.checked })}
          />
          Permitir agrandar (upscale)
        </label>
      </div>
    </section>
  );
}
