import { useEffect, useRef } from "react";
import type { WorkspaceAction, WorkspaceState, LayoutNode } from "../../types/workspace";
import { tituloDoLink } from "../workspace/abrirLink";
import { rotuloDoNavegador } from "./NavegadorPane";

/**
 * Fork: links do app abrem numa aba de navegador do layout (`ABRIR_LINK`), nunca na janela.
 *
 * - Clique em `<a href="http…">` (conversa, markdown, qualquer tela) é interceptado aqui, antes de o
 *   webview navegar. O painel de onde veio o clique decide onde a aba nasce (à direita dele).
 * - O que escapar (navegação por script, `location = …`) é barrado no Rust (`navegador.rs`), que
 *   devolve o endereço pelo evento `abrir-link`.
 * - Webview de aba que deixou de existir (fechada) é destruído.
 */
function abasWeb(node: LayoutNode): string[] {
  if (node.type === "split") return [...abasWeb(node.first), ...abasWeb(node.second)];
  return node.tabs.filter((tab) => tab.kind === "web").map((tab) => tab.id);
}

export function useLinksEmAba(workspaces: WorkspaceState[], dispatch: (action: WorkspaceAction) => void, aoAbrir: () => void) {
  const aoAbrirRef = useRef(aoAbrir);
  aoAbrirRef.current = aoAbrir;

  useEffect(() => {
    const abrir = (url: string, paneId?: string) => {
      dispatch({ type: "ABRIR_LINK", url, title: tituloDoLink(url), paneId });
      aoAbrirRef.current();
    };
    const clique = (evento: MouseEvent) => {
      if (evento.defaultPrevented || evento.button > 1) return;
      const link = (evento.target as Element | null)?.closest?.("a[href]") as HTMLAnchorElement | null;
      if (!link) return;
      // Fork (Roadmaps): área que abre os próprios links (o estudo da etapa tem navegador dentro).
      if (link.closest("[data-links-proprios]")) return;
      const href = link.href;
      if (!/^https?:\/\//i.test(href)) return;
      // Endereço do próprio app (dev em localhost) navega normalmente.
      try { if (new URL(href).origin === window.location.origin) return; } catch { return; }
      evento.preventDefault();
      evento.stopPropagation();
      const painel = link.closest<HTMLElement>("[data-drop-pane]")?.dataset.dropPane;
      abrir(href, painel);
    };
    document.addEventListener("click", clique, true);
    document.addEventListener("auxclick", clique, true);

    let parar: (() => void) | undefined;
    let vivo = true;
    if ("__TAURI_INTERNALS__" in window) {
      void import("@tauri-apps/api/event").then(({ listen }) => listen<string>("abrir-link", (e) => abrir(e.payload)))
        .then((desligar) => { if (vivo) parar = desligar; else desligar(); }).catch(() => undefined);
    }
    return () => {
      vivo = false;
      document.removeEventListener("click", clique, true);
      document.removeEventListener("auxclick", clique, true);
      parar?.();
    };
  }, [dispatch]);

  // Aba de navegador fechada (em qualquer workspace/projeto): o webview nativo vai junto.
  const vivas = useRef(new Set<string>());
  useEffect(() => {
    const agora = new Set(workspaces.flatMap((w) => w.projects.flatMap((p) => abasWeb(p.layout))));
    if (!("__TAURI_INTERNALS__" in window)) { vivas.current = agora; return; }
    for (const id of vivas.current) {
      if (!agora.has(id)) {
        void import("@tauri-apps/api/core").then(({ invoke }) => invoke("navegador_fechar", { rotulo: rotuloDoNavegador(id) })).catch(() => undefined);
      }
    }
    vivas.current = agora;
  }, [workspaces]);
}
