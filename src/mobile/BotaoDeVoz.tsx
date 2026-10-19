import { useCallback, useEffect, useRef, useState } from "react";
import { CloseIcon, LoaderIcon, MicIcon, StopIcon } from "../components/ui/PixelIcon";

/*
 * Fork: voz no chat (só no app do desktop). Clique grava, clique de novo transcreve com o
 * whisper.cpp local (`src-tauri/src/voz.rs`) e o texto entra no campo, sem enviar — dá para
 * revisar antes. No navegador do celular não há Rust por trás, então o botão nem aparece.
 */

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

function relogio(ms: number): string {
  const s = Math.floor(ms / 1000);
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}

/** Junta o ditado ao que já está escrito, com um espaço entre os dois. */
export function juntarDitado(atual: string, ditado: string): string {
  const limpo = ditado.trim();
  if (!limpo) return atual;
  if (!atual.trim()) return limpo;
  return /\s$/.test(atual) ? atual + limpo : `${atual} ${limpo}`;
}

export function BotaoDeVoz({ onDitado, travado }: { onDitado: (texto: string) => void; travado: boolean }) {
  const [fase, setFase] = useState<Fase>("parado");
  const [erro, setErro] = useState<string | null>(null);
  const [progresso, setProgresso] = useState(0);
  const [inicio, setInicio] = useState(0);
  const [agora, setAgora] = useState(0);
  const [previa, setPrevia] = useState("");
  const montado = useRef(true);

  useEffect(() => () => {
    montado.current = false;
    // Saiu da conversa no meio da gravação: solta o microfone.
    void chamar("voz_cancelar").catch(() => undefined);
  }, []);

  useEffect(() => {
    if (fase !== "gravando") return;
    const id = window.setInterval(() => setAgora(Date.now()), 250);
    return () => window.clearInterval(id);
  }, [fase]);

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
      setInicio(Date.now());
      setAgora(Date.now());
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

  const rotulo = fase === "gravando" ? "Parar e transcrever (Enter; Esc descarta)"
    : fase === "transcrevendo" ? "Transcrevendo…"
    : fase === "baixando" ? "Baixando o modelo de voz…"
    : "Falar (transcrição neste Mac)";

  return (
    <span className="voz">
      {fase === "gravando" && (
        <>
          <span className="voz-tempo" aria-live="polite"><span className="voz-ponto" aria-hidden />{relogio(agora - inicio)}</span>
          <button type="button" className="composer-botao" title="Descartar a gravação" aria-label="Descartar a gravação" onClick={cancelar}>
            <CloseIcon aria-hidden className="h-4 w-4" />
          </button>
        </>
      )}
      <button type="button" className={`composer-botao${fase === "gravando" ? " voz-gravando" : ""}`}
        title={rotulo} aria-label={rotulo} aria-pressed={fase === "gravando"}
        disabled={travado && fase !== "gravando" || fase === "transcrevendo" || fase === "baixando"}
        onClick={() => void clicar()}>
        {fase === "transcrevendo" || fase === "baixando" ? <LoaderIcon aria-hidden className="anexo-girando h-4 w-4" />
          : fase === "gravando" ? <StopIcon aria-hidden className="h-4 w-4" />
          : <MicIcon aria-hidden className="h-4 w-4" />}
      </button>
      {(fase === "gravando" || fase === "transcrevendo") && (
        <span className="voz-balao voz-previa" aria-live="polite">
          <span className={previa ? undefined : "voz-previa-espera"}>
            {fase === "transcrevendo" ? (previa || "Transcrevendo…") : previa || "Pode falar…"}
          </span>
          {fase === "transcrevendo" && <span className="voz-previa-espera">Corrigindo a transcrição…</span>}
        </span>
      )}
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
