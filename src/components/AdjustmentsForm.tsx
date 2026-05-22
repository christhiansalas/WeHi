import type { ImageAdjustments } from "../types";
import { defaultAdjustments } from "../types";
import { useWehiStore } from "../store";

interface SliderRowProps {
  label: string;
  hintLeft: string;
  hintRight: string;
  value: number;
  onChange: (v: number) => void;
}

function SliderRow({ label, hintLeft, hintRight, value, onChange }: SliderRowProps) {
  const display = value === 0 ? "0" : value > 0 ? `+${value.toFixed(2)}` : value.toFixed(2);
  return (
    <label className="text-[10px] text-neutral-600 block">
      <div className="flex items-center justify-between mb-0.5">
        <span>{label}</span>
        <span className="font-mono text-neutral-900">{display}</span>
      </div>
      <input
        type="range"
        min={-1}
        max={1}
        step={0.01}
        value={value}
        onChange={(e) => onChange(Number(e.target.value))}
        className="w-full"
      />
      <div className="flex items-center justify-between text-[9px] text-neutral-400">
        <span>{hintLeft}</span>
        <span>{hintRight}</span>
      </div>
    </label>
  );
}

export function AdjustmentsForm() {
  const adjustments = useWehiStore((s) => s.preset.adjustments);
  const patchPreset = useWehiStore((s) => s.patchPreset);

  function set(next: Partial<ImageAdjustments>) {
    patchPreset({ adjustments: { ...adjustments, ...next } });
  }

  const isIdentity =
    adjustments.brightness === 0 &&
    adjustments.contrast === 0 &&
    adjustments.saturation === 0 &&
    adjustments.temperature === 0;

  return (
    <section className="space-y-2">
      <div className="flex items-center justify-between">
        <h3 className="text-xs font-semibold text-neutral-700">Ajustes</h3>
        <button
          type="button"
          onClick={() => patchPreset({ adjustments: defaultAdjustments() })}
          disabled={isIdentity}
          className="text-[10px] px-2 py-0.5 text-neutral-600 hover:text-neutral-900 disabled:opacity-30"
        >
          Restablecer
        </button>
      </div>
      <SliderRow
        label="Exposición / Brillo"
        hintLeft="oscuro"
        hintRight="claro"
        value={adjustments.brightness}
        onChange={(v) => set({ brightness: v })}
      />
      <SliderRow
        label="Contraste"
        hintLeft="plano"
        hintRight="contrastado"
        value={adjustments.contrast}
        onChange={(v) => set({ contrast: v })}
      />
      <SliderRow
        label="Saturación"
        hintLeft="B&N"
        hintRight="saturado"
        value={adjustments.saturation}
        onChange={(v) => set({ saturation: v })}
      />
      <SliderRow
        label="Temperatura"
        hintLeft="frío (azul)"
        hintRight="cálido (rojo)"
        value={adjustments.temperature}
        onChange={(v) => set({ temperature: v })}
      />
    </section>
  );
}
