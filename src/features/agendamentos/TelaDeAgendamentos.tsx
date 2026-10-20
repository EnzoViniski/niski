import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { usePoll } from "../../mobile/usePoll";
import type { Conversation } from "../../mobile/api";
import { agirNoAgendamento, conversasAgendaveis, ESTADOS, listarAgendamentos, opcoesAgendamentos, type Agendamento, type Opcoes } from "./agendamentosService";
import { EditorDeAgendamento, type Rascunho } from "./EditorDeAgendamento";
import "./agendamentos.css";
function descrever(a: Agendamento) {
  const r = a.regra;
  if (r.tipo === "uma_vez") return "Uma vez";
  if (r.tipo === "intervalo") return `A cada ${r.minutos % 60 === 0 ? `${r.minutos / 60} h` : `${r.minutos} min`}`;
  if (r.tipo === "diario") return `Diário às ${r.horario}`;
  return `${r.dias.map(d => ["Seg", "Ter", "Qua", "Qui", "Sex", "Sáb", "Dom"][d]).join(", ")} às ${r.horario}`;
}
export function TelaDeAgendamentos({ inicial, onFechar }: { inicial?: Rascunho; onFechar: () => void }) {
  const [itens, setItens] = useState<Agendamento[] | null>(null);
  const [opcoes, setOpcoes] = useState<Opcoes | null>(null);
  const [conversas, setConversas] = useState<Conversation[]>([]);
  const [editor, setEditor] = useState<Agendamento | "novo" | null>(inicial ? "novo" : null);
  const [erro, setErro] = useState<string | null>(null);
  const [ocupado, setOcupado] = useState(false);
  const [apagar, setApagar] = useState<string | null>(null);
  const painel = useRef<HTMLElement>(null);
  const vivo = useRef(true);
  async function atualizar() { const lista = await listarAgendamentos(); if (vivo.current) { setItens(lista); setErro(null); } }
  usePoll(atualizar, e => { if (vivo.current) setErro(String(e)); }, [], 5_000);
  useEffect(() => {
    vivo.current = true;
    void Promise.all([opcoesAgendamentos(), conversasAgendaveis()]).then(([o, c]) => { if (vivo.current) { setOpcoes(o); setConversas(c); } }).catch(e => { if (vivo.current) setErro(String(e)); });
    return () => { vivo.current = false; };
  }, []);
  useEffect(() => {
    const anterior = document.activeElement as HTMLElement | null;
    painel.current?.querySelector<HTMLButtonElement>("button")?.focus();
    const tecla = (e: KeyboardEvent) => {
      if (e.key === "Escape") { e.preventDefault(); e.stopImmediatePropagation(); onFechar(); }
      if (e.key === "Tab") {
        const lista = painel.current?.querySelectorAll<HTMLElement>("button:not(:disabled), input:not(:disabled), textarea:not(:disabled), select:not(:disabled), summary");
        const primeiro = lista?.[0], ultimo = lista?.[lista.length - 1];
        if (e.shiftKey && document.activeElement === primeiro) { e.preventDefault(); ultimo?.focus(); }
        else if (!e.shiftKey && document.activeElement === ultimo) { e.preventDefault(); primeiro?.focus(); }
      }
    };
    window.addEventListener("keydown", tecla, true);
    return () => { window.removeEventListener("keydown", tecla, true); anterior?.focus(); };
  }, [onFechar]);
  async function agir(id: string, acao: "rodar" | "pausar" | "retomar" | "apagar") {
    setOcupado(true); setErro(null);
    try { await agirNoAgendamento(id, acao); setApagar(null); await atualizar(); }
    catch (e) { setErro(e instanceof Error ? e.message : String(e)); } finally { setOcupado(false); }
  }
  return createPortal(<div className="agenda-fundo" onMouseDown={e => { if (e.target === e.currentTarget) onFechar(); }}>
    <section ref={painel} className="agenda-painel" role="dialog" aria-modal="true" aria-label="Agendamentos">
      <header><div><small>AGENTES · ROTINAS</small><h2>Agendamentos</h2></div><button type="button" aria-label="Fechar agendamentos" onClick={onFechar}>×</button></header>
      <p className="agenda-aviso">O engine precisa estar ligado e o computador acordado. Depois de uma suspensão, recupera uma execução por agendamento, sem acumular as perdidas.</p>
      {erro && <p role="alert" className="text-danger">{erro}</p>}
      {editor && opcoes ? <EditorDeAgendamento key={typeof editor === "object" ? editor.id : "novo"} existente={typeof editor === "object" ? editor : undefined} inicial={inicial} opcoes={opcoes} conversas={conversas}
        onCancelar={() => setEditor(null)} onSalvar={() => { setEditor(null); void atualizar().catch(e => setErro(String(e))); }} /> : <>
        <div className="agenda-barra"><span>{itens?.length ?? 0} agendados</span><button type="button" className="agenda-primario" disabled={!opcoes} onClick={() => setEditor("novo")}>Criar agendamento</button></div>
        {itens === null && !erro && <p role="status">Carregando agendamentos…</p>}
        {itens?.length === 0 && <div className="agenda-vazio"><h3>Sua próxima rotina começa aqui</h3><p>Agende um prompt para uma conversa ou abra um agente no horário escolhido.</p></div>}
        <ul className="agenda-lista">{itens?.map(a => <li key={a.id}>
          <div className="agenda-linha"><h3>{a.titulo}</h3><span className={a.pausado ? "" : "text-accent"}>{a.pendente ? "Em execução" : a.pausado ? "Pausado" : a.proxima_ms ? "Ativo" : "Concluído"}</span></div>
          <p>{descrever(a)} · {a.destino.tipo === "conversa" ? "Conversa existente" : `Novo agente · ${a.destino.plataforma}${a.destino.worktree ? " · worktree isolada" : ""}`}</p>
          <p>Próxima: {a.pausado ? "pausada" : a.proxima_ms ? new Date(a.proxima_ms).toLocaleString("pt-BR", { timeZoneName: "short" }) : "sem nova execução"}</p>
          <div className="agenda-acoes"><button disabled={ocupado || Boolean(a.pendente)} onClick={() => setEditor(a)}>Editar</button>
            <button disabled={ocupado || Boolean(a.pendente)} onClick={() => void agir(a.id, "rodar")}>Rodar agora</button>
            <button disabled={ocupado} onClick={() => void agir(a.id, a.pausado ? "retomar" : "pausar")}>{a.pausado ? "Retomar" : "Pausar"}</button>
            <button disabled={ocupado || Boolean(a.pendente)} onClick={() => setApagar(a.id)}>Apagar</button></div>
          {apagar === a.id && <div className="agenda-acoes" role="group" aria-label="Confirmar exclusão"><span>Apagar este agendamento?</span><button disabled={ocupado} onClick={() => void agir(a.id, "apagar")}>Confirmar exclusão</button><button onClick={() => setApagar(null)}>Manter</button></div>}
          {!!a.historico.length && <details><summary>Últimas execuções ({a.historico.length})</summary><ul>{[...a.historico].reverse().map(h => <li key={h.em_ms}><time>{new Date(h.em_ms).toLocaleString("pt-BR")}</time> · {ESTADOS[h.estado] ?? h.estado}{h.conversa && <small> · {h.conversa}</small>}</li>)}</ul></details>}
        </li>)}</ul>
      </>}
    </section>
  </div>, document.body);
}
