import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import type { Preset } from "../types";
import { useWehiStore } from "../store";
import { AdjustmentsForm } from "./AdjustmentsForm";
import { LogoManager } from "./LogoManager";
import { ResizeConfigForm } from "./ResizeConfigForm";
import { LogoConfigForm } from "./LogoConfigForm";
import { OutputConfigForm } from "./OutputConfigForm";
import { TextWatermarkForm } from "./TextWatermarkForm";

export function PresetEditor() {
  const preset = useWehiStore((s) => s.preset);
  const setPreset = useWehiStore((s) => s.setPreset);
  const patchPreset = useWehiStore((s) => s.patchPreset);

  const [presets, setPresets] = useState<Preset[]>([]);
  const [error, setError] = useState<string | null>(null);

  async function refreshPresets() {
    try {
      const list = await invoke<Preset[]>("list_presets");
      setPresets(list);
    } catch (e) {
      setError(String(e));
    }
  }

  useEffect(() => {
    void refreshPresets();
  }, []);

  async function savePreset() {
    setError(null);
    try {
      await invoke("save_preset", { preset });
      await refreshPresets();
    } catch (e) {
      setError(String(e));
    }
  }

  async function deletePreset() {
    setError(null);
    try {
      await invoke("delete_preset", { name: preset.name });
      await refreshPresets();
    } catch (e) {
      setError(String(e));
    }
  }

  function loadPreset(name: string) {
    const p = presets.find((x) => x.name === name);
    if (p) setPreset(p);
  }

  return (
    <div className="h-full flex flex-col">
      <div className="px-3 py-2 border-b border-neutral-200 bg-neutral-50 space-y-2">
        <div className="flex items-center gap-1">
          <input
            value={preset.name}
            onChange={(e) => patchPreset({ name: e.target.value })}
            placeholder="Nombre del preset"
            className="flex-1 border border-neutral-300 rounded px-2 py-1 text-xs bg-white"
          />
          <button
            type="button"
            onClick={savePreset}
            className="text-[11px] px-2 py-1 bg-neutral-900 text-white rounded hover:bg-neutral-700"
          >
            Guardar
          </button>
          <button
            type="button"
            onClick={deletePreset}
            className="text-[11px] px-2 py-1 bg-white border border-neutral-300 rounded hover:bg-neutral-100"
          >
            Eliminar
          </button>
        </div>
        <div>
          <select
            value=""
            onChange={(e) => {
              if (e.target.value) loadPreset(e.target.value);
            }}
            className="w-full text-[11px] border border-neutral-300 rounded px-2 py-1 bg-white"
          >
            <option value="">Cargar preset…</option>
            {presets.map((p) => (
              <option key={p.name} value={p.name}>
                {p.name}
              </option>
            ))}
          </select>
        </div>
        {error && (
          <div className="text-[11px] text-red-600 break-all">{error}</div>
        )}
      </div>
      <div className="flex-1 overflow-auto p-3 space-y-4">
        <LogoManager />
        <ResizeConfigForm />
        <AdjustmentsForm />
        <LogoConfigForm />
        <TextWatermarkForm />
        <OutputConfigForm />
      </div>
    </div>
  );
}
