import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { ApiError, actionKey, request, type Timeline, type Conversation } from "../../mobile/api";
import { AjustesDaConversa, type Ajustes } from "../../mobile/AjustesDaConversa";
import { usePoll } from "../../mobile/usePoll";
import { ChatIcon } from "../../components/ui/PixelIcon";
import { EVENTO_ANEXAR, EVENTO_ARRASTANDO, tipoPelaExtensao } from "../../mobile/anexos";
import { ConversationDetail } from "../../mobile/MobileApp";
import { listTerminalSessions } from "../terminal/terminalService";
import { useSessionProvider } from "../terminal/sessionProviders";
import { temConversa } from "../../mobile/api";
import type { AgentView } from "../../types/settings";
import "../../mobile/mobile.css";
import "../../web/web.css";

/**
 * Aba de agente no modo escolhido em Configurações → Aparência → Agentes.
 *
 * O terminal fica **sempre montado**, por baixo da conversa: é ele que roda o failover de conta,
 * responde as perguntas que o ConPTY faz ao abrir e mantém o tamanho da PTY. Desmontar o xterm no
 * modo Conversa quebraria os três. A conversa é uma camada por cima, e "Terminal" só a esconde.
 */
export function AbaDeAgente({
  agentView,
  sessionId,
  children,
}: {
  agentView: AgentView;
  sessionId: string | null;
  /** O `TerminalPane` da aba. */
  children: ReactNode;
}) {
  const [verTerminal, setVerTerminal] = useState(agentView === "terminal");
  useEffect(() => setVerTerminal(agentView === "terminal"), [agentView]);

  // Sem sessão ainda (escolhendo o agente, abrindo): o terminal mostra o lançador. Agente sem
  // visão de conversa (Cursor) fica sempre no terminal — antes a aba parecia vazia e travada.
  const provider = useSessionProvider(sessionId ?? undefined);
  const conversa = agentView === "conversa" && sessionId !== null && temConversa(provider);
  const chatNaTela = conversa && !verTerminal;

  return (
    <div className="relative flex h-full flex-col">
      {/* Terminal na tela e sessão aberta: a faixa com conta/modelo/esforço (e, no modo Conversa,
          o caminho de volta). Faixa e não botão flutuante: nada fica por cima do texto da CLI. */}
      {sessionId !== null && !chatNaTela && (
        <FaixaDoTerminal sessionId={sessionId}>
          {conversa && (
            <button type="button" onClick={() => setVerTerminal(false)} className="btn btn-primary min-h-0 px-2 py-1 text-[11px]">
              <ChatIcon aria-hidden className="h-3.5 w-3.5" />Ver conversa
            </button>
          )}
        </FaixaDoTerminal>
      )}
      {/* `invisible`, não desmontado nem `display: none`: o xterm continua com tamanho de verdade
          (a PTY não encolhe) e o failover segue rodando — mas nada dele aparece por trás da
          conversa, e elemento invisível não recebe foco, então nenhuma tecla cai no terminal. */}
      <div className={`min-h-0 flex-1${chatNaTela ? " invisible" : ""}`} aria-hidden={chatNaTela || undefined}>
        {children}
      </div>
      {chatNaTela && <ChatDaSessao sessionId={sessionId} onVerTerminal={() => setVerTerminal(true)} />}
    </div>
  );
}

/** Conversa (id e estado) da sessão desta aba. Relida a cada poucos segundos: o "pronto" (agente
 *  parado) decide se dá para trocar agora. */
function useConversaDaSessao(sessionId: string) {
  const [conversa, setConversa] = useState<Conversation | null>(null);
  usePoll(async () => {
    const sessao = (await listTerminalSessions()).find((item) => item.id === sessionId);
    if (!sessao?.conversation_id) return;
    const dados = await request<Timeline>(`/conversas/${encodeURIComponent(sessao.conversation_id)}/timeline`);
    if (dados.conversation) setConversa(dados.conversation);
  }, () => undefined, [sessionId], 3_000);
  return conversa;
}

/**
 * Conta, modelo e esforço no modo Terminal — os mesmos chips da conversa. Trocar reabre a mesma
 * sessão com `--resume` (a aba continua a mesma), só com o agente parado.
 */
function FaixaDoTerminal({ sessionId, children }: { sessionId: string; children?: ReactNode }) {
  const conversa = useConversaDaSessao(sessionId);
  const [erro, setErro] = useState<string | null>(null);

  async function aplicar(ajustes: Ajustes) {
    if (!conversa?.capabilities) return;
    setErro(null);
    await request(`/conversas/${encodeURIComponent(conversa.id)}/ajustes`, {
      method: "POST",
      headers: { "Content-Type": "application/json", "Idempotency-Key": actionKey(), "If-Match": conversa.capabilities.revision },
      body: JSON.stringify(ajustes),
    }).catch((motivo) => {
      setErro(motivo instanceof Error ? motivo.message : String(motivo));
      throw motivo;
    });
  }

  if (!conversa?.capabilities && !children) return null;
  return (
    <div className="flex shrink-0 flex-wrap items-center gap-1.5 border-b-2 border-border-subtle bg-bg-elevated/90 px-2 py-1">
      {conversa?.capabilities && (
        <AjustesDaConversa conversaId={conversa.id} pronto={Boolean(conversa.capabilities.prompt)} onAplicar={aplicar} />
      )}
      {erro && <span role="alert" className="text-[11px] text-danger">{erro}</span>}
      <span className="flex-1" />
      {children}
    </div>
  );
}

