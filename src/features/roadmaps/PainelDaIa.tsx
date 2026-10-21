// Fork (Roadmaps): conversa com a IA ao lado da trilha (dúvida, aprofundar, exercícios, reajustar).
import ReactMarkdown from "react-markdown";
import rehypeSanitize from "rehype-sanitize";
import remarkGfm from "remark-gfm";
import { useEffect, useRef, useState, type FormEvent } from "react";
import { Button, Textarea } from "../../components/ui";
import { adicionarTarefas, aplicarPlano, chaves, lerConversaDoPainel, mensagemNoPainel } from "./acoes";
import { PedidoEmAndamento, textoDoErro } from "./comuns";
import { foiCancelado } from "./ia";
import { lembranca, lembrar, tarefaEmAndamento, useLoja } from "./loja";
import { etapasComProgressoQueSomem, etapasEmOrdem } from "./progresso";
import type { RoadmapAberto } from "./armazenamento";
import type { MensagemDoPainel, Plano } from "./tipos";

interface Propostas { plano?: Plano; tarefas?: { etapaId: string; textos: string[] } }
/** Proposta ainda não aplicada e conversa já lida sobrevivem a fechar e abrir o painel (na mesma execução do app). */
const propostasPendentes = {
  get: (id: string) => lembranca<Propostas>(`propostas:${id}`),
  set: (id: string, p: Propostas) => lembrar(`propostas:${id}`, p),
};
const conversas = {
  has: (id: string) => lembranca(`conversa:${id}`) !== undefined,
  get: (id: string) => lembranca<MensagemDoPainel[]>(`conversa:${id}`),
  set: (id: string, m: MensagemDoPainel[]) => lembrar(`conversa:${id}`, m),
};

const SUGESTOES = ["Explique esta etapa com um exemplo", "Me dê mais exercícios", "Estou atrasado, reajuste o caminho", "Isso ficou fácil, acelere"];

