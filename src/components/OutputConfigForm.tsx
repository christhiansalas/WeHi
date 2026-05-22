import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";

import type {
  NetworkCrop,
  NetworkSpec,
  OutputConfig,
  OutputFormat,
} from "../types";
import { useWehiStore } from "../store";

const FORMATS: { value: OutputFormat; label: string }[] = [
  { value: "Jpeg", label: "JPEG" },
  { value: "Png", label: "PNG" },
  { value: "WebP", label: "WebP" },
];

function networkToCrop(n: NetworkSpec): NetworkCrop {
  return {
    id: n.id,
    label: n.name,
    width: n.suggested_width,
    height: n.suggested_height,
  };
}

export function OutputConfigForm() {
  const output = useWehiStore((s) => s.preset.output);
  const networkCrops = useWehiStore((s) => s.preset.network_crops);
  const patchPreset = useWehiStore((s) => s.patchPreset);
  const [networks, setNetworks] = useState<NetworkSpec[]>([]);

  useEffect(() => {
    invoke<NetworkSpec[]>("list_networks")
      .then(setNetworks)
      .catch(() => setNetworks([]));
  }, []);

  const set = (next: Partial<OutputConfig>) =>
    patchPreset({ output: { ...output, ...next } });

  function toggleCrop(net: NetworkSpec) {
    const exists = networkCrops.some((c) => c.id === net.id);
    const next = exists
      ? networkCrops.filter((c) => c.id !== net.id)
      : [...networkCrops, networkToCrop(net)];
    patchPreset({ network_crops: next });
  }

  async function pickFolder() {
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected === "string") set({ folder: selected });
  }

  return (
    <section className="space-y-3">
      <h3 className="text-xs font-semibold text-neutral-700">Salida</h3>

      <div>
        <label className="text-[11px] text-neutral-600 mb-1 block">
          Formato
        </label>
        <div className="flex gap-1">
          {FORMATS.map((f) => (
            <button
              key={f.value}
              type="button"
              onClick={() => set({ format: f.value })}
              className={`flex-1 text-[11px] py-1 rounded border ${
                output.format === f.value
                  ? "bg-neutral-900 text-white border-neutral-900"
                  : "bg-white text-neutral-700 border-neutral-300"
              }`}
            >
              {f.label}
            </button>
          ))}
        </div>
      </div>

      {output.format !== "Png" && (
        <label className="text-[11px] text-neutral-600 block">
          Calidad: {output.quality}
          <input
            type="range"
            min={1}
            max={100}
            step={1}
            value={output.quality}
            onChange={(e) => set({ quality: Number(e.target.value) })}
            className="w-full"
          />
        </label>
      )}

      <div className="space-y-1">
        <label className="text-[11px] text-neutral-600 block">
          Carpeta de salida
        </label>
        <div className="flex gap-1">
          <input
            value={output.folder}
            onChange={(e) => set({ folder: e.target.value })}
            placeholder="Sin definir"
            className="flex-1 border border-neutral-300 rounded px-2 py-1 text-xs"
          />
          <button
            type="button"
            onClick={pickFolder}
            className="text-[11px] px-2 py-1 bg-neutral-100 border border-neutral-300 rounded hover:bg-neutral-200"
          >
            …
          </button>
        </div>
      </div>

      <label className="text-[11px] text-neutral-600 block">
        Patrón de nombre
        <input
          value={output.filename_pattern}
          onChange={(e) => set({ filename_pattern: e.target.value })}
          placeholder="{name}{ext}"
          className="mt-0.5 w-full border border-neutral-300 rounded px-2 py-1 text-xs"
        />
        <span className="text-[10px] text-neutral-400">
          Soporta {"{name}"} y {"{ext}"}.
        </span>
      </label>

      <label className="flex items-center gap-2 text-[11px] text-neutral-700">
        <input
          type="checkbox"
          checked={output.preserve_structure}
          onChange={(e) => set({ preserve_structure: e.target.checked })}
        />
        Preservar estructura de carpetas
      </label>

      <div className="border-t border-neutral-200 pt-2">
        <div className="text-[11px] font-medium text-neutral-700 mb-1">
          Recortes adicionales por red
        </div>
        <div className="text-[10px] text-neutral-500 mb-2">
          Por cada red marcada se genera un archivo extra{" "}
          <span className="font-mono">nombre__id.ext</span> centrado al
          aspect ratio y reescalado.
        </div>
        <div className="space-y-1">
          {networks.length === 0 && (
            <div className="text-[10px] text-neutral-400 italic">
              Cargando redes…
            </div>
          )}
          {networks.map((n) => {
            const checked = networkCrops.some((c) => c.id === n.id);
            return (
              <label
                key={n.id}
                className="flex items-center gap-2 text-[11px] text-neutral-700"
              >
                <input
                  type="checkbox"
                  checked={checked}
                  onChange={() => toggleCrop(n)}
                />
                <span className="flex-1 truncate">{n.name}</span>
                <span className="text-[10px] text-neutral-400">
                  {n.suggested_width}×{n.suggested_height}
                </span>
              </label>
            );
          })}
        </div>
        {networkCrops.length > 0 && (
          <div className="mt-2 text-[10px] text-neutral-500">
            {networkCrops.length} recorte(s) por archivo procesado.
          </div>
        )}
      </div>
    </section>
  );
}
