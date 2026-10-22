// Fork (Copiloto): a janela do copiloto — a conversa normal do app (áudio, anexos, troca de modelo,
// conta e esforço, aprovações, terminal por baixo) numa janela flutuante ou presa à direita.
// Mostra os agentes que ele criou e abre o fluxo deles no painel.
import { useEffect, useMemo, useRef, useState, type PointerEvent as PointerReact } from "react";
import { MarcaDoProduto } from "../../components/ui/MarcaDoProduto";
import { AbaDeAgente } from "../conversa/AbaDeAgente";
import { TerminalPane } from "../terminal/TerminalPane";
import { listTerminalSessions } from "../terminal/terminalService";
import { useSession } from "../terminal/sessionProviders";
import { pedirAbrirConversa, pedirAbrirFluxo } from "../orquestracao/abrirFluxo";
import { obterFluxo, type NoAoVivo } from "../orquestracao/orquestracaoService";
import type { WorkspaceTab } from "../../types/workspace";
import {
  PROJETO_DO_COPILOTO, janelaPadrao, mudarCopiloto, novaConversaDoCopiloto, useCopiloto, type Retangulo,
} from "./copiloto";
import "./copiloto.css";

async function pastaDoUsuario(): Promise<string> {
  try { const { homeDir } = await import("@tauri-apps/api/path"); return await homeDir(); } catch { return "/"; }
}

function dentro(r: Retangulo): Retangulo {
  const w = Math.min(Math.max(340, r.w), window.innerWidth - 16), h = Math.min(Math.max(360, r.h), window.innerHeight - 16);
  return { w, h, x: Math.min(Math.max(8, r.x), window.innerWidth - w - 8), y: Math.min(Math.max(8, r.y), window.innerHeight - h - 8) };
}

/** Agentes que o copiloto criou (relidos a cada 4 s com a janela aberta). */
function useAgentesDoCopiloto(conversa: string | null) {
  const [agentes, setAgentes] = useState<NoAoVivo[]>([]);
  useEffect(() => {
    if (!conversa) { setAgentes([]); return; }
    let vivo = true;
    // Fork (desempenho): lista igual não troca o estado (a janela tem a conversa inteira dentro e
    // redesenhava a cada 4 s) e, com a janela escondida, não pergunta.
    let anterior = "";
    const ler = () => {
      if (document.hidden) return;
      void obterFluxo(conversa).then((f) => {
        const lidos = f.agentes ?? [];
        const assinatura = JSON.stringify(lidos);
        if (vivo && assinatura !== anterior) { anterior = assinatura; setAgentes(lidos); }
      }).catch(() => undefined);
    };
    ler();
    const timer = window.setInterval(ler, 4_000);
    return () => { vivo = false; window.clearInterval(timer); };
  }, [conversa]);
  return agentes;
}

