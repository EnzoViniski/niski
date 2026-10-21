// Fork (Roadmaps): a fonte aberta dentro do estudo da etapa, no mesmo navegador nativo da aba
// "Navegador" (`features/navegador/NavegadorPane.tsx`). Um webview por fonte: trocar de fonte ou sair
// do estudo fecha o anterior, e nada fica aberto com a tela fechada.
import { useEffect } from "react";
import { NavegadorPane, rotuloDoNavegador } from "../navegador/NavegadorPane";
import type { WorkspaceTab } from "../../types/workspace";

/** Hash curto e estável do endereço (vira parte do rótulo do webview: só `[a-z0-9]`). */
export function hashDoEndereco(url: string): string {
  let h = 2166136261;
  for (let i = 0; i < url.length; i++) { h ^= url.charCodeAt(i); h = Math.imul(h, 16777619); }
  return (h >>> 0).toString(36);
}

export function idDaAbaDoEstudo(roadmapId: string, url: string) {
  return `roadmap-${roadmapId}-${hashDoEndereco(url)}`.slice(0, 70);
}

/**
 * `oculto`: algo precisa ficar por cima (o webview nativo cobre o HTML): desmonta e o webview some,
 * sem perder a página (volta igual).
 */
export function NavegadorDoEstudo({ roadmapId, url, oculto = false, onTitulo }: { roadmapId: string; url: string; oculto?: boolean; onTitulo?: (t: string) => void }) {
  const tabId = idDaAbaDoEstudo(roadmapId, url);
  // Fecha de verdade ao trocar de fonte ou sair do estudo (o NavegadorPane só esconde).
  useEffect(() => () => {
    if (!("__TAURI_INTERNALS__" in window)) return;
    void import("@tauri-apps/api/core").then(({ invoke }) => invoke("navegador_fechar", { rotulo: rotuloDoNavegador(tabId) })).catch(() => undefined);
  }, [tabId]);
  if (oculto) return <div className="flex h-full items-center justify-center text-xs text-text-muted">A página volta quando você fechar o painel de cima.</div>;
  const tab: WorkspaceTab = { id: tabId, kind: "web", title: url, resourceId: url };
  return <NavegadorPane key={tabId} tab={tab} onTitulo={onTitulo} />;
}
