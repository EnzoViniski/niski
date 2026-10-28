import { useCallback, useEffect, useState } from "react";

export type Rota =
  | { tela: "inicio" }
  | { tela: "atividade" }
  | { tela: "uso" }
  | { tela: "sobre-mim" } // Fork (Sobre mim).
  | { tela: "roadmaps"; roadmap?: string } // Fork (Roadmaps de estudo).
  | { tela: "tarefas" } // Fork (Tarefas no celular).
  | { tela: "rotina" } // Fork (Rotina no celular).
  | { tela: "telegram" } // Fork (Telegram no celular).
  | { tela: "projeto"; projeto: string }
  | { tela: "conversa"; conversa: string };

/** `#/p/<projeto>` ou `#/c/<conversa>`. Qualquer outra coisa (inclusive o `#t=` do QR) é o início. */
export function lerRota(hash = location.hash): Rota {
  if (hash === "#/atividade") return { tela: "atividade" }; // Caixa compartilhada com o PC.
  if (hash === "#/uso") return { tela: "uso" }; // Uso das contas, como no rodapé do PC.
  if (hash === "#/sobre-mim") return { tela: "sobre-mim" }; // Fork (Sobre mim).
  if (hash === "#/tarefas") return { tela: "tarefas" }; // Fork (Tarefas no celular).
  if (hash === "#/rotina") return { tela: "rotina" }; // Fork (Rotina no celular).
  if (hash === "#/telegram") return { tela: "telegram" }; // Fork (Telegram no celular).
  // Fork (Roadmaps): `#/roadmaps` e `#/roadmaps/<id>`.
  if (hash === "#/roadmaps") return { tela: "roadmaps" };
  const [, roadmap] = hash.match(/^#\/roadmaps\/([a-z0-9-]{1,64})$/) ?? [];
  if (roadmap) return { tela: "roadmaps", roadmap };
  const [, tipo, id] = hash.match(/^#\/([pc])\/([^/?#]+)$/) ?? [];
  if (!id) return { tela: "inicio" };
  let valor: string;
  try { valor = decodeURIComponent(id); } catch { return { tela: "inicio" }; }
  return tipo === "p" ? { tela: "projeto", projeto: valor } : { tela: "conversa", conversa: valor };
}

export function urlDaRota(rota: Rota): string {
  const base = location.pathname + location.search;
  if (rota.tela === "atividade") return `${base}#/atividade`;
  if (rota.tela === "uso") return `${base}#/uso`;
  if (rota.tela === "sobre-mim") return `${base}#/sobre-mim`;
  if (rota.tela === "tarefas") return `${base}#/tarefas`;
  if (rota.tela === "rotina") return `${base}#/rotina`;
  if (rota.tela === "telegram") return `${base}#/telegram`;
  if (rota.tela === "roadmaps") return `${base}#/roadmaps${rota.roadmap ? `/${rota.roadmap}` : ""}`;
  if (rota.tela === "projeto") return `${base}#/p/${encodeURIComponent(rota.projeto)}`;
  if (rota.tela === "conversa") return `${base}#/c/${encodeURIComponent(rota.conversa)}`;
  return base;
}

/**
 * Onde o usuário está, guardado no endereço. Atualizar a página volta para a mesma conversa — antes
 * a tela vivia só no estado do React e todo refresh caía no início — e o gesto de voltar do celular
 * anda pelas telas do app como em qualquer outro.
 */
export function useRota() {
  const [rota, setRota] = useState<Rota>(() => lerRota());

  useEffect(() => {
    const sincronizar = () => setRota(lerRota());
    window.addEventListener("popstate", sincronizar);
    window.addEventListener("hashchange", sincronizar);
    return () => {
      window.removeEventListener("popstate", sincronizar);
      window.removeEventListener("hashchange", sincronizar);
    };
  }, []);

  const ir = useCallback((destino: Rota) => {
    history.pushState({ omni: true }, "", urlDaRota(destino));
    setRota(destino);
  }, []);

  /** Volta pela pilha quando a tela anterior é do próprio app; aberto direto num endereço (favorito,
   *  app da tela inicial reaberto), não há para onde voltar — então vai para a tela-pai. */
  const voltar = useCallback((pai: Rota) => {
    if ((history.state as { omni?: boolean } | null)?.omni) { history.back(); return; }
    history.replaceState(null, "", urlDaRota(pai));
    setRota(pai);
  }, []);

  return { rota, ir, voltar };
}
