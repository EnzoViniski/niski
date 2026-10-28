// Comandos oficiais por lista fechada; modelo/esforço continuam em AjustesDaConversa.
import { useEffect, useState } from "react";
import { request } from "../api";
import { MODOS_DE_PERMISSAO } from "../SeloDoModo";
import { TerminalFlutuante } from "./TerminalFlutuante";
import "./controlesDaConversa.css";

// Os do Codex (aprovação/sandbox na linha; trocar reabre a mesma conversa, `modo_codex.rs`).
export type ModoDoCodex = "default" | "semPerguntar" | "acessoTotal" | "somenteLeitura";
export type ModoDePermissao = "default" | "acceptEdits" | "plan" | "auto" | ModoDoCodex;
export type ComandoDaConversa = "fast" | "branch" | "rewind" | "compact";
const MODOS: ModoDePermissao[] = ["default", "acceptEdits", "plan", "auto"];
const MODOS_DO_CODEX: ModoDoCodex[] = ["default", "semPerguntar", "acessoTotal", "somenteLeitura"];

export function ControlesDaConversa({ conversaId, provider, modo, fast, pronto, onModo, onComando, onAgendar }: {
  conversaId: string; provider: string | null; modo?: string | null; fast?: boolean | null; pronto: boolean;
  onAgendar?: () => void; // Agendar o rascunho pelo menu existente (3D).
  onModo: (alvo: ModoDePermissao) => Promise<void>;
  onComando: (comando: ComandoDaConversa, argumento?: string) => Promise<void>;
}) {
  const [pedido, setPedido] = useState<string | null>(null);
  const [menu, setMenu] = useState(false);
  const [terminal, setTerminal] = useState<"rewind" | "branch" | null>(null);
  const [erro, setErro] = useState<string | null>(null);
  const [suportaFast, setSuportaFast] = useState(false);
  useEffect(() => {
    let cancelado = false;
    setSuportaFast(false);
    if (provider === "claude" && pronto) void request<{ fast_supported?: boolean }>(`/conversas/${encodeURIComponent(conversaId)}/opcoes`)
      .then((o) => { if (!cancelado) setSuportaFast(o.fast_supported === true); }).catch(() => undefined);
    return () => { cancelado = true; };
  }, [conversaId, provider, pronto]);
  const bloqueado = !pronto || pedido !== null;
  async function executar(nome: string, acao: () => Promise<void>) {
    if (bloqueado) return;
    setPedido(nome); setErro(null); setMenu(false);
    try { await acao(); } catch (e) { setErro(String(e)); } finally { setPedido(null); }
  }
  if (provider !== "claude" && provider !== "codex" && !onAgendar) return null;
  return <div className="conversa-controles" aria-label="Controles da conversa">
    {provider === "claude" && <label className="conversa-controle-modo">
      <span className="sr-only">Modo do agente</span>
      <select aria-label="Modo do agente" value={MODOS.includes(modo as ModoDePermissao) ? modo! : ""} disabled={bloqueado}
        title="Modo confirmado pela CLI" onChange={(e) => { const alvo = e.target.value as ModoDePermissao; void executar(alvo, () => onModo(alvo)); }}>
        {!MODOS.includes(modo as ModoDePermissao) && <option value="">{modo ? MODOS_DE_PERMISSAO[modo]?.rotulo ?? modo : "Modo não confirmado"}</option>}
        {MODOS.map((m) => <option key={m} value={m}>{MODOS_DE_PERMISSAO[m].rotulo}</option>)}
      </select>
    </label>}
    {provider === "codex" && <label className="conversa-controle-modo">
      <span className="sr-only">Modo do Codex</span>
      <select aria-label="Modo do Codex" value={MODOS_DO_CODEX.includes(modo as ModoDoCodex) ? modo! : ""} disabled={bloqueado}
        title="Trocar reabre o Codex na mesma conversa, já no modo escolhido"
        onChange={(e) => { const alvo = e.target.value as ModoDoCodex; void executar(alvo, () => onModo(alvo)); }}>
        {!MODOS_DO_CODEX.includes(modo as ModoDoCodex) && <option value="">{modo ? MODOS_DE_PERMISSAO[modo]?.rotulo ?? modo : "Modo da conta"}</option>}
        {MODOS_DO_CODEX.map((m) => <option key={m} value={m} title={MODOS_DE_PERMISSAO[m].dica}>{MODOS_DE_PERMISSAO[m].rotulo}</option>)}
      </select>
    </label>}
    {provider === "claude" && suportaFast && <button type="button" className="composer-modo" disabled={bloqueado}
      aria-label={fast === true ? "Desligar fast" : "Ligar fast"} aria-pressed={fast === true}
      title="Fast tem custo maior. O estado é confirmado pela CLI."
      onClick={() => void executar("fast", () => onComando("fast", fast === true ? "off" : "on"))}>Fast{fast == null ? " ?" : fast ? " ligado" : " desligado"}</button>}
    <button type="button" className="composer-botao" aria-label="Mais controles" aria-expanded={menu} disabled={bloqueado && !onAgendar}
      onClick={() => setMenu(!menu)}>…</button>
    {menu && <div className="conversa-controles-menu" role="group" aria-label="Mais ações da conversa">
      {onAgendar && <button type="button" onClick={() => { setMenu(false); onAgendar(); }}>Agendar esta mensagem</button>}
      {provider === "claude" && <>
        <button type="button" disabled={bloqueado} onClick={() => void executar("rewind", async () => { await onComando("rewind"); setTerminal("rewind"); })}>Voltar a um ponto</button>
        <button type="button" disabled={bloqueado} onClick={() => void executar("branch", async () => { await onComando("branch"); setTerminal("branch"); })}>Ramificar aqui</button>
      </>}
      {(provider === "claude" || provider === "codex") && <button type="button" disabled={bloqueado} onClick={() => void executar("compact", () => onComando("compact"))}>Compactar</button>}
    </div>}
    {pedido && <span role="status" className="m-dica">Aguardando a CLI…</span>}
    {erro && <span role="alert" className="m-alerta">{erro}</span>}
    {terminal && <TerminalFlutuante conversaId={conversaId} tipo={terminal} onFechar={() => setTerminal(null)} />}
  </div>;
}
