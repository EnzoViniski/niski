// Cópia, paste e SIGINT usam a mesma regra do terminal desktop.
import { handleTerminalKey } from "../features/terminal/keyBindings";
import { useEffect, useRef, useState } from "react";
import { Terminal } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";
import { ApiError, request } from "../mobile/api";
import { FONTE_MONO } from "../lib/fontes";

interface Saida {
  live: boolean;
  data: string;
  from_seq: number;
  next_seq: number;
  cols: number;
  rows: number;
  /** O servidor recomeçou de outro ponto (primeira leitura ou o buffer andou): limpa antes. */
  reset?: boolean;
}

/** Ritmo da leitura: rápido logo depois de chegar saída, devagar com o terminal parado. */
const RAPIDO_MS = 120;
const DEVAGAR_MS = 900;
const OCULTO_MS = 3_000;

/**
 * Respostas que o xterm gera sozinho quando a CLI pergunta algo ao terminal (posição do cursor,
 * tipo de terminal, cores, foco). Quem responde a sessão é o xterm do PC ou o engine; se a página
 * também respondesse, a resposta chegaria em dobro e apareceria como texto digitado dentro da CLI.
 */
const RESPOSTAS_AUTOMATICAS = [
  /\x1b\[\??[\d;]*[cnR]/g, // DA1, DSR, posição do cursor
  /\x1b\[>[\d;]*c/g, // DA2
  /\x1b\][\d]+;[^\x07\x1b]*(?:\x07|\x1b\\)/g, // cores (OSC 10/11/4…)
  /\x1bP[\s\S]*?\x1b\\/g, // DCS (DECRQSS)
  /\x1b\[[IO]/g, // foco
];

export function semRespostasAutomaticas(dados: string): string {
  return RESPOSTAS_AUTOMATICAS.reduce((resto, padrao) => resto.replace(padrao, ""), dados);
}

/** Fonte que faz as `cols` da PTY caberem na largura disponível — a página não redimensiona a PTY
 *  (isso bagunçaria o terminal do PC); quem se ajusta é a letra. */
function fonteQueCabe(largura: number, cols: number): number {
  if (!largura || !cols) return 13;
  return Math.max(8, Math.min(15, Math.floor((largura - 16) / (cols * 0.6))));
}

/**
 * Um terminal do PC, ao vivo: a mesma saída que o app desenha.
 *
 * Dá para digitar nele (clique e digite): Esc interrompe, setas e Enter respondem os menus da CLI,
 * Ctrl+C para um `npm run dev`. Tudo que se digita vai para a mesma PTY que o PC está mostrando.
 *
 * `saida`/`teclas`: as rotas do engine — `/conversas/{id}/terminal` e `/teclas` para o terminal de
 * uma conversa, `/terminais/{id}/saida` e `/teclas` para um terminal sem agente.
 */
export default function TerminalEspelho({ saida: rotaSaida, teclas: rotaTeclas, rotulo = "Terminal ao vivo — clique e digite como no PC (Esc interrompe o agente)" }: {
  saida: string;
  teclas: string;
  rotulo?: string;
}) {
  const host = useRef<HTMLDivElement>(null);
  const [estado, setEstado] = useState<"carregando" | "ao-vivo" | "sem-sessao" | "erro">("carregando");
  const [erro, setErro] = useState<string | null>(null);

  useEffect(() => {
    const elemento = host.current;
    if (!elemento) return;
    let cancelado = false;
    let timer: number | undefined;
    let seq = 0;
    let colsDaPty = 0;
    const terminal = new Terminal({
      convertEol: true,
      cursorBlink: true,
      fontFamily: FONTE_MONO,
      fontSize: 13,
      scrollback: 10_000,
      theme: { background: "#0e0e14", foreground: "#e2e2ec", cursor: "#f97316", selectionBackground: "#f973164d" },
    });
    terminal.open(elemento);

    const ajustarFonte = () => {
      const tamanho = fonteQueCabe(elemento.clientWidth, colsDaPty || terminal.cols);
      if (terminal.options.fontSize !== tamanho) terminal.options.fontSize = tamanho;
    };
    const observador = new ResizeObserver(ajustarFonte);
    observador.observe(elemento);

    // Teclas em lote: digitar rápido vira uma requisição a cada poucos ms, não uma por letra.
    let fila = "";
    let envio: number | undefined;
    const digitar = terminal.onData((dados) => {
      const limpo = semRespostasAutomaticas(dados);
      if (!limpo) return;
      fila += limpo;
      window.clearTimeout(envio);
      envio = window.setTimeout(() => {
        const lote = fila;
        fila = "";
        void request(rotaTeclas, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ dados: lote }),
        }).catch((motivo) => setErro(String(motivo)));
      }, 12);
    });

    terminal.attachCustomKeyEventHandler((evento) => handleTerminalKey(evento, {
      hasSelection: () => terminal.hasSelection(),
      copySelection: () => { void navigator.clipboard.writeText(terminal.getSelection()).catch(() => undefined); },
      write: (dados) => { void request(rotaTeclas, { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ dados }) }).catch(() => setErro("Não foi possível enviar a tecla.")); },
    }));

    async function ler() {
      if (cancelado) return;
      let proximo = DEVAGAR_MS;
      if (document.hidden) {
        proximo = OCULTO_MS;
      } else {
        try {
          const saida = await request<Saida>(`${rotaSaida}?desde=${seq}`);
          if (cancelado) return;
          if (!saida.live) {
            setEstado("sem-sessao");
          } else {
            if (saida.reset) terminal.reset();
            if (saida.cols && saida.rows && (saida.cols !== terminal.cols || saida.rows !== terminal.rows)) {
              terminal.resize(saida.cols, saida.rows);
            }
            if (saida.cols !== colsDaPty) { colsDaPty = saida.cols; ajustarFonte(); }
            if (saida.data) { terminal.write(saida.data); proximo = RAPIDO_MS; }
            seq = saida.next_seq;
            setEstado("ao-vivo");
            setErro(null);
          }
        } catch (motivo) {
          // Terminal sem agente fechado no PC: a rota responde 404, que aqui é "acabou", não erro.
          if (motivo instanceof ApiError && motivo.status === 404) {
            setEstado("sem-sessao");
            return;
          }
          setEstado((atual) => (atual === "carregando" ? "erro" : atual));
          setErro(String(motivo));
          proximo = DEVAGAR_MS * 2;
        }
      }
      if (!cancelado) timer = window.setTimeout(() => void ler(), proximo);
    }
    void ler();

    return () => {
      cancelado = true;
      window.clearTimeout(timer);
      window.clearTimeout(envio);
      digitar.dispose();
      observador.disconnect();
      terminal.dispose();
    };
  }, [rotaSaida, rotaTeclas]);

  return (
    <div className="w-terminal">
      <div className="w-terminal-barra">
        <span className={`m-dot ${estado === "ao-vivo" ? "m-dot-ativo" : estado === "erro" ? "m-dot-erro" : "m-dot-parado"}`} aria-hidden />
        <span>
          {estado === "ao-vivo" ? rotulo
            : estado === "sem-sessao" ? "Este terminal não está mais aberto no PC."
            : estado === "erro" ? "Não consegui ler o terminal."
            : "Conectando ao terminal…"}
        </span>
        {erro && estado === "ao-vivo" && <span className="w-terminal-erro">{erro}</span>}
      </div>
      <div ref={host} className="w-terminal-tela" aria-label="Terminal" />
    </div>
  );
}