export function PainelDaIa({ r, etapaId }: { r: RoadmapAberto; etapaId?: string }) {
  useLoja();
  const id = r.meta.id;
  const [mensagens, setMensagens] = useState<MensagemDoPainel[] | null>(conversas.get(id) ?? null);
  const [texto, setTexto] = useState("");
  const [erro, setErro] = useState("");
  const [propostas, setPropostas] = useState<Propostas>(propostasPendentes.get(id) ?? {});
  const fim = useRef<HTMLDivElement>(null);
  const ocupado = Boolean(tarefaEmAndamento(chaves.painel(id)));
  const etapa = etapaId && r.plano ? etapasEmOrdem(r.plano).find((e) => e.id === etapaId) : undefined;

  useEffect(() => {
    if (conversas.has(id)) return;
    let vivo = true;
    void lerConversaDoPainel(id).then((m) => { conversas.set(id, m); if (vivo) setMensagens(m); }).catch((e) => { if (vivo) { setMensagens([]); setErro(textoDoErro(e)); } });
    return () => { vivo = false; };
  }, [id]);
  useEffect(() => { fim.current?.scrollIntoView?.({ block: "end" }); }, [mensagens?.length, ocupado]);

  const guardarPropostas = (p: Propostas) => { propostasPendentes.set(id, p); setPropostas(p); };
  const acrescentar = (m: MensagemDoPainel) => {
    const lista = [...(conversas.get(id) ?? []), m];
    conversas.set(id, lista);
    setMensagens(lista);
  };

  const enviar = (conteudo: string) => {
    if (!conteudo.trim() || ocupado || r.somenteLeitura) return;
    setTexto(""); setErro("");
    const historico = conversas.get(id) ?? [];
    // Aparece na hora; o arquivo recebe a mesma pergunta antes de a IA começar.
    acrescentar({ de: "eu", texto: conteudo.trim(), em: new Date().toISOString(), ...(etapaId ? { etapaId } : {}) });
    void mensagemNoPainel(id, conteudo, etapaId, historico).then((resposta) => {
      acrescentar(resposta.mensagem);
      const novas: Propostas = { ...propostasPendentes.get(id) };
      if (resposta.plano) novas.plano = resposta.plano;
      if (resposta.tarefas && etapaId) novas.tarefas = { etapaId, textos: resposta.tarefas };
      guardarPropostas(novas);
    }).catch((e) => { if (!foiCancelado(e)) setErro(textoDoErro(e)); });
  };
  const submeter = (e: FormEvent) => { e.preventDefault(); enviar(texto); };

  const somem = propostas.plano && r.plano ? etapasComProgressoQueSomem(r.plano, propostas.plano, r.progresso) : [];
  const etapasNoNovo = propostas.plano ? etapasEmOrdem(propostas.plano).length : 0;

  return (
    <div className="roadmap-ia">
      <div className="roadmap-ia-mensagens" aria-live="polite">
        {mensagens === null ? <p className="text-xs text-text-muted">Abrindo a conversa…</p>
          : mensagens.length === 0 ? <p className="text-xs text-text-muted">Pergunte qualquer coisa sobre o caminho{etapa ? ` ou sobre “${etapa.titulo}”` : ""}.</p>
          : mensagens.map((m, i) => (
            <div key={i} className={`roadmap-balao ${m.de === "eu" ? "is-eu" : "is-ia"}`}>
              {m.de === "ia" ? <div className="roadmap-markdown"><ReactMarkdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeSanitize]}>{m.texto}</ReactMarkdown></div> : m.texto}
            </div>
          ))}
        <div ref={fim} />
      </div>

      {propostas.plano && (
        <div className="roadmap-proposta glass" role="region" aria-label="Reajuste proposto">
          <strong className="text-sm text-text-primary">Caminho reajustado</strong>
          <p className="text-xs text-text-secondary">{propostas.plano.modulos.length} módulos · {etapasNoNovo} etapas{r.plano ? ` (hoje: ${etapasEmOrdem(r.plano).length})` : ""}. O progresso das etapas que continuam é mantido.</p>
          {somem.length > 0 && <p className="text-xs text-warning">Sai do caminho com progresso: {somem.map((e) => e.titulo).join(", ")}. O progresso fica guardado no arquivo.</p>}
          <div className="flex justify-end gap-2">
            <Button size="sm" variant="ghost" onClick={() => guardarPropostas({ ...propostas, plano: undefined })}>Descartar</Button>
            <Button size="sm" variant="primary" disabled={r.somenteLeitura} onClick={() => {
              const plano = propostas.plano;
              if (!plano) return;
              void aplicarPlano(id, plano).then(() => guardarPropostas({ ...propostas, plano: undefined })).catch((e) => setErro(textoDoErro(e)));
            }}>Aplicar</Button>
          </div>
        </div>
      )}
      {propostas.tarefas && (
        <div className="roadmap-proposta glass" role="region" aria-label="Exercícios sugeridos">
          <strong className="text-sm text-text-primary">Guardar {propostas.tarefas.textos.length} exercícios na etapa?</strong>
          <div className="flex justify-end gap-2">
            <Button size="sm" variant="ghost" onClick={() => guardarPropostas({ ...propostas, tarefas: undefined })}>Agora não</Button>
            <Button size="sm" variant="primary" disabled={r.somenteLeitura} onClick={() => {
              const t = propostas.tarefas;
              if (!t) return;
              void adicionarTarefas(id, t.etapaId, t.textos).then(() => guardarPropostas({ ...propostas, tarefas: undefined })).catch((e) => setErro(textoDoErro(e)));
            }}>Guardar</Button>
          </div>
        </div>
      )}

      <PedidoEmAndamento chave={chaves.painel(id)} />
      {erro && <p role="alert" className="roadmap-erro">{erro}</p>}
      {!ocupado && mensagens?.length === 0 && (
        <div className="flex flex-wrap gap-1.5">
          {SUGESTOES.map((s) => <Button key={s} size="sm" variant="ghost" onClick={() => enviar(s)}>{s}</Button>)}
        </div>
      )}
      <form className="space-y-2" onSubmit={submeter}>
        <Textarea aria-label="Mensagem para a IA" rows={2} value={texto} disabled={ocupado || r.somenteLeitura} onChange={(e) => setTexto(e.target.value)}
          onKeyDown={(e) => { if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); enviar(texto); } }}
          placeholder={etapa ? `Sobre “${etapa.titulo}”…` : "Sua dúvida…"} />
        <div className="flex justify-end"><Button size="sm" variant="primary" type="submit" disabled={ocupado || !texto.trim()}>Enviar</Button></div>
      </form>
    </div>
  );
}
