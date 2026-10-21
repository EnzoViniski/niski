// Fork: Jarvis. Conteúdo da janela flutuante (`src-tauri/src/jarvis/janela.rs`): orbe, transcrição
// ao vivo e controles. Encolhida, vira só o orbe (bolinha). Esc para a fala na hora.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useReducer, useRef, useState, type FormEvent } from "react";
import { OrbeDoJarvis, type Niveis } from "./OrbeDoJarvis";
import { aplicarEvento, estadoInicial, ROTULO_DO_ESTADO, type EventoDoJarvis } from "./estadoDoJarvis";
import { AjustesDoJarvis, type ConfiguracaoDoJarvis } from "./AjustesDoJarvis";
import { prepararAnexos, paraOJarvis, TIPOS_DO_JARVIS, type AnexoNaJanela } from "./anexosDoJarvis";
import "./jarvis.css";

/** Abaixo disso a janela é a bolinha. */
const LARGURA_DA_BOLINHA = 150;

export function JanelaDoJarvis() {
  const [estado, despachar] = useReducer(aplicarEvento, estadoInicial);
  const niveis = useRef<Niveis>({ mic: 0, fala: 0 });
  const [bolinha, setBolinha] = useState(() => window.innerWidth < LARGURA_DA_BOLINHA);
  const [ouvindo, setOuvindo] = useState(true);
  const [texto, setTexto] = useState("");
  const [ajustesAbertos, setAjustesAbertos] = useState(false);
  const [configuracao, setConfiguracao] = useState<ConfiguracaoDoJarvis | null>(null);
  const [anexos, setAnexos] = useState<AnexoNaJanela[]>([]);
  const [enviando, setEnviando] = useState(false);
  const [lendoAnexos, setLendoAnexos] = useState(false);
  const [erroDoCampo, setErroDoCampo] = useState("");
  const envioEmCurso = useRef(false);
  const leituraEmCurso = useRef(false);
  const fim = useRef<HTMLDivElement>(null);

  const iniciar = () => {
    setOuvindo(true);
    despachar({ tipo: "reiniciar" });
    invoke("jarvis_iniciar").catch((erro: unknown) => despachar({ tipo: "erro", msg: String(erro) }));
  };

  useEffect(() => {
    let solto = false;
    let soltar: (() => void) | undefined;
    // Fork: só inicia depois de assinar os eventos; não perde o primeiro aviso/erro da voz.
    void listen<EventoDoJarvis>("jarvis", ({ payload }) => {
      if (payload.tipo === "nivel") niveis.current = { mic: payload.mic, fala: payload.fala };
      else despachar(payload);
    }).then((parar) => {
      if (solto) parar();
      else { soltar = parar; iniciar(); }
    }).catch((erro: unknown) => { if (!solto) despachar({ tipo: "erro", msg: String(erro) }); });
    void invoke<ConfiguracaoDoJarvis>("jarvis_configuracao").then((valor) => { if (!solto && valor) setConfiguracao(valor); }).catch(() => undefined);
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
  const adicionar = async (arquivos: File[]) => {
    if (!arquivos.length || leituraEmCurso.current || envioEmCurso.current) return;
    leituraEmCurso.current = true; setLendoAnexos(true); setErroDoCampo("");
    try {
      const novos = await prepararAnexos(arquivos, anexos);
      setAnexos((atuais) => [...atuais, ...novos]);
    } catch (erro) { setErroDoCampo(erro instanceof Error ? erro.message : String(erro)); }
    finally { leituraEmCurso.current = false; setLendoAnexos(false); }
  };
  const enviar = async (e: FormEvent) => {
    e.preventDefault();
    if ((!texto.trim() && !anexos.length) || envioEmCurso.current || leituraEmCurso.current) return;
    envioEmCurso.current = true; setEnviando(true); setErroDoCampo("");
    try {
      await invoke("jarvis_escrever", { texto, ...(anexos.length ? { anexos: paraOJarvis(anexos) } : {}) });
      setTexto(""); setAnexos([]);
    } catch (erro) { setErroDoCampo(String(erro)); }
    finally { envioEmCurso.current = false; setEnviando(false); }
  };
  const tom = estado.erro ? "erro" : estado.estado;
  const ocupado = estado.estado === "pensando" || estado.estado === "falando";

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
        <button type="button" className="jarvis-icone" aria-label="Ver agentes do Jarvis" title="Ver equipe e tarefas no app" onClick={() => void invoke("jarvis_abrir_fluxo").catch((erro: unknown) => setErroDoCampo(String(erro)))}>
          <svg viewBox="0 0 20 20" width="15" height="15" fill="none" stroke="currentColor" strokeWidth="1.4" aria-hidden><rect x="1" y="7" width="5" height="5" rx="1" /><rect x="14" y="2" width="5" height="5" rx="1" /><rect x="14" y="13" width="5" height="5" rx="1" /><path d="M6 9.5h4V4.5h4M10 9.5v6h4" /></svg>
        </button>
        <button type="button" className="jarvis-icone" aria-label="Ajustar modelo, conta e voz" aria-expanded={ajustesAbertos} title="Modelo, conta e voz" onClick={() => setAjustesAbertos((abertos) => !abertos)}>⚙</button>
        <button type="button" className="jarvis-icone" aria-label="Encolher para bolinha" title="Encolher" onClick={() => trocarBolinha(true)}>–</button>
        <button type="button" className="jarvis-icone" aria-label="Fechar o Jarvis" title="Fechar (⌃⌥J)" onClick={() => void invoke("jarvis_fechar")}>×</button>
      </header>

      {ajustesAbertos && <AjustesDoJarvis ocupado={ocupado} onFechar={() => setAjustesAbertos(false)} onSalvo={setConfiguracao} />}

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
          <p className="jarvis-dica">Fale, escreva ou envie um arquivo. Peça para consultar suas conversas ou coordenar os agentes.</p>
        )}
        {estado.falas.map((fala) => (
          <p key={fala.id} className={`jarvis-fala jarvis-fala-${fala.quem}${fala.cortada ? " is-cortada" : ""}`}>{fala.texto}</p>
        ))}
        {estado.parcial && <p className="jarvis-fala jarvis-fala-voce is-parcial">{estado.parcial}</p>}
        <div ref={fim} />
      </div>

      {estado.ferramenta && <div className="jarvis-ferramenta" role="status">{estado.ferramenta.estado === "iniciada" ? "Executando" : estado.ferramenta.estado === "erro" ? "Falhou" : "Concluído"}: {rotuloDaFerramenta(estado.ferramenta.nome)}</div>}
      {configuracao && <button type="button" className="jarvis-modelo-atual" onClick={() => setAjustesAbertos(true)} title="Trocar modelo, conta ou voz">
        {configuracao.modelos.find((m) => m.id === configuracao.modelo)?.nome ?? configuracao.modelo}
        {configuracao.contas.find((c) => c.id === configuracao.perfil_id)?.nome && ` · ${configuracao.contas.find((c) => c.id === configuracao.perfil_id)?.nome}`}
      </button>}
      {anexos.length > 0 && <ul className="jarvis-anexos" aria-label="Anexos para enviar">{anexos.map((anexo) => <li key={anexo.id}>
        {anexo.mime.startsWith("image/") && <img src={`data:${anexo.mime};base64,${anexo.base64}`} alt="" />}
        <span title={anexo.nome}>{anexo.nome}</span><button type="button" className="jarvis-icone" disabled={enviando || lendoAnexos} aria-label={`Remover ${anexo.nome}`} onClick={() => setAnexos((atuais) => atuais.filter((a) => a.id !== anexo.id))}>×</button>
      </li>)}</ul>}
      {erroDoCampo && <p className="jarvis-erro-do-campo" role="alert">{erroDoCampo}</p>}

      <form className="jarvis-rodape" onSubmit={(e) => void enviar(e)} onDragOver={(e) => { if (e.dataTransfer.types.includes("Files")) e.preventDefault(); }}
        onDrop={(e) => { if (e.dataTransfer.files.length) { e.preventDefault(); void adicionar(Array.from(e.dataTransfer.files)); } }}>
        <button type="button" className={`jarvis-icone jarvis-mic${ouvindo ? "" : " is-mudo"}`} onClick={alternarMicrofone}
          aria-pressed={!ouvindo} aria-label={ouvindo ? "Pausar o microfone" : "Ligar o microfone"} title={ouvindo ? "Pausar o microfone" : "Ligar o microfone"}>
          <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" aria-hidden>
            <rect x="9" y="3" width="6" height="11" rx="3" /><path d="M5 11a7 7 0 0 0 14 0M12 18v3" />
            {!ouvindo && <path d="M4 4l16 16" />}
          </svg>
        </button>
        <label className={`jarvis-icone jarvis-anexar${enviando || lendoAnexos ? " is-desabilitado" : ""}`} title="Anexar imagem, PDF ou texto">
          <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" aria-hidden><path d="m8 12 6-6a3 3 0 0 1 4 4l-8 8a5 5 0 0 1-7-7l9-9M7 13l7-7" /></svg>
          <input type="file" aria-label="Anexar imagem ou documento" accept={TIPOS_DO_JARVIS} multiple disabled={enviando || lendoAnexos} onChange={(e) => { void adicionar(Array.from(e.target.files ?? [])); e.target.value = ""; }} />
        </label>
        <input className="jarvis-campo" value={texto} disabled={enviando} maxLength={32000} onChange={(e) => setTexto(e.target.value)} placeholder={lendoAnexos ? "Preparando anexos…" : "Ou escreva aqui…"} aria-label="Escrever para o Jarvis"
          onPaste={(e) => { if (e.clipboardData.files.length) { e.preventDefault(); void adicionar(Array.from(e.clipboardData.files)); } }} />
        <button type="submit" className="jarvis-icone jarvis-enviar" aria-label="Enviar para o Jarvis" title="Enviar (Enter)" disabled={enviando || lendoAnexos || (!texto.trim() && !anexos.length)}>
          <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden><path d="M12 19V5m-6 6 6-6 6 6" /></svg>
        </button>
        <button type="button" className="jarvis-icone" aria-label="Parar a fala (Esc)" title="Parar a fala (Esc)" onClick={() => void invoke("jarvis_calar")}>■</button>
      </form>
    </div>
  );
}

function rotuloDaFerramenta(nome: string): string {
  const rotulos: Record<string, string> = {
    listar_projetos: "Consultar projetos", listar_conversas: "Consultar conversas", consultar_conversa: "Consultar conversa",
    listar_plataformas_e_contas: "Consultar agentes e contas", criar_agente: "Abrir agente", conferir_arquivos: "Conferir alterações",
    mandar_prompt: "Enviar tarefa", ler_conversa: "Ler resposta do agente", esperar: "Acompanhar tarefa", listar_agentes: "Consultar equipe",
    responder_pedido: "Responder ao agente", parar_agente: "Encerrar agente",
  };
  return rotulos[nome.split("__").pop() ?? nome] ?? "Coordenar agentes";
}
