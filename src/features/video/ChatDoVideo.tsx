// Fork (Editor de vídeo): chat com o agente que edita a composição. Cada pedido guarda antes uma
// versão (desfazer volta nela), mostra o que o agente está fazendo e recarrega o preview a cada
// arquivo que ele mexe. A conversa fica em `conversa.jsonl` e a sessão da CLI continua de onde parou.
import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useRef, useState } from "react";
import ReactMarkdown from "react-markdown";
import { Button } from "../../components/ui";
import { LoaderIcon, SendIcon } from "../../components/ui/PixelIcon";
import * as disco from "./armazenamento";
import { ChipDaIa, nomeDaCli } from "./EscolhaDaIa";
import type { AcoesDoProjeto } from "./useProjeto";
import type { IaDoVideo, MensagemDoChat } from "./tipos";

interface EventoDoAgente { projeto: string; tipo: "inicio" | "texto" | "ferramenta" | "arquivo" | "erro"; texto?: string; nome?: string; detalhe?: string; arquivo?: string; modelo?: string | null; sessao?: string | null }
interface RespostaDoAgente { texto: string; sessao: string | null; modelo: string | null; arquivos: string[] }

const SUGESTOES = [
  "Monte uma primeira versão seguindo o briefing",
  "Corta os 3 primeiros segundos",
  "Põe legendas animadas no estilo das referências",
  "Troca a música e abaixa o volume no fim",
];

const NOME_DA_FERRAMENTA: Record<string, string> = {
  Read: "Lendo", Edit: "Editando", MultiEdit: "Editando", Write: "Escrevendo", Glob: "Procurando", Grep: "Procurando",
  Bash: "Rodando", Comando: "Rodando", TodoWrite: "Planejando",
};

export function chaveDaSessao(ia: IaDoVideo) { return `${ia.cli}:${ia.conta ?? ""}`; }

function lerConversa(texto: string | null): MensagemDoChat[] {
  if (!texto) return [];
  return texto.split("\n").flatMap((l) => { try { const m = JSON.parse(l); return m && typeof m.texto === "string" ? [m as MensagemDoChat] : []; } catch { return []; } });
}