export function JanelaDoCopiloto() {
  const c = useCopiloto();
  const [pastaPadrao, setPastaPadrao] = useState("");
  const [rodada, setRodada] = useState(0);
  const [verAgentes, setVerAgentes] = useState(false);
  const [arrasto, setArrasto] = useState<Retangulo | null>(null);
  const inicio = useRef<{ x: number; y: number; r: Retangulo; tipo: "mover" | "tamanho" | "largura" } | null>(null);
  const sessao = useSession(c.sessao || undefined);
  const conversa = sessao?.conversation_id ?? null;
  const agentes = useAgentesDoCopiloto(c.aberto ? conversa : null);
  const vistos = useRef(new Set<string>());

  useEffect(() => { if (!c.pasta) void pastaDoUsuario().then(setPastaPadrao); }, [c.pasta]);

  // Agente novo do copiloto: o fluxo ao vivo abre no painel (uma vez por conversa).
  useEffect(() => {
    if (!conversa || !agentes.length || !c.abrirAgentesNoPainel || vistos.current.has(conversa)) return;
    vistos.current.add(conversa);
    pedirAbrirFluxo(conversa, "Fluxo · Copiloto");
  }, [agentes.length, c.abrirAgentesNoPainel, conversa]);

  // Arrastar a janela, o canto (tamanho) ou a borda (largura presa à direita).
  useEffect(() => {
    const mover = (e: PointerEvent) => {
      const s = inicio.current;
      if (!s) return;
      const dx = e.clientX - s.x, dy = e.clientY - s.y;
      if (s.tipo === "mover") setArrasto(dentro({ ...s.r, x: s.r.x + dx, y: s.r.y + dy }));
      else if (s.tipo === "tamanho") setArrasto(dentro({ ...s.r, w: s.r.w + dx, h: s.r.h + dy }));
      else setArrasto({ ...s.r, w: Math.min(900, Math.max(340, s.r.w - dx)) });
    };
    const soltar = () => {
      const s = inicio.current;
      inicio.current = null;
      document.body.classList.remove("copiloto-arrastando");
      setArrasto((r) => {
        if (r && s) mudarCopiloto(s.tipo === "largura" ? { larguraDireita: r.w } : { janela: r });
        return null;
      });
    };
    window.addEventListener("pointermove", mover);
    window.addEventListener("pointerup", soltar);
    return () => { window.removeEventListener("pointermove", mover); window.removeEventListener("pointerup", soltar); };
  }, []);

  const pasta = c.pasta || pastaPadrao;
  const tab = useMemo<WorkspaceTab>(() => ({
    id: `copiloto-${rodada}`, kind: "agent", title: "Copiloto",
    ...(c.sessao ? { resourceId: c.sessao } : {}), ...(c.provider ? { provider: c.provider } : {}),
  }), [c.provider, c.sessao, rodada]);

  if (!c.aberto) return null;
  const trabalhando = sessao?.state === "working";
  const janela = arrasto && inicio.current?.tipo !== "largura" ? arrasto : dentro(c.janela ?? janelaPadrao());
  const largura = arrasto && inicio.current?.tipo === "largura" ? arrasto.w : c.larguraDireita;
  const comecar = (e: PointerReact, tipo: "mover" | "tamanho" | "largura") => {
    if (e.button !== 0 || (tipo === "mover" && (e.target as HTMLElement).closest("button"))) return;
    e.preventDefault();
    document.body.classList.add("copiloto-arrastando");
    inicio.current = { x: e.clientX, y: e.clientY, tipo, r: tipo === "largura" ? { x: 0, y: 0, w: largura, h: 0 } : janela };
  };
  const estilo = c.modo === "direita" ? { width: largura } : { left: janela.x, top: janela.y, width: janela.w, height: janela.h };

  const escolherPasta = async () => {
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const escolhida = await open({ directory: true, multiple: false, title: "Pasta em que o copiloto trabalha" });
      if (typeof escolhida === "string") { void novaConversaDoCopiloto({ pasta: escolhida }); setRodada((r) => r + 1); }
    } catch { /* sem diálogo (fora do app) */ }
  };
  const abrirAgente = async (no: NoAoVivo) => {
    const s = (await listTerminalSessions().catch(() => [])).find((x) => x.conversation_id === no.id);
    if (s) pedirAbrirConversa(s.id, no.titulo ?? "Agente do copiloto");
    else if (conversa) pedirAbrirFluxo(conversa, "Fluxo · Copiloto");
  };

  return (
    <aside className={`copiloto copiloto-${c.modo}`} style={estilo} aria-label="Copiloto">
      {c.modo === "direita" && <div className="copiloto-borda" onPointerDown={(e) => comecar(e, "largura")} title="Arraste para mudar a largura" />}
      <header className="copiloto-topo" onPointerDown={c.modo === "flutuante" ? (e) => comecar(e, "mover") : undefined}>
        <MarcaDoProduto size={18} aria-hidden />
        <strong className="flex-1 truncate">Copiloto</strong>
        {agentes.length > 0 && (
          <button type="button" className="copiloto-mini copiloto-agentes" aria-expanded={verAgentes} onClick={() => setVerAgentes((v) => !v)}
            title="Agentes que o copiloto criou">{agentes.length} agente{agentes.length === 1 ? "" : "s"}</button>
        )}
        <button type="button" className="copiloto-mini" title={`Pasta: ${pasta || "…"} (trocar)`} onClick={() => void escolherPasta()}>📁</button>
        <button type="button" className="copiloto-mini" disabled={trabalhando || !c.sessao}
          title={trabalhando ? "Espere o copiloto terminar" : "Nova conversa (a atual fica no Histórico)"}
          onClick={() => { void novaConversaDoCopiloto(); setRodada((r) => r + 1); }}>＋</button>
        <button type="button" className="copiloto-mini" title={c.modo === "direita" ? "Soltar como janela flutuante" : "Prender à direita"}
          onClick={() => mudarCopiloto({ modo: c.modo === "direita" ? "flutuante" : "direita" })}>{c.modo === "direita" ? "⧉" : "⇥"}</button>
        <button type="button" className="copiloto-mini" aria-label="Fechar o copiloto (a conversa continua)" onClick={() => mudarCopiloto({ aberto: false })}>×</button>
      </header>
      {verAgentes && agentes.length > 0 && (
        <div className="copiloto-lista-agentes">
          {agentes.map((a) => (
            <button key={a.id} type="button" className="copiloto-agente" onClick={() => void abrirAgente(a)} title="Abrir a conversa no painel">
              <span className={`copiloto-estado is-${a.estado}`} aria-hidden />
              <span className="min-w-0 flex-1 truncate">{a.titulo ?? a.tarefa ?? a.id}</span>
              <small>{[a.plataforma, a.conta].filter(Boolean).join(" · ")}</small>
            </button>
          ))}
          {conversa && <button type="button" className="copiloto-agente justify-center text-accent" onClick={() => pedirAbrirFluxo(conversa, "Fluxo · Copiloto")}>Ver o fluxo de todos no painel</button>}
        </div>
      )}
      {!c.sessao && (
        <p className="copiloto-dica">
          Escolha o agente, a conta e o modelo. O copiloto já abre com as ferramentas do app: cria e comanda agentes de qualquer conta,
          cria e organiza suas tarefas e trabalha na pasta {pasta ? <code>{pasta}</code> : "do usuário"}.
        </p>
      )}
      <div className="copiloto-corpo">
        {pasta ? (
          <AbaDeAgente agentView="conversa" sessionId={c.sessao || null}>
            <TerminalPane key={`${rodada}-${pasta}`} projectId={PROJETO_DO_COPILOTO} projectPath={pasta} paneId="copiloto" tab={tab}
              onSessionCreated={(sessionId, _titulo, provider) => mudarCopiloto({ sessao: sessionId, provider: provider ?? "" })} />
          </AbaDeAgente>
        ) : <p className="p-4 text-sm text-text-muted">Abrindo…</p>}
      </div>
      {c.modo === "flutuante" && <div className="copiloto-canto" onPointerDown={(e) => comecar(e, "tamanho")} title="Arraste para mudar o tamanho" />}
    </aside>
  );
}
