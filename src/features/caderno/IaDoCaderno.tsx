// Fork (Caderno): a conversa com a IA ao lado da folha. Ela lê o caderno inteiro; cada resposta pode
// ser colada na página aberta com um clique.
import ReactMarkdown from "react-markdown";
import rehypeSanitize from "rehype-sanitize";
import remarkGfm from "remark-gfm";
import { useEffect, useRef, useState, type FormEvent } from "react";
import { Button, Textarea } from "../../components/ui";
import { PedidoEmAndamento, textoDoErro } from "../roadmaps/comuns";
import { foiCancelado } from "../roadmaps/ia";
import { tarefaEmAndamento, useLoja } from "../roadmaps/loja";
import { chaveDaIa, perguntarAoCaderno, SUGESTOES } from "./ia";
import { conversaDoCaderno, digitarNaPagina, gravarPagina, lerConversaDoCaderno, lerPagina, useCadernos } from "./loja";
import type { MetaDoCaderno } from "./tipos";

export function IaDoCaderno({ meta, paginaId }: { meta: MetaDoCaderno; paginaId?: string }) {
  useCadernos();
  useLoja();
  const id = meta.id;
  const mensagens = conversaDoCaderno(id);
  const [texto, setTexto] = useState("");
  const [web, setWeb] = useState(false);
  const [erro, setErro] = useState("");
  const [colada, setColada] = useState<number | null>(null);
  const [sugestoes, setSugestoes] = useState(false);
  const fim = useRef<HTMLDivElement>(null);
  const ocupado = Boolean(tarefaEmAndamento(chaveDaIa(id)));

  useEffect(() => { if (!mensagens) void lerConversaDoCaderno(id).catch((e) => setErro(textoDoErro(e))); }, [id, mensagens]);
  useEffect(() => { fim.current?.scrollIntoView?.({ block: "end" }); }, [mensagens?.length, ocupado]);

  const enviar = (conteudo: string) => {
    if (!conteudo.trim() || ocupado) return;
    setTexto(""); setErro("");
    void perguntarAoCaderno(id, paginaId, conteudo, web).catch((e) => { if (!foiCancelado(e)) setErro(textoDoErro(e)); });
  };
  const submeter = (e: FormEvent) => { e.preventDefault(); enviar(texto); };

  const colar = async (resposta: string, i: number) => {
    if (!paginaId) return;
    const atual = await lerPagina(id, paginaId);
    digitarNaPagina(id, paginaId, `${atual.trimEnd()}${atual.trim() ? "\n\n" : ""}${resposta.trim()}\n`);
    await gravarPagina(id, paginaId);
    setColada(i);
  };

  return (
    <aside className="caderno-ia" aria-label="IA do caderno">
      <header className="caderno-ia-topo">
        <strong>IA do caderno</strong>
        <span className="text-[11px] text-text-muted">{meta.ia.cli === "claude" ? "Claude" : "Codex"} · lê todas as páginas</span>
      </header>
      <div className="caderno-ia-mensagens" aria-live="polite">
        {!mensagens ? <p className="text-xs text-text-muted">Abrindo a conversa…</p>
          : mensagens.length === 0 ? <p className="text-xs text-text-muted">Pergunte qualquer coisa sobre o que você escreveu{meta.tipo === "livro" ? " ou sobre o livro" : ""}. A IA lê o caderno inteiro.</p>
          : mensagens.map((m, i) => (
            <div key={i} className={`caderno-balao ${m.de === "eu" ? "is-eu" : "is-ia"}`}>
              {m.de === "ia" ? (
                <>
                  <div className="caderno-markdown"><ReactMarkdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeSanitize]}>{m.texto}</ReactMarkdown></div>
                  {paginaId && (
                    <button type="button" className="caderno-mini" onClick={() => void colar(m.texto, i).catch((e) => setErro(textoDoErro(e)))}>
                      {colada === i ? "✓ Colado na página" : "Colar na página"}
                    </button>
                  )}
                </>
              ) : m.texto}
            </div>
          ))}
        <div ref={fim} />
      </div>
      <PedidoEmAndamento chave={chaveDaIa(id)} />
      {erro && <p role="alert" className="text-xs text-danger">{erro}</p>}
      {/* Sugestões: abertas na conversa vazia; depois, num toque em "Sugestões". */}
      {!ocupado && (mensagens?.length === 0 || sugestoes) && (
        <div className="caderno-sugestoes">
          {SUGESTOES[meta.tipo].map((s) => <button key={s} type="button" className="caderno-sugestao" onClick={() => { setSugestoes(false); enviar(s); }}>{s}</button>)}
        </div>
      )}
      <form className="space-y-2" onSubmit={submeter}>
        <Textarea aria-label="Mensagem para a IA do caderno" rows={3} value={texto} disabled={ocupado} onChange={(e) => setTexto(e.target.value)}
          onKeyDown={(e) => { if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); enviar(texto); } }}
          placeholder="Converse sobre suas anotações…" />
        <div className="flex items-center gap-2">
          <label className="flex items-center gap-1.5 text-xs text-text-secondary" title="Deixa a IA buscar na internet (ex.: contexto do livro e do autor)">
            <input type="checkbox" checked={web} onChange={(e) => setWeb(e.target.checked)} /> Pesquisar na web
          </label>
          <span className="flex-1" />
          {!!mensagens?.length && <button type="button" className="caderno-mini" aria-expanded={sugestoes} onClick={() => setSugestoes((s) => !s)}>✦ Sugestões</button>}
          <Button size="sm" variant="primary" type="submit" disabled={ocupado || !texto.trim()}>Enviar</Button>
        </div>
      </form>
    </aside>
  );
}