/**
 * Arrastar arquivos do Windows para a conversa. No app o arrastar é do Tauri (caminhos, não
 * `File`) e não chega aos eventos de soltar da página: lê os arquivos aqui e entrega à conversa
 * pelo `EVENTO_ANEXAR` — dali em diante é o mesmo anexo do celular e da web.
 */
function useSoltarArquivos(area: React.RefObject<HTMLDivElement | null>) {
  useEffect(() => {
    let desligar: (() => void) | undefined;
    let desmontado = false;
    const tela = () => area.current?.querySelector(".chat-tela") ?? null;
    const dentro = (posicao: { x: number; y: number }) => {
      const caixa = area.current?.getBoundingClientRect();
      const escala = window.devicePixelRatio || 1;
      const x = posicao.x / escala, y = posicao.y / escala;
      return Boolean(caixa && x >= caixa.left && x <= caixa.right && y >= caixa.top && y <= caixa.bottom);
    };
    void (async () => {
      try {
        const { getCurrentWebviewWindow } = await import("@tauri-apps/api/webviewWindow");
        const parar = await getCurrentWebviewWindow().onDragDropEvent(async (evento) => {
          const dados = evento.payload;
          const alvo = tela();
          if (!alvo) return;
          if (dados.type === "leave") { alvo.dispatchEvent(new CustomEvent(EVENTO_ARRASTANDO, { detail: false })); return; }
          const aqui = dentro(dados.position);
          if (dados.type === "enter" || dados.type === "over") {
            alvo.dispatchEvent(new CustomEvent(EVENTO_ARRASTANDO, { detail: aqui }));
            return;
          }
          alvo.dispatchEvent(new CustomEvent(EVENTO_ARRASTANDO, { detail: false }));
          if (!aqui) return;
          const { readFile } = await import("@tauri-apps/plugin-fs");
          const arquivos: File[] = [];
          for (const caminho of dados.paths) {
            const nome = caminho.split(/[\/]/).pop() || "anexo";
            try {
              arquivos.push(new File([await readFile(caminho)], nome, { type: tipoPelaExtensao(nome) }));
            } catch {
              // Pasta ou arquivo sem acesso: pula — os outros seguem.
            }
          }
          if (arquivos.length > 0) alvo.dispatchEvent(new CustomEvent(EVENTO_ANEXAR, { detail: arquivos }));
        });
        if (desmontado) parar(); else desligar = parar;
      } catch {
        // Fora do Tauri (testes): o arrastar do navegador já é tratado pela própria conversa.
      }
    })();
    return () => { desmontado = true; desligar?.(); };
  }, [area]);
}

function ChatDaSessao({ sessionId, onVerTerminal }: { sessionId: string; onVerTerminal: () => void }) {
  const [conversa, setConversa] = useState<Conversation | null>(null);
  const [erro, setErro] = useState<string | null>(null);
  const area = useRef<HTMLDivElement>(null);
  useSoltarArquivos(area);

  const carregar = useCallback(async () => {
    // Sessão recém-aberta: o engine pode ainda estar subindo o processo (wsl.exe frio leva ~3 s) ou
    // registrando a conversa. Tenta de novo sozinho antes de mostrar erro — antes o primeiro tropeço
    // virava uma tela de erro com botão, e o "tentar de novo" era trabalho do usuário.
    let ultimoErro = "";
    for (let tentativa = 0; tentativa < 5; tentativa += 1) {
      if (tentativa > 0) await new Promise((resolve) => setTimeout(resolve, 800 * tentativa));
      try {
        const sessao = (await listTerminalSessions()).find((item) => item.id === sessionId);
        if (!sessao?.conversation_id) {
          ultimoErro = "Esta aba ainda não tem uma conversa registrada. Mande a primeira mensagem pelo terminal.";
          continue;
        }
        const dados = await request<Timeline>(`/conversas/${encodeURIComponent(sessao.conversation_id)}/timeline`);
        if (!dados.conversation) throw new ApiError("O engine em execução é anterior à visão Conversa. Reinicie o OMNI.", 0);
        setConversa(dados.conversation);
        setErro(null);
        return;
      } catch (motivo) {
        ultimoErro = motivo instanceof Error ? motivo.message : String(motivo);
        // Engine desatualizado não melhora com novas tentativas: explica na hora.
        if ((motivo instanceof ApiError && motivo.status === 0) || /unknown variant|mais antigo/.test(ultimoErro)) break;
      }
    }
    setErro(ultimoErro);
  }, [sessionId]);

  useEffect(() => {
    void carregar();
  }, [carregar]);

  return (
    <div ref={area} className="absolute inset-0 z-20 flex flex-col bg-bg-surface">
      <div className="flex shrink-0 items-center justify-end px-3 pt-2">
        <button type="button" onClick={onVerTerminal} className="btn btn-ghost min-h-0 px-2 py-1 text-[11px]">
          Ver terminal
        </button>
      </div>
      {conversa ? (
        <div className="w-painel-central">
          <ConversationDetail key={conversa.id} conversation={conversa} refresh={carregar} />
        </div>
      ) : (
        <div className="flex flex-1 flex-col items-center justify-center gap-3 p-6 text-center">
          <p className="text-xs text-text-secondary">{erro ?? "Abrindo a conversa…"}</p>
          {erro && (
            <button type="button" onClick={() => void carregar()} className="btn btn-ghost min-h-0 px-2 py-1 text-xs">
              Tentar de novo
            </button>
          )}
        </div>
      )}
    </div>
  );
}
