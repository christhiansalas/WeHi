import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import { useWehiStore } from "../store";
import type { LogoConfig, PreviewResult } from "../types";
import { allLogos } from "../types";

function clamp01(v: number): number {
  return Math.max(0, Math.min(1, v));
}

function mimeForFormat(format: PreviewResult["format"]): string {
  switch (format) {
    case "Png":
      return "image/png";
    case "WebP":
      return "image/webp";
    case "Jpeg":
    default:
      return "image/jpeg";
  }
}

export function PreviewPanel() {
  const selectedPath = useWehiStore((s) => s.selectedPath);
  const preset = useWehiStore((s) => s.preset);
  const patchPreset = useWehiStore((s) => s.patchPreset);

  const [preview, setPreview] = useState<PreviewResult | null>(null);
  const [original, setOriginal] = useState<string | null>(null);
  const [logoUrls, setLogoUrls] = useState<Record<string, string>>({});
  const [showOriginal, setShowOriginal] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Índice de la marca actualmente "seleccionada" en la previa para
  // arrastrar. El usuario clickea sobre una marca para activarla.
  const [activeLogoIdx, setActiveLogoIdx] = useState<number>(0);

  // Estado local de arrastre: posición normalizada mientras el usuario
  // mueve la marca. NO se escribe al store hasta soltar.
  const [dragPos, setDragPos] = useState<{ x: number; y: number } | null>(null);
  const draggingRef = useRef(false);
  const imageBoxRef = useRef<HTMLDivElement | null>(null);

  const logos: LogoConfig[] = allLogos(preset);

  // Carga (en paralelo) los data URLs de TODAS las marcas del preset.
  useEffect(() => {
    let cancelled = false;
    const ids = Array.from(new Set(logos.map((l) => l.logo_id))).filter(Boolean);
    if (ids.length === 0) {
      setLogoUrls({});
      return;
    }
    Promise.all(
      ids.map((id) =>
        invoke<string>("get_logo_data_url", { id })
          .then((url) => [id, url] as const)
          .catch(() => [id, ""] as const),
      ),
    ).then((pairs) => {
      if (cancelled) return;
      const next: Record<string, string> = {};
      for (const [id, url] of pairs) if (url) next[id] = url;
      setLogoUrls(next);
    });
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [logos.map((l) => l.logo_id).join("|")]);

  // Carga el original cuando cambia el archivo seleccionado.
  useEffect(() => {
    setOriginal(null);
    if (!selectedPath) return;
    let cancelled = false;
    invoke<string>("get_file_data_url", { file: selectedPath })
      .then((url) => {
        if (!cancelled) setOriginal(url);
      })
      .catch(() => {
        if (!cancelled) setOriginal(null);
      });
    return () => {
      cancelled = true;
    };
  }, [selectedPath]);

  // Llama a process_preview con debounce de 300 ms.
  useEffect(() => {
    if (!selectedPath) {
      setPreview(null);
      return;
    }
    const timer = window.setTimeout(async () => {
      setLoading(true);
      setError(null);
      try {
        const result = await invoke<PreviewResult>("process_preview", {
          file: selectedPath,
          preset,
        });
        setPreview(result);
      } catch (e) {
        setError(String(e));
      } finally {
        setLoading(false);
      }
    }, 300);
    return () => window.clearTimeout(timer);
  }, [selectedPath, preset]);

  function updateDragFromEvent(e: React.PointerEvent<HTMLDivElement>) {
    const rect = imageBoxRef.current?.getBoundingClientRect();
    if (!rect) return;
    const x = clamp01((e.clientX - rect.left) / rect.width);
    const y = clamp01((e.clientY - rect.top) / rect.height);
    setDragPos({ x, y });
  }

  function onPointerDown(e: React.PointerEvent<HTMLDivElement>) {
    if (logos.length === 0) return;
    e.preventDefault();
    e.currentTarget.setPointerCapture(e.pointerId);
    draggingRef.current = true;
    updateDragFromEvent(e);
  }
  function onPointerMove(e: React.PointerEvent<HTMLDivElement>) {
    if (!draggingRef.current) return;
    updateDragFromEvent(e);
  }
  function onPointerUp(e: React.PointerEvent<HTMLDivElement>) {
    if (!draggingRef.current) return;
    try {
      e.currentTarget.releasePointerCapture(e.pointerId);
    } catch {
      /* noop */
    }
    draggingRef.current = false;
    if (dragPos && logos.length > 0) {
      // Actualiza solo la marca activa. Si el legacy `logo` está set
      // y activeLogoIdx === 0, actualiza ese; si no, actualiza
      // `logos[activeLogoIdx - offset]`.
      const newPosition = { x: dragPos.x, y: dragPos.y };
      const hasLegacy = preset.logo !== null;
      if (hasLegacy && activeLogoIdx === 0) {
        patchPreset({
          logo: { ...preset.logo!, position: newPosition },
        });
      } else {
        const listIdx = hasLegacy ? activeLogoIdx - 1 : activeLogoIdx;
        const updated = preset.logos.map((l, i) =>
          i === listIdx ? { ...l, position: newPosition } : l,
        );
        patchPreset({ logos: updated });
      }
    }
    setDragPos(null);
  }

  if (!selectedPath) {
    return (
      <div className="h-full flex items-center justify-center text-sm text-neutral-400 p-6 text-center">
        Selecciona un archivo de la cola para ver la previsualización.
      </div>
    );
  }

  const processedSrc = preview
    ? `data:${mimeForFormat(preview.format)};base64,${preview.base64}`
    : null;
  const displayedSrc = showOriginal ? original : processedSrc ?? original;

  const draggingNow = draggingRef.current;
  // Capa superpuesta: si está arrastrando, mostramos la marca activa
  // siguiendo el cursor. Si está viendo el original, mostramos TODAS
  // las marcas en sus posiciones del preset (para que el usuario
  // sepa dónde quedan).
  const showOverlay = logos.length > 0 && (draggingNow || showOriginal);

  return (
    <div className="h-full flex flex-col">
      <div className="px-3 py-2 border-b border-neutral-200 flex items-center justify-between bg-white">
        <div className="flex gap-1 text-xs">
          <button
            type="button"
            onClick={() => setShowOriginal(true)}
            className={`px-2 py-1 rounded border ${
              showOriginal
                ? "bg-neutral-900 text-white border-neutral-900"
                : "bg-white text-neutral-700 border-neutral-300"
            }`}
          >
            Original
          </button>
          <button
            type="button"
            onClick={() => setShowOriginal(false)}
            className={`px-2 py-1 rounded border ${
              !showOriginal
                ? "bg-neutral-900 text-white border-neutral-900"
                : "bg-white text-neutral-700 border-neutral-300"
            }`}
          >
            Procesada
          </button>
        </div>
        <div className="flex items-center gap-3">
          {logos.length > 1 && (
            <label className="text-[11px] text-neutral-600 flex items-center gap-1">
              Mover:
              <select
                value={activeLogoIdx}
                onChange={(e) => setActiveLogoIdx(Number(e.target.value))}
                className="border border-neutral-300 rounded px-1 py-0.5 text-[11px] bg-white"
              >
                {logos.map((_, i) => (
                  <option key={i} value={i}>
                    Marca {i + 1}
                  </option>
                ))}
              </select>
            </label>
          )}
          <div className="text-[11px] text-neutral-500">
            {preview ? `${preview.width} × ${preview.height} px` : "—"}
          </div>
        </div>
      </div>

      <div className="flex-1 flex items-center justify-center p-4 overflow-hidden bg-neutral-100">
        {!displayedSrc ? (
          <div className="text-xs text-neutral-400">
            {loading ? "Procesando…" : "Cargando…"}
          </div>
        ) : (
          <div
            ref={imageBoxRef}
            className="relative inline-block max-w-full max-h-full select-none"
            onPointerDown={onPointerDown}
            onPointerMove={onPointerMove}
            onPointerUp={onPointerUp}
            onPointerCancel={onPointerUp}
            style={{
              cursor: logos.length > 0 ? "crosshair" : "default",
              touchAction: "none",
            }}
          >
            <img
              src={displayedSrc}
              alt="preview"
              draggable={false}
              className="block max-w-full max-h-[calc(100vh-220px)]"
            />
            {showOverlay &&
              logos.map((logo, i) => {
                const isActive = i === activeLogoIdx;
                const pos =
                  isActive && dragPos ? dragPos : logo.position;
                const url = logoUrls[logo.logo_id];
                if (!url) return null;
                return (
                  <div
                    key={i}
                    style={{
                      position: "absolute",
                      left: `${pos.x * 100}%`,
                      top: `${pos.y * 100}%`,
                      transform: "translate(-50%, -50%)",
                      width: `${logo.scale_pct * 100}%`,
                      opacity: logo.opacity,
                      pointerEvents: "none",
                      outline:
                        logos.length > 1 && isActive
                          ? "1px dashed rgba(0,0,0,0.5)"
                          : "none",
                    }}
                  >
                    <img
                      src={url}
                      alt=""
                      draggable={false}
                      style={{ width: "100%", display: "block" }}
                    />
                  </div>
                );
              })}
          </div>
        )}
      </div>

      <div className="px-3 py-2 border-t border-neutral-100 text-[11px] text-neutral-500 flex items-center justify-between bg-white">
        <span>
          {loading
            ? "Procesando…"
            : draggingNow
              ? `Moviendo marca ${activeLogoIdx + 1}`
              : "Listo"}
        </span>
        {error && <span className="text-red-600 break-all ml-3">{error}</span>}
      </div>
    </div>
  );
}
