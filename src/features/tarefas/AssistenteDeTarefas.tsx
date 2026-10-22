// Fork (Tarefas): coluna de conversa com a IA dentro do painel de tarefas.
import { useEffect, useRef, useState } from "react";
import { Button } from "../../components/ui";
import { SparkleIcon } from "../../components/ui/PixelIcon";
import { EscolhaDaIa } from "../roadmaps/comuns";
import { cancelar, foiCancelado } from "../roadmaps/ia";
import {
  aplicarAcoes, CHAVE_DO_PEDIDO, desfazerFeitos, gravarChat, lerChat, montarPedido, perguntarAoAssistente, type ChatSalvo, type MensagemDoChat,
} from "./assistente";
import { destacar, pedirAbrirTarefa, pessoasConhecidas, salvar, useTarefas } from "./loja";
import { escondidaPelosFiltros, limparFiltros } from "./filtros";

const SUGESTOES = [
  "Organiza o meu dia de hoje por prioridade",
  "Cria uma tarefa de consulta médica na sexta às 15h",
  "Quebra a tarefa mais importante em passos",
  "Passa as atrasadas para amanhã",
];

export function AssistenteDeTarefas({ onFechar }: { onFechar: () => void }) {
  const e = useTarefas();
  const [chat, setChat] = useState<ChatSalvo>(lerChat);
  const [texto, setTexto] = useState("");
  const [pensando, setPensando] = useState(false);
  const [erro, setErro] = useState("");
  const [ajustes, setAjustes] = useState(false);
  const fim = useRef<HTMLDivElement>(null);
  const campo = useRef<HTMLTextAreaElement>(null);

  useEffect(() => { gravarChat(chat); }, [chat]);
  useEffect(() => { fim.current?.scrollIntoView?.({ block: "end" }); }, [chat.mensagens.length, pensando]);
  // Fechar o painel no meio de um pedido cancela a CLI.
  useEffect(() => () => cancelar(CHAVE_DO_PEDIDO), []);

  const enviar = async (pergunta = texto) => {
    const limpo = pergunta.trim();
    if (!limpo || pensando) return;
    setErro(""); setTexto(""); setPensando(true);
    const minha: MensagemDoChat = { de: "eu", texto: limpo, em: new Date().toISOString() };
    const historico = chat.mensagens;
    setChat((c) => ({ ...c, mensagens: [...c.mensagens, minha] }));
    try {
      const pedido = montarPedido(limpo, e.tarefas, pessoasConhecidas(e), e.ajustes.eu, historico);
      const { resposta, acoes } = await perguntarAoAssistente(chat.ia, pedido);
      const { gravar, feitos } = aplicarAcoes(acoes, e.tarefas, e.ajustes.eu.id);
      await Promise.all(gravar.map((t) => salvar(t).catch(() => undefined)));
      // O que a IA mudou aparece: brilha na tela e, se um filtro esconderia, os filtros saem.
      destacar(gravar.map((t) => t.id));
      if (gravar.some((t) => !t.apagada && escondidaPelosFiltros(t, e.ajustes))) limparFiltros();
      setChat((c) => ({ ...c, mensagens: [...c.mensagens, { de: "ia", texto: resposta || "Feito.", em: new Date().toISOString(), feitos }] }));
    } catch (err) {
      if (!foiCancelado(err)) setErro(err instanceof Error ? err.message : String(err));
    } finally {
      setPensando(false);
      campo.current?.focus();
    }
  };

  const desfazer = (indice: number) => {
    const m = chat.mensagens[indice];
    if (!m?.feitos?.length || m.desfeito) return;
    for (const t of desfazerFeitos(m.feitos, e.tarefas)) void salvar(t).catch(() => undefined);
    setChat((c) => ({ ...c, mensagens: c.mensagens.map((x, i) => (i === indice ? { ...x, desfeito: true } : x)) }));
  };

  return (
    <aside className="tarefas-ia" aria-label="Assistente de tarefas">
      <header className="tarefas-ia-topo">
        <SparkleIcon className="h-4 w-4 text-accent" aria-hidden />
        <strong className="flex-1 text-sm text-text-primary">Assistente</strong>
        <button type="button" className="tarefa-mini" title="Qual IA e conta usar" aria-expanded={ajustes} onClick={() => setAjustes((v) => !v)}>⚙</button>
        {chat.mensagens.length > 0 && <button type="button" className="tarefa-mini" title="Limpar conversa" onClick={() => setChat((c) => ({ ...c, mensagens: [] }))}>⟲</button>}
        <button type="button" className="tarefa-mini" aria-label="Esconder assistente" onClick={onFechar}>×</button>
      </header>
      {ajustes && <div className="border-b border-border-subtle/60 p-3"><EscolhaDaIa valor={chat.ia} onChange={(ia) => setChat((c) => ({ ...c, ia }))} /></div>}
      <div className="tarefas-ia-mensagens">
        {!chat.mensagens.length && (
          <div className="space-y-3 p-1">
            <p className="text-sm text-text-secondary">Me diga o que precisa fazer, do jeito que vier na cabeça. Eu crio, organizo e reagendo as tarefas para você.</p>
            <div className="flex flex-col gap-1.5">
              {SUGESTOES.map((s) => <button key={s} type="button" className="tarefas-ia-sugestao" onClick={() => void enviar(s)}>{s}</button>)}
            </div>
          </div>
        )}
        {chat.mensagens.map((m, i) => (
          <div key={i} className={`tarefas-ia-balao ${m.de === "eu" ? "is-eu" : "is-ia"}`}>
            <p>{m.texto}</p>
            {m.feitos && m.feitos.length > 0 && (
              <div className="mt-2 space-y-1">
                {m.feitos.map((f, j) => (
                  <button key={j} type="button" disabled={m.desfeito || f.descricao.startsWith("Apagada")} title="Abrir a tarefa" onClick={() => pedirAbrirTarefa(f.id)}
                    className={`tarefas-ia-feito block text-left hover:underline${m.desfeito ? " line-through opacity-60" : ""}`}>✓ {f.descricao}</button>
                ))}
                {!m.desfeito && <button type="button" className="text-xs text-accent hover:underline" onClick={() => desfazer(i)}>Desfazer</button>}
              </div>
            )}
          </div>
        ))}
        {pensando && (
          <div className="tarefas-ia-balao is-ia flex items-center gap-2">
            <span className="tarefas-ia-pontos" aria-hidden><i /><i /><i /></span>
            <span className="text-xs text-text-muted">Pensando…</span>
            <button type="button" className="ml-auto text-xs text-text-muted hover:text-text-primary" onClick={() => cancelar(CHAVE_DO_PEDIDO)}>Cancelar</button>
          </div>
        )}
        {erro && <p role="alert" className="tarefas-aviso is-erro">{erro}</p>}
        <div ref={fim} />
      </div>
      <div className="tarefas-ia-campo">
        <textarea ref={campo} rows={2} value={texto} placeholder="Ex.: dentista amanhã às 10h" aria-label="Mensagem para o assistente"
          onChange={(ev) => setTexto(ev.target.value)}
          onKeyDown={(ev) => { if (ev.key === "Enter" && !ev.shiftKey) { ev.preventDefault(); void enviar(); } }} />
        <Button size="sm" variant="primary" disabled={!texto.trim() || pensando} onClick={() => void enviar()}>Enviar</Button>
      </div>
      <p className="px-3 pb-2 text-[10px] text-text-muted">Usa a CLI oficial ({chat.ia.cli === "claude" ? "Claude" : "Codex"}) na sua conta, sem acesso a arquivos. Gasta uso da conta.</p>
    </aside>
  );
}
