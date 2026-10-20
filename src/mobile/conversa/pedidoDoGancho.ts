// Fork (Onda 2, item 2): responder o pedido de permissão que o gancho do Claude segurou — pela conversa
// (PC, web, celular) e pelo nó do fluxo do orquestrador. O engine confere o `id`: outro pedido → 412.
import { useEffect, useState } from "react";
import { actionKey, request, type PedidoDoGancho } from "../api";

const conversa = (id: string) => `/conversas/${encodeURIComponent(id)}`;
const json = { "Content-Type": "application/json" };

/** Permitir ou negar (com o "dizer o que fazer" opcional, que vira a mensagem para o agente). */
export function responderPedido(conversaId: string, pedido: PedidoDoGancho, permitir: boolean, mensagem?: string) {
  const corpo: Record<string, unknown> = { permitir, pedido: pedido.id };
  const texto = mensagem?.trim();
  if (!permitir && texto) corpo.mensagem = texto;
  return request(`${conversa(conversaId)}/aprovar`, {
    method: "POST", headers: { ...json, "Idempotency-Key": actionKey() }, body: JSON.stringify(corpo),
  });
}

/** "Responder no terminal": solta o pedido e a CLI mostra o diálogo dela. */
export function soltarPedido(conversaId: string, pedido: PedidoDoGancho) {
  return request(`${conversa(conversaId)}/pedido/terminal`, { method: "POST", headers: json, body: JSON.stringify({ pedido: pedido.id }) });
}

/** Segundos até o pedido ir para o terminal (atualiza a cada segundo). */
export function useSegundosRestantes(expiraEm: number | undefined): number | null {
  const [agora, setAgora] = useState(() => Date.now());
  useEffect(() => {
    if (!expiraEm) return;
    const relogio = window.setInterval(() => setAgora(Date.now()), 1_000);
    return () => window.clearInterval(relogio);
  }, [expiraEm]);
  return expiraEm ? Math.max(0, Math.ceil((expiraEm - agora) / 1_000)) : null;
}

/** 275 → "4:35". */
export function formatarRestante(segundos: number): string {
  return `${Math.floor(segundos / 60)}:${String(segundos % 60).padStart(2, "0")}`;
}
