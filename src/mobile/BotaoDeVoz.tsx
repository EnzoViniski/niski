import { useCallback, useEffect, useRef, useState } from "react";
import { CloseIcon, LoaderIcon, MicIcon } from "../components/ui/PixelIcon";
import { VisualizadorDaVoz } from "./VisualizadorDaVoz";

/*
 * Voz no chat (só no app do desktop). Clique grava, clique de novo transcreve com o
 * whisper.cpp local (`src-tauri/src/voz.rs`) e o texto entra no campo, sem enviar — dá para
 * revisar antes. No navegador do celular não há Rust por trás, então o botão nem aparece.
 * Desde a 0.8.6 a prévia aparece dentro do próprio campo, em cinza (o Composer desenha a partir de
 * `onPrevia`), e o microfone fica à direita da linha do texto com as barrinhas da voz ao lado.
 */

/** O que está sendo ditado, para o Composer mostrar no campo enquanto não vira texto final. */
export interface PreviaDaVoz {
  texto: string;
  /** Parou de gravar e a transcrição final está relendo o áudio inteiro. */
  corrigindo: boolean;
}

interface EstadoDaVoz {
  modelo_pronto: boolean;
  baixando: boolean;
  baixado_bytes: number;
  total_bytes: number;
  gravando: boolean;
}

type Fase = "parado" | "perguntando" | "baixando" | "gravando" | "transcrevendo";

export const VOZ_DISPONIVEL = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

async function chamar<T>(comando: string): Promise<T> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(comando);
}

/** Junta o ditado ao que já está escrito, com um espaço entre os dois. */
export function juntarDitado(atual: string, ditado: string): string {
  const limpo = ditado.trim();
  if (!limpo) return atual;
  if (!atual.trim()) return limpo;
  return /\s$/.test(atual) ? atual + limpo : `${atual} ${limpo}`;
}

interface BotaoDeVozProps {
  onDitado: (texto: string) => void;
  travado: boolean;
  /** Recebe a prévia enquanto grava/corrige e `null` quando não há ditado em andamento. */
  onPrevia?: (previa: PreviaDaVoz | null) => void;
}

