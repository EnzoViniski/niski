// Fork: Jarvis. Conteúdo da janela flutuante (`src-tauri/src/jarvis/janela.rs`): orbe, transcrição
// ao vivo e controles. Encolhida, vira só o orbe (bolinha). Esc para a fala na hora.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useReducer, useRef, useState, type FormEvent } from "react";
import { OrbeDoJarvis, type Niveis } from "./OrbeDoJarvis";
import { aplicarEvento, estadoInicial, ROTULO_DO_ESTADO, type EventoDoJarvis } from "./estadoDoJarvis";
import "./jarvis.css";

/** Abaixo disso a janela é a bolinha. */
const LARGURA_DA_BOLINHA = 150;

export function JanelaDoJarvis() {
  const [estado, despachar] = useReducer(aplicarEvento, estadoInicial);
  const niveis = useRef<Niveis>({ mic: 0, fala: 0 });
  const [bolinha, setBolinha] = useState(() => window.innerWidth < LARGURA_DA_BOLINHA);
  const [ouvindo, setOuvindo] = useState(true);
  const [texto, setTexto] = useState("");
  const fim = useRef<HTMLDivElement>(null);

  const iniciar = () => {
    despachar({ tipo: "reiniciar" });
    invoke("jarvis_iniciar").catch((erro: unknown) => despachar({ tipo: "erro", msg: String(erro) }));
  };

  useEffect(() => {
    let solto = false;
    let soltar: (() => void) | undefined;
    void listen<EventoDoJarvis>("jarvis", ({ payload }) => {
      if (payload.tipo === "nivel") niveis.current = { mic: payload.mic, fala: payload.fala };
      else despachar(payload);
    }).then((parar) => { if (solto) parar(); else soltar = parar; });
    iniciar();
    return () => { solto = true; soltar?.(); };
  }, []);

  useEffect(() => {
    const aoRedimensionar = () => setBolinha(window.innerWidth < LARGURA_DA_BOLINHA);
    const aoTeclar = (e: KeyboardEvent) => { if (e.key === "Escape") void invoke("jarvis_calar"); };
    window.addEventListener("resize", aoRedimensionar);
    window.addEventListener("keydown", aoTeclar);
    return () => { window.removeEventListener("resize", aoRedimensionar); window.removeEventListener("keydown", aoTeclar); };
  }, []);

  useEffect(() => { fim.current?.scrollIntoView?.({ block: "end" }); }, [estado.falas, estado.parcial]);

  const trocarBolinha = (ligada: boolean) => { void invoke("jarvis_bolinha", { ligada }).catch(() => undefined); };
  const alternarMicrofone = () => {
    const ligado = !ouvindo;
    setOuvindo(ligado);
    void invoke("jarvis_ouvir", { ligado });
  };
  const enviar = (e: FormEvent) => {
    e.preventDefault();
    if (!texto.trim()) return;
    void invoke("jarvis_escrever", { texto });
    setTexto("");
  };
  const tom = estado.erro ? "erro" : estado.estado;

  if (bolinha) {
    return (
      <div className="jarvis-bolinha" data-tauri-drag-region data-estado={tom} onDoubleClick={() => trocarBolinha(false)}
        title={`Jarvis — ${estado.erro ?? ROTULO_DO_ESTADO[estado.estado]} (dois cliques para abrir)`}>
        <OrbeDoJarvis estado={tom} niveis={niveis} tamanho={76} compacto />
        <button type="button" className="jarvis-bolinha-abrir" aria-label="Abrir o Jarvis" onClick={() => trocarBolinha(false)}>⤢</button>
      </div>
    );
  }

  return (
    <div className="jarvis-janela" data-estado={tom}>
      <header className="jarvis-topo" data-tauri-drag-region>
        <span className="jarvis-marca" data-tauri-drag-region>JARVIS</span>
        <span className="jarvis-estado" data-tauri-drag-region role="status">{estado.erro ? "Com problema" : ROTULO_DO_ESTADO[estado.estado]}</span>
        <button type="button" className="jarvis-icone" aria-label="Encolher para bolinha" title="Encolher" onClick={() => trocarBolinha(true)}>–</button>
        <button type="button" className="jarvis-icone" aria-label="Fechar o Jarvis" title="Fechar (⌃⌥J)" onClick={() => void invoke("jarvis_fechar")}>×</button>
      </header>

      <div className="jarvis-palco" data-tauri-drag-region>
        <OrbeDoJarvis estado={tom} niveis={niveis} tamanho={200} />
      </div>

      {estado.erro && (
        <div className="jarvis-erro" role="alert">
          <span>{estado.erro}</span>
          <button type="button" onClick={iniciar}>Tentar de novo</button>
        </div>
      )}
      {estado.aviso && !estado.erro && <div className="jarvis-aviso">{estado.aviso}</div>}

      <div className="jarvis-falas" aria-live="polite">
        {estado.falas.length === 0 && !estado.parcial && (
          <p className="jarvis-dica">Fale normalmente. Interrompa quando quiser, ou diga “Medza, para”.</p>
        )}
        {estado.falas.map((fala) => (
          <p key={fala.id} className={`jarvis-fala jarvis-fala-${fala.quem}${fala.cortada ? " is-cortada" : ""}`}>{fala.texto}</p>
        ))}
        {estado.parcial && <p className="jarvis-fala jarvis-fala-voce is-parcial">{estado.parcial}</p>}
        <div ref={fim} />
      </div>

      <form className="jarvis-rodape" onSubmit={enviar}>
        <button type="button" className={`jarvis-icone jarvis-mic${ouvindo ? "" : " is-mudo"}`} onClick={alternarMicrofone}
          aria-pressed={!ouvindo} aria-label={ouvindo ? "Pausar o microfone" : "Ligar o microfone"} title={ouvindo ? "Pausar o microfone" : "Ligar o microfone"}>
          <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" aria-hidden>
            <rect x="9" y="3" width="6" height="11" rx="3" /><path d="M5 11a7 7 0 0 0 14 0M12 18v3" />
            {!ouvindo && <path d="M4 4l16 16" />}
          </svg>
        </button>
        <input className="jarvis-campo" value={texto} onChange={(e) => setTexto(e.target.value)} placeholder="Ou escreva aqui…" aria-label="Escrever para o Jarvis" />
        <button type="button" className="jarvis-icone" aria-label="Parar a fala (Esc)" title="Parar a fala (Esc)" onClick={() => void invoke("jarvis_calar")}>■</button>
      </form>
    </div>
  );
}
