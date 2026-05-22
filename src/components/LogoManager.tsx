import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

import type { LogoEntry } from "../types";
import { useWehiStore } from "../store";

function LogoThumb({ logo }: { logo: LogoEntry }) {
  const [src, setSrc] = useState<string | null>(null);
  useEffect(() => {
    let cancelled = false;
    invoke<string>("get_logo_data_url", { id: logo.id })
      .then((url) => {
        if (!cancelled) setSrc(url);
      })
      .catch(() => {
        if (!cancelled) setSrc(null);
      });
    return () => {
      cancelled = true;
    };
  }, [logo.id]);
  if (!src) {
    return (
      <div className="w-full h-full flex items-center justify-center text-[10px] text-neutral-400">
        cargando…
      </div>
    );
  }
  return (
    <img
      src={src}
      alt={logo.name}
      className="w-full h-full object-contain"
    />
  );
}

export function LogoManager() {
  const logos = useWehiStore((s) => s.logos);
  const setLogos = useWehiStore((s) => s.setLogos);
  const preset = useWehiStore((s) => s.preset);
  const patchPreset = useWehiStore((s) => s.patchPreset);
  const [error, setError] = useState<string | null>(null);

  async function refresh() {
    try {
      const list = await invoke<LogoEntry[]>("list_logos");
      setLogos(list);
    } catch (e) {
      setError(String(e));
    }
  }

  useEffect(() => {
    void refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  async function addLogo() {
    setError(null);
    try {
      const selected = await open({
        multiple: false,
        filters: [
          {
            name: "Imagen con transparencia",
            extensions: ["png", "webp", "jpg", "jpeg"],
          },
        ],
      });
      if (typeof selected !== "string") return;
      await invoke("add_logo", { sourcePath: selected });
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  }

  async function deleteLogo(id: string) {
    setError(null);
    try {
      await invoke("delete_logo", { id });
      if (preset.logo?.logo_id === id) {
        patchPreset({ logo: null });
      }
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  }

  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between">
        <div className="text-xs font-medium text-neutral-700">
          Biblioteca de logos
        </div>
        <button
          type="button"
          onClick={addLogo}
          className="text-xs px-2 py-1 bg-neutral-900 text-white rounded hover:bg-neutral-700"
        >
          + Importar
        </button>
      </div>
      {logos.length === 0 ? (
        <div className="text-xs text-neutral-500 italic">
          Aún no hay logos guardados.
        </div>
      ) : (
        <div className="grid grid-cols-3 gap-2">
          {logos.map((logo) => {
            const seleccionado = preset.logo?.logo_id === logo.id;
            return (
              <div
                key={logo.id}
                className={`relative aspect-square border rounded bg-[conic-gradient(at_50%_50%,#eee_25%,#fff_0_50%,#eee_0_75%,#fff_0)] bg-[length:12px_12px] ${
                  seleccionado ? "ring-2 ring-neutral-900" : "border-neutral-200"
                }`}
              >
                <LogoThumb logo={logo} />
                <button
                  type="button"
                  onClick={() => deleteLogo(logo.id)}
                  title="Eliminar"
                  className="absolute top-0.5 right-0.5 bg-white/90 text-red-600 text-[10px] w-4 h-4 rounded-sm leading-none"
                >
                  ×
                </button>
                <div className="absolute left-0 right-0 bottom-0 text-[9px] text-center bg-white/80 truncate px-1">
                  {logo.name}
                </div>
              </div>
            );
          })}
        </div>
      )}
      {error && <div className="text-xs text-red-600 break-all">{error}</div>}
    </div>
  );
}