export function ChatDoVideo({ projeto, acoes, contexto, autor, bloqueadoPor, onRodando }: {
  projeto: string;
  acoes: AcoesDoProjeto;
  /** Agulha e clipe selecionado, para "corta este clipe" funcionar. */
  contexto: () => string;
  autor?: string;
  /** Numa sessão a dois: quem está com o agente agora (ninguém mais pede até terminar). */
  bloqueadoPor?: string | null;
  onRodando: (rodando: boolean) => void;
}) {
  const { estado } = acoes;
  const ia: IaDoVideo = estado.projeto?.ia ?? { cli: "claude" };
  const [mensagens, setMensagens] = useState<MensagemDoChat[]>([]);
  const [texto, setTexto] = useState("");
  const [rodando, setRodando] = useState(false);
  const [andamento, setAndamento] = useState<string[]>([]);
  const [modeloRodando, setModeloRodando] = useState<string | undefined>();
  const fim = useRef<HTMLDivElement>(null);
  const recarga = useRef<number | undefined>(undefined);
  const acoesAtuais = useRef(acoes);
  acoesAtuais.current = acoes;

  useEffect(() => {
    let vivo = true;
    void disco.lerArquivo(projeto, "conversa.jsonl").then((t) => { if (vivo) setMensagens(lerConversa(t)); }).catch(() => undefined);
    void invoke<boolean>("videos_agente_rodando", { projeto }).then((r) => { if (vivo && r) { setRodando(true); setAndamento(["O agente ainda está trabalhando num pedido anterior…"]); } }).catch(() => undefined);
    return () => { vivo = false; };
  }, [projeto]);

  useEffect(() => { fim.current?.scrollIntoView?.({ block: "end" }); }, [mensagens, andamento]);
  useEffect(() => { onRodando(rodando); }, [rodando, onRodando]);

  // Eventos só existem enquanto um pedido roda: nada fica escutando à toa.
  useEffect(() => {
    if (!rodando) return;
    let parar: (() => void) | undefined;
    let vivo = true;
    void import("@tauri-apps/api/event").then(({ listen }) => listen<EventoDoAgente>("video-agente", ({ payload: e }) => {
      if (e.projeto !== projeto) return;
      if (e.tipo === "inicio" && e.modelo) setModeloRodando(e.modelo);
      else if (e.tipo === "texto" && e.texto) setAndamento((a) => [...a.slice(-30), e.texto!]);
      else if (e.tipo === "ferramenta") setAndamento((a) => [...a.slice(-30), `${NOME_DA_FERRAMENTA[e.nome ?? ""] ?? e.nome} ${e.detalhe ?? ""}`.trim()]);
      else if (e.tipo === "arquivo") {
        // Várias edições seguidas: uma recarga só.
        window.clearTimeout(recarga.current);
        recarga.current = window.setTimeout(() => void acoesAtuais.current.recarregar(), 350);
      }
    })).then((p) => { if (vivo) parar = p; else p(); }).catch(() => undefined);
    return () => { vivo = false; parar?.(); window.clearTimeout(recarga.current); };
  }, [rodando, projeto]);

  const anexar = useCallback(async (m: MensagemDoChat) => {
    setMensagens((l) => [...l, m]);
    await disco.anexarNaConversa(projeto, JSON.stringify(m)).catch(() => undefined);
  }, [projeto]);

  const enviar = useCallback(async (pedido: string) => {
    const limpo = pedido.trim();
    if (!limpo || rodando || bloqueadoPor) return;
    setTexto("");
    setRodando(true);
    setAndamento([]);
    await anexar({ de: "eu", texto: limpo, quando: Date.now(), autor });
    await acoes.guardarVersao(`Antes do pedido: ${limpo.slice(0, 80)}`);
    const sessoes = estado.projeto?.sessoes ?? {};
    const chave = chaveDaSessao(ia);
    try {
      const r = await invoke<RespostaDoAgente>("videos_agente", { pedido: {
        projeto, cli: ia.cli, conta: ia.conta ?? null, modelo: ia.modelo ?? null, sessao: sessoes[chave] ?? null,
        texto: `${limpo}\n\n${contexto()}`,
      } });
      if (r.modelo) setModeloRodando(r.modelo);
      if (r.sessao && r.sessao !== sessoes[chave]) await acoes.mudarProjeto({ sessoes: { ...sessoes, [chave]: r.sessao } });
      await anexar({ de: "ia", texto: r.texto, quando: Date.now(), cli: ia.cli, modelo: r.modelo ?? ia.modelo, arquivos: r.arquivos });
    } catch (e) {
      const erro = e instanceof Error ? e.message : String(e);
      // Sessão que a CLI não acha mais (conta trocada, transcript apagado): a próxima começa outra.
      if (/no conversation found|session.*not found|não encontr/i.test(erro) && sessoes[chave]) {
        const { [chave]: _, ...resto } = sessoes;
        await acoes.mudarProjeto({ sessoes: resto });
        await anexar({ de: "sistema", texto: "A conversa anterior com a IA não foi encontrada nesta conta. Mande de novo: a próxima começa uma conversa nova (o vídeo e o histórico continuam).", quando: Date.now(), erro: true });
      } else {
        await anexar({ de: "sistema", texto: erro === "Cancelado." ? "Pedido cancelado. O que o agente já tinha mudado continua; use Desfazer para voltar." : erro, quando: Date.now(), erro: erro !== "Cancelado." });
      }
    } finally {
      setRodando(false);
      setAndamento([]);
      await acoes.recarregar();
    }
  }, [acoes, anexar, autor, bloqueadoPor, contexto, estado.projeto?.sessoes, ia, projeto, rodando]);

  return (
    <div className="video-chat">
      <div className="video-chat-topo">
        <ChipDaIa ia={ia} modeloRodando={modeloRodando} desligado={rodando} onTrocar={(nova) => void acoes.mudarProjeto({ ia: nova })} />
        {bloqueadoPor && <span className="video-trava" title="Só um agente edita por vez">{bloqueadoPor} está com o agente</span>}
      </div>
      <div className="video-chat-mensagens" aria-live="polite">
        {mensagens.length === 0 && !rodando && (
          <div className="video-chat-vazio">
            <p>Peça o que quiser: o agente edita a composição e o preview atualiza sozinho.</p>
            <div className="video-sugestoes">
              {SUGESTOES.map((s) => <button key={s} type="button" className="video-sugestao" disabled={!!bloqueadoPor} onClick={() => void enviar(s)}>{s}</button>)}
            </div>
          </div>
        )}
        {mensagens.map((m, i) => (
          <div key={`${m.quando}-${i}`} className={`video-balao is-${m.de}${m.erro ? " is-erro" : ""}`}>
            {m.autor && m.de === "eu" && <small className="video-balao-autor">{m.autor}</small>}
            {m.de === "ia" ? <div className="video-md"><ReactMarkdown>{m.texto}</ReactMarkdown></div> : <p>{m.texto}</p>}
            {m.de === "ia" && (m.arquivos?.length || m.modelo) ? (
              <small className="video-balao-rodape">{m.cli ? nomeDaCli(m.cli) : ""}{m.modelo ? ` · ${m.modelo}` : ""}{m.arquivos?.length ? ` · mexeu em ${m.arquivos.join(", ")}` : ""}</small>
            ) : null}
          </div>
        ))}
        {rodando && (
          <div className="video-andamento" role="status">
            <div className="video-andamento-topo"><LoaderIcon size={14} className="animate-spin" aria-hidden="true" /><span>{nomeDaCli(ia.cli)} editando…</span>
              <Button size="sm" variant="ghost" onClick={() => void invoke("videos_agente_cancelar", { projeto })}>Parar</Button>
            </div>
            {andamento.slice(-6).map((a, i) => <p key={i} className="video-andamento-linha">{a}</p>)}
          </div>
        )}
        <div ref={fim} />
      </div>
      <form className="video-chat-campo" onSubmit={(e) => { e.preventDefault(); void enviar(texto); }}>
        <textarea className="field" rows={2} value={texto} placeholder={bloqueadoPor ? `Espere ${bloqueadoPor} terminar com o agente…` : "Ex.: corta os 3 primeiros segundos, legenda animada estilo X, troca a música…"}
          disabled={rodando || !!bloqueadoPor} onChange={(e) => setTexto(e.target.value)} aria-label="Pedido para o agente"
          onKeyDown={(e) => { e.stopPropagation(); if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); void enviar(texto); } }} />
        <Button type="submit" size="sm" variant="primary" disabled={rodando || !texto.trim() || !!bloqueadoPor} aria-label="Enviar"><SendIcon size={14} /></Button>
      </form>
    </div>
  );
}
