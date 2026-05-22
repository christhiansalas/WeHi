import { useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";

import { useWehiStore } from "../store";
import type { Preferences } from "../types";

/** Carga las preferencias al iniciar y las re-guarda en disco
 *  cuando cambian los campos relevantes (debounced 500 ms). */
export function usePersistedPreferences() {
  const sourceFolder = useWehiStore((s) => s.sourceFolder);
  const preset = useWehiStore((s) => s.preset);
  const weights = useWehiStore((s) => s.weights);
  const setSourceFolder = useWehiStore((s) => s.setSourceFolder);
  const setPreset = useWehiStore((s) => s.setPreset);
  const setWeights = useWehiStore((s) => s.setWeights);

  const loadedRef = useRef(false);

  // Carga inicial
  useEffect(() => {
    (async () => {
      try {
        const prefs = await invoke<Preferences>("read_preferences");
        if (prefs.last_source_folder) {
          setSourceFolder(prefs.last_source_folder);
        }
        if (prefs.weights) {
          setWeights(prefs.weights);
        }
        if (prefs.last_preset_name) {
          // Intenta cargar el preset guardado por nombre.
          try {
            const presets = await invoke<Array<Preset>>("list_presets");
            const found = presets.find((p) => p.name === prefs.last_preset_name);
            if (found) setPreset(found);
          } catch {
            /* ignora */
          }
        }
      } catch {
        /* sin preferencias previas, sigue */
      } finally {
        loadedRef.current = true;
      }
    })();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Guardado debounced
  useEffect(() => {
    if (!loadedRef.current) return;
    const t = window.setTimeout(() => {
      const prefs: Preferences = {
        last_source_folder: sourceFolder ?? undefined,
        last_output_folder: preset.output.folder || undefined,
        last_preset_name: preset.name || undefined,
        weights,
      };
      invoke("write_preferences", { preferences: prefs }).catch(() => {
        /* silencio: si falla, simplemente no persistirá */
      });
    }, 500);
    return () => window.clearTimeout(t);
  }, [sourceFolder, preset, weights]);
}

// Re-exporta Preset para el closure local.
import type { Preset } from "../types";
