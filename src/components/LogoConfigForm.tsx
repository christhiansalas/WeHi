import type { LogoBackground, LogoConfig } from "../types";
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

const DEFAULT_LOGO_CONFIG: Omit<LogoConfig, "logo_id"> = {
  position: { x: 0.95, y: 0.95 },
  scale_pct: 0.18,
  opacity: 0.9,
  background: null,
};

const DEFAULT_BG: LogoBackground = {
  color_rgba: [0, 0, 0, 180],
  padding_pct: 0.08,
};

/** Editor para una marca individual dentro de la lista. */
function LogoEditor({
  logo,
  index,
  total,
  onChange,
  onRemove,
  onMove,
}: {
  logo: LogoConfig;
  index: number;
  total: number;
  onChange: (next: LogoConfig) => void;
  onRemove: () => void;
  onMove: (delta: -1 | 1) => void;
}) {
  const logos = useWehiStore((s) => s.logos);

  function set(next: Partial<LogoConfig>) {
    onChange({ ...logo, ...next });
  }

  function setBg(next: Partial<LogoBackground> | null) {
    if (next === null) {
      set({ background: null });
    } else if (logo.background) {
      set({ background: { ...logo.background, ...next } });
    } else {
      set({ background: { ...DEFAULT_BG, ...next } });
    }
  }

  return (
    <div className="border border-neutral-200 rounded p-2 space-y-2 bg-white">
      <div className="flex items-center gap-1">
        <div className="text-[11px] font-medium text-neutral-700 flex-1">
          Marca {index + 1} de {total}
        </div>
        <button
          type="button"
          onClick={() => onMove(-1)}
          disabled={index === 0}
          className="text-[10px] px-1 py-0.5 border rounded disabled:opacity-30"
          title="Subir"
        >
          ↑
        </button>
        <button
          type="button"
          onClick={() => onMove(1)}
          disabled={index === total - 1}
          className="text-[10px] px-1 py-0.5 border rounded disabled:opacity-30"
          title="Bajar"
        >
          ↓
        </button>
        <button
          type="button"
          onClick={onRemove}
          className="text-[10px] px-1 py-0.5 text-red-600 hover:bg-red-50 rounded"
          title="Quitar marca"
        >
          ✕
        </button>
      </div>

      <label className="text-[10px] text-neutral-600 block">
        Logo
        <select
          value={logo.logo_id}
          onChange={(e) => set({ logo_id: e.target.value })}
          className="mt-0.5 w-full border border-neutral-300 rounded px-2 py-1 text-xs bg-white"
        >
          {logos.length === 0 && <option value="">— sin logos —</option>}
          {logos.map((l) => (
            <option key={l.id} value={l.id}>
              {l.name}
            </option>
          ))}
        </select>
      </label>

      <div>
        <div className="text-[10px] text-neutral-600 mb-1">Posición</div>
        <div className="grid grid-cols-3 gap-1 max-w-[120px]">
          {QUICK_POSITIONS.map((q, i) => {
            const activa =
              Math.abs(logo.position.x - q.x) < 0.01 &&
              Math.abs(logo.position.y - q.y) < 0.01;
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
        Tamaño: {(logo.scale_pct * 100).toFixed(0)}% del ancho
        <input
          type="range"
          min={2}
          max={60}
          step={1}
          value={Math.round(logo.scale_pct * 100)}
          onChange={(e) => set({ scale_pct: Number(e.target.value) / 100 })}
          className="w-full"
        />
      </label>

      <label className="text-[10px] text-neutral-600 block">
        Opacidad: {(logo.opacity * 100).toFixed(0)}%
        <input
          type="range"
          min={0}
          max={100}
          step={1}
          value={Math.round(logo.opacity * 100)}
          onChange={(e) => set({ opacity: Number(e.target.value) / 100 })}
          className="w-full"
        />
      </label>

      <div className="border-t border-neutral-100 pt-1 space-y-1">
        <label className="text-[10px] text-neutral-700 flex items-center gap-2">
          <input
            type="checkbox"
            checked={!!logo.background}
            onChange={(e) => (e.target.checked ? setBg({}) : setBg(null))}
          />
          Fondo detrás del logo
        </label>
        {logo.background && (
          <>
            <label className="text-[10px] text-neutral-600 flex items-center gap-2">
              Color
              <input
                type="color"
                value={rgbToHex(logo.background.color_rgba)}
                onChange={(e) => {
                  const [r, g, b] = hexToRgb(e.target.value);
                  setBg({
                    color_rgba: [r, g, b, logo.background!.color_rgba[3]],
                  });
                }}
                className="w-7 h-5 border border-neutral-300 rounded"
              />
            </label>
            <label className="text-[10px] text-neutral-600 block">
              Opacidad fondo:{" "}
              {Math.round((logo.background.color_rgba[3] / 255) * 100)}%
              <input
                type="range"
                min={0}
                max={255}
                step={1}
                value={logo.background.color_rgba[3]}
                onChange={(e) => {
                  const [r, g, b] = logo.background!.color_rgba;
                  setBg({ color_rgba: [r, g, b, Number(e.target.value)] });
                }}
                className="w-full"
              />
            </label>
            <label className="text-[10px] text-neutral-600 block">
              Margen: {(logo.background.padding_pct * 100).toFixed(0)}%
              <input
                type="range"
                min={0}
                max={50}
                step={1}
                value={Math.round(logo.background.padding_pct * 100)}
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

export function LogoConfigForm() {
  const logos = useWehiStore((s) => s.logos);
  const preset = useWehiStore((s) => s.preset);
  const patchPreset = useWehiStore((s) => s.patchPreset);

  // Lista unificada (incluye el legacy `logo` si existe).
  const list: LogoConfig[] = [
    ...(preset.logo ? [preset.logo] : []),
    ...preset.logos,
  ];

  function commit(next: LogoConfig[]) {
    // Migramos cualquier `logo` legacy a `logos`: si modificamos la
    // lista al menos una vez, dejamos el legacy slot en null y todo
    // queda dentro de `logos`.
    patchPreset({ logo: null, logos: next });
  }

  function addLogo() {
    const firstId = logos[0]?.id ?? "";
    commit([...list, { logo_id: firstId, ...DEFAULT_LOGO_CONFIG }]);
  }

  function removeAt(i: number) {
    commit(list.filter((_, idx) => idx !== i));
  }

  function updateAt(i: number, next: LogoConfig) {
    commit(list.map((l, idx) => (idx === i ? next : l)));
  }

  function moveAt(i: number, delta: -1 | 1) {
    const j = i + delta;
    if (j < 0 || j >= list.length) return;
    const copy = [...list];
    [copy[i], copy[j]] = [copy[j], copy[i]];
    commit(copy);
  }

  return (
    <section className="space-y-2">
      <div className="flex items-center justify-between">
        <h3 className="text-xs font-semibold text-neutral-700">
          Marcas de agua
        </h3>
        <button
          type="button"
          onClick={addLogo}
          disabled={logos.length === 0}
          className="text-[11px] px-2 py-1 bg-neutral-900 text-white rounded hover:bg-neutral-700 disabled:opacity-50"
          title={logos.length === 0 ? "Importa un logo primero" : ""}
        >
          + Añadir marca
        </button>
      </div>

      {list.length === 0 ? (
        <div className="text-[11px] text-neutral-500 italic">
          Sin marcas. Pulsa "+ Añadir marca" para agregar la primera.
        </div>
      ) : (
        <div className="space-y-2">
          {list.map((logo, i) => (
            <LogoEditor
              key={i}
              logo={logo}
              index={i}
              total={list.length}
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