export function BotaoDeVoz({ onDitado, travado, onPrevia }: BotaoDeVozProps) {
  const [fase, setFase] = useState<Fase>("parado");
  const [erro, setErro] = useState<string | null>(null);
  const [progresso, setProgresso] = useState(0);
  const [previa, setPrevia] = useState("");
  const montado = useRef(true);
  const avisarPrevia = useRef(onPrevia);
  avisarPrevia.current = onPrevia;

  useEffect(() => () => {
    montado.current = false;
    // Saiu da conversa no meio da gravação: solta o microfone.
    void chamar("voz_cancelar").catch(() => undefined);
  }, []);

  // A prévia vai para o campo do Composer; some (null) quando o texto final entra ou a gravação é descartada.
  useEffect(() => {
    const emAndamento = fase === "gravando" || fase === "transcrevendo";
    avisarPrevia.current?.(emAndamento ? { texto: previa, corrigindo: fase === "transcrevendo" } : null);
  }, [fase, previa]);
  useEffect(() => () => avisarPrevia.current?.(null), []);

  // Prévia ao vivo: uma transcrição por vez dos últimos segundos, enquanto grava. A final, ao
  // parar, relê tudo e corrige o que a prévia errou.
  useEffect(() => {
    // A prévia fica na tela enquanto a final corrige; some quando o texto entra no campo.
    if (fase !== "gravando") { if (fase === "parado") setPrevia(""); return; }
    let vivo = true;
    void (async () => {
      while (vivo) {
        await new Promise((pronto) => window.setTimeout(pronto, 600));
        if (!vivo) break;
        try {
          const texto = await chamar<string>("voz_previa");
          if (vivo && texto.trim()) setPrevia(texto.trim());
        } catch {
          // Prévia é bônus: erro aqui não interrompe a gravação.
        }
      }
    })();
    return () => { vivo = false; };
  }, [fase]);

  // Download do modelo: o progresso sai do tamanho do arquivo parcial.
  useEffect(() => {
    if (fase !== "baixando") return;
    const id = window.setInterval(() => {
      void chamar<EstadoDaVoz>("voz_estado").then((estado) => {
        if (montado.current) setProgresso(estado.total_bytes ? estado.baixado_bytes / estado.total_bytes : 0);
      }).catch(() => undefined);
    }, 700);
    return () => window.clearInterval(id);
  }, [fase]);

  const gravar = useCallback(async () => {
    setErro(null);
    try {
      await chamar("voz_gravar");
      setFase("gravando");
    } catch (e) {
      setErro(String(e));
      setFase("parado");
    }
  }, []);

  async function baixar() {
    setFase("baixando");
    setErro(null);
    try {
      await chamar("voz_baixar_modelo");
      if (montado.current) await gravar();
    } catch (e) {
      if (!montado.current) return;
      setErro(String(e));
      setFase("parado");
    }
  }

  async function clicar() {
    if (fase === "gravando") {
      setFase("transcrevendo");
      try {
        const texto = await chamar<string>("voz_transcrever");
        if (!montado.current) return;
        if (texto.trim()) onDitado(texto);
        else setErro("Não ouvi nada. Fale um pouco mais perto do microfone.");
      } catch (e) {
        if (montado.current) setErro(String(e));
      }
      if (montado.current) setFase("parado");
      return;
    }
    if (fase !== "parado") return;
    setErro(null);
    try {
      const estado = await chamar<EstadoDaVoz>("voz_estado");
      if (estado.baixando) { setFase("baixando"); return; }
      if (!estado.modelo_pronto) { setFase("perguntando"); return; }
    } catch (e) {
      setErro(String(e));
      return;
    }
    await gravar();
  }

  function cancelar() {
    void chamar("voz_cancelar").catch(() => undefined);
    setFase("parado");
  }

  // Gravando: Enter (Return) para e transcreve, Esc descarta. Escuta na captura da janela para o
  // Enter não chegar ao campo do Composer, que enviaria a mensagem.
  const atalhos = useRef({ clicar, cancelar });
  atalhos.current = { clicar, cancelar };
  useEffect(() => {
    if (fase !== "gravando") return;
    function tecla(event: KeyboardEvent) {
      if (event.isComposing) return;
      if (event.key === "Enter" && !event.shiftKey) {
        event.preventDefault();
        event.stopPropagation();
        void atalhos.current.clicar();
      } else if (event.key === "Escape") {
        event.preventDefault();
        event.stopPropagation();
        atalhos.current.cancelar();
      }
    }
    window.addEventListener("keydown", tecla, true);
    return () => window.removeEventListener("keydown", tecla, true);
  }, [fase]);

  const gravando = fase === "gravando";
  const rotulo = gravando ? "Parar e transcrever (Enter; Esc descarta)"
    : fase === "transcrevendo" ? "Transcrevendo…"
    : fase === "baixando" ? "Baixando o modelo de voz…"
    : "Falar (transcrição neste Mac)";

  // Gravando: [descartar] [barrinhas da voz] [⏎ confirmar] [microfone aceso], como no exemplo do Enzo.
  return (
    <span className="voz">
      {gravando && (
        <>
          <button type="button" className="composer-botao voz-descartar" title="Descartar a gravação (Esc)" aria-label="Descartar a gravação" onClick={cancelar}>
            <CloseIcon aria-hidden className="h-3.5 w-3.5" />
          </button>
          <VisualizadorDaVoz ativo />
          <button type="button" className="composer-botao voz-confirmar" title="Confirmar e transcrever (Enter)" aria-label="Confirmar a transcrição" onClick={() => void clicar()}>
            <svg aria-hidden viewBox="0 0 24 24" className="h-4 w-4" fill="none" stroke="currentColor" strokeWidth={2} strokeLinecap="round" strokeLinejoin="round">
              <path d="M20 5v6a4 4 0 0 1-4 4H5" /><path d="m9 10-5 5 5 5" />
            </svg>
          </button>
        </>
      )}
      <button type="button" className={`composer-botao voz-microfone${gravando ? " voz-gravando" : ""}`}
        title={rotulo} aria-label={rotulo} aria-pressed={gravando}
        disabled={travado && !gravando || fase === "transcrevendo" || fase === "baixando"}
        onClick={() => void clicar()}>
        {fase === "transcrevendo" || fase === "baixando" ? <LoaderIcon aria-hidden className="anexo-girando h-4 w-4" />
          : <MicIcon aria-hidden className="h-4 w-4" />}
      </button>
      {fase === "baixando" && <span className="voz-tempo">{Math.round(progresso * 100)}%</span>}
      {fase === "perguntando" && (
        <span className="voz-balao" role="dialog" aria-label="Baixar modelo de voz">
          <span>A voz roda neste Mac. Na primeira vez baixo o modelo Whisper (575 MB, uma vez só).</span>
          <span className="voz-balao-acoes">
            <button type="button" className="voz-balao-sim" onClick={() => void baixar()}>Baixar</button>
            <button type="button" className="voz-balao-nao" onClick={() => setFase("parado")}>Agora não</button>
          </span>
        </span>
      )}
      {erro && fase === "parado" && (
        <span className="voz-balao voz-balao-erro" role="alert">
          <span>{erro}</span>
          <span className="voz-balao-acoes">
            <button type="button" className="voz-balao-nao" onClick={() => setErro(null)}>Fechar</button>
          </span>
        </span>
      )}
    </span>
  );
}
