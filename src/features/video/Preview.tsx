// Fork (Editor de vídeo): preview com o `<hyperframes-player>` oficial (0.8.183).
//
// - A composição roda num iframe com sandbox **opaco** (`sandbox-origin="opaque"`): sem acesso à
//   janela do app nem às APIs do Tauri; só conversa com o player (e com a ponte da edição na mão,
//   `src-tauri/src/videos/ponte_do_editor.js`) por mensagem. O HTML vem do protocolo `medzavideo://`,
//   com CSP fechada (ver `src-tauri/src/videos/protocolo.rs`).
// - Ocupa todo o espaço do meio, no formato do vídeo (medido, não `aspect-ratio`: com altura e largura
//   limitadas ao mesmo tempo o CSS distorce a caixa).
// - Diagnóstico (0.16): se o player não fica pronto em 12 s, avisa na tela, grava no log (só o motivo)
//   e tenta de novo uma vez no modo compatível (iframe com origem própria `medzavideo://`, ainda
//   separado do app). Antes, um preview que não abria ficava só preto.
// - Recarrega quando `versao` muda e volta para o mesmo ponto do tempo.
// - Pausa quando a janela some ou perde o foco (pausa de energia, `src/lib/energia.ts`); nunca toca
//   sozinho (respeita "reduzir movimento"). A agulha anda por `proximoQuadro`, que para na pausa.
import { invoke } from "@tauri-apps/api/core";
import { forwardRef, useEffect, useImperativeHandle, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { cancelarQuadro, proximoQuadro } from "../../lib/energia";
import { carregarEndereco, urlDoProjeto } from "./enderecos";
import type { RelogioDoPreview } from "./relogio";

/** O que o editor usa do elemento (tipos de `@hyperframes/player`). */
interface Player extends HTMLElement {
  play(): void;
  pause(): void;
  seek(t: number): void;
  readonly currentTime: number;
  readonly duration: number;
  readonly paused: boolean;
  readonly ready: boolean;
  readonly iframeElement?: HTMLIFrameElement;
  playbackRate: number;
  muted: boolean;
}

export interface ControleDoPreview {
  tocar(): void;
  pausar(): void;
  alternar(): void;
  irPara(t: number): void;
  /** Anda `n` quadros (negativo volta). */
  quadros(n: number): void;
  /** Manda uma mensagem para a ponte da edição na mão (dentro do iframe). */
  paraPonte(mensagem: Record<string, unknown>): void;
}

/** Mensagem que a ponte manda (ver `ponte_do_editor.js`). */
export interface MensagemDaPonte { tipo: string; ref?: string | null; el?: ElementoNoPreview | null }
export interface ElementoNoPreview {
  ref: string; id: string | null; tag: string;
  rect: { x: number; y: number; w: number; h: number };
  texto: string | null; fonte: number | null; peso: string; cor: string; fundo: string;
  translate: string; scale: string; rotate: string; visivel: boolean;
}

interface Props {
  projeto: string;
  versao: number;
  largura: number;
  altura: number;
  fps: number;
  relogio: RelogioDoPreview;
  onErro?: (mensagem: string | null) => void;
  onPonte?: (m: MensagemDaPonte) => void;
  /** Camada por cima do vídeo (edição na mão). Recebe a escala do vídeo na tela. */
  sobreposicao?: (escala: number) => ReactNode;
}

const ESPERA_MS = 12_000;
type Modo = "opaco" | "compativel";

let carregarPlayer: Promise<unknown> | null = null;

function registrar(detalhe: string) { void invoke("registrar_evento_da_ui", { evento: "video_preview", detalhe }).catch(() => undefined); }

export const Preview = forwardRef<ControleDoPreview, Props>(function Preview({ projeto, versao, largura, altura, fps, relogio, onErro, onPonte, sobreposicao }, ref) {
  const area = useRef<HTMLDivElement>(null);
  const caixa = useRef<HTMLDivElement>(null);
  const player = useRef<Player | null>(null);
  const [pronto, setPronto] = useState(false);
  const [carregado, setCarregado] = useState(false);
  const [modo, setModo] = useState<Modo>("opaco");
  const [problema, setProblema] = useState<string | null>(null);
  const [tentativa, setTentativa] = useState(0);
  const [tamanho, setTamanho] = useState({ w: 0, h: 0 });
  const [url, setUrl] = useState<string | null>(() => urlDoProjeto(projeto, "index.html", versao));
  const tempoGuardado = useRef(0);
  const erroRef = useRef(onErro);
  erroRef.current = onErro;
  const ponteRef = useRef(onPonte);
  ponteRef.current = onPonte;

  // Caixa do vídeo: o maior retângulo no formato do vídeo que cabe no espaço.
  useLayoutEffect(() => {
    const el = area.current;
    if (!el) return;
    const medir = () => {
      const w = el.clientWidth, h = el.clientHeight;
      if (!w || !h) { setTamanho({ w: 0, h: 0 }); return; }
      const proporcao = largura / altura;
      const cabe = w / h > proporcao ? { w: Math.round(h * proporcao), h } : { w, h: Math.round(w / proporcao) };
      setTamanho((t) => (t.w === cabe.w && t.h === cabe.h ? t : cabe));
    };
    medir();
    if (typeof ResizeObserver === "undefined") return;
    const obs = new ResizeObserver(medir);
    obs.observe(el);
    return () => obs.disconnect();
  }, [largura, altura]);

  useEffect(() => {
    carregarPlayer ??= import("@hyperframes/player").catch((e) => { carregarPlayer = null; throw e; });
    let vivo = true;
    void Promise.all([carregarPlayer, carregarEndereco()]).then(() => {
      if (!vivo) return;
      setPronto(true);
      setUrl(urlDoProjeto(projeto, "index.html", versao));
    }).catch((e) => {
      if (!vivo) return;
      setProblema("Não consegui carregar o player do vídeo.");
      registrar(`sem_player ${String(e).slice(0, 80)}`);
      erroRef.current?.("Não consegui abrir o preview.");
    });
    return () => { vivo = false; };
  }, [projeto, versao]);

  // Cria o elemento uma vez e só troca o `src` (recarregar não pisca o quadro inteiro).
  useEffect(() => {
    const alvo = caixa.current;
    if (!pronto || !alvo || !url) return;
    let el = player.current;
    if (!el) {
      el = document.createElement("hyperframes-player") as Player;
      el.setAttribute("disable-click-to-play", "");
      el.setAttribute("low-power-idle", "");
      el.style.width = "100%";
      el.style.height = "100%";
      el.style.display = "block";
      alvo.appendChild(el);
      player.current = el;
    }
    if (modo === "opaco") el.setAttribute("sandbox-origin", "opaque"); else el.removeAttribute("sandbox-origin");
    el.setAttribute("width", String(largura));
    el.setAttribute("height", String(altura));
    tempoGuardado.current = relogio.ler().tempo;
    relogio.mudar({ pronto: false, tocando: false });
    setCarregado(false);
    // "Tentar de novo" muda o endereço (o protocolo ignora a consulta): o player recarrega de verdade.
    el.setAttribute("src", tentativa ? `${url}${url.includes("?") ? "&" : "?"}r=${tentativa}` : url);
  }, [pronto, url, largura, altura, relogio, modo, tentativa]);

  // Diagnóstico: sem `ready` em 12 s, tenta o modo compatível uma vez; depois, avisa com o motivo.
  useEffect(() => {
    if (!pronto || carregado) return;
    const inicio = performance.now();
    const t = window.setTimeout(() => {
      registrar(`sem_ready modo=${modo}`);
      if (modo === "opaco") { setProblema("O preview está demorando. Tentando de outro jeito…"); setModo("compativel"); }
      else { setProblema("O preview não abriu. Confira a internet (o vídeo usa bibliotecas da web) ou peça ao agente para rodar o lint."); erroRef.current?.("O preview não abriu."); }
    }, ESPERA_MS);
    const el = player.current;
    const aoPronto = () => {
      window.clearTimeout(t);
      registrar(`pronto modo=${modo} s=${Math.round((performance.now() - inicio) / 1000)}`);
    };
    el?.addEventListener("ready", aoPronto, { once: true });
    return () => { window.clearTimeout(t); el?.removeEventListener("ready", aoPronto); };
  }, [pronto, carregado, modo, url, tentativa]);

  useEffect(() => () => { player.current?.remove(); player.current = null; }, []);

  // Mensagens da ponte (só do iframe do player).
  useEffect(() => {
    const aoMensagem = (e: MessageEvent) => {
      const janela = player.current?.iframeElement?.contentWindow;
      if (!janela || e.source !== janela) return;
      const d = e.data as { medza?: string; tipo?: unknown } | null;
      if (!d || d.medza !== "ponte" || typeof d.tipo !== "string") return;
      ponteRef.current?.(d as MensagemDaPonte);
    };
    window.addEventListener("message", aoMensagem);
    return () => window.removeEventListener("message", aoMensagem);
  }, []);

  // Eventos do player → relógio. A agulha anda entre os `timeupdate` (10/s) por quadro de energia.
  useEffect(() => {
    const el = player.current;
    if (!pronto || !el) return;
    let quadro = 0;
    let base = { tempo: 0, quando: 0, taxa: 1 };
    const andar = () => {
      quadro = 0;
      const p = player.current;
      if (!p || p.paused) return;
      const t = base.tempo + ((performance.now() - base.quando) / 1000) * base.taxa;
      relogio.mudar({ tempo: Math.min(t, relogio.ler().duracao || t) });
      quadro = proximoQuadro(andar);
    };
    const aoPronto = (e: Event) => {
      const d = (e as CustomEvent<{ duration?: number }>).detail?.duration ?? el.duration;
      relogio.mudar({ pronto: true, duracao: Number.isFinite(d) ? d : 0 });
      setCarregado(true);
      setProblema(null);
      erroRef.current?.(null);
      const t = Math.min(tempoGuardado.current, Math.max(0, (d || 0) - 1 / fps));
      if (t > 0) el.seek(t);
      relogio.mudar({ tempo: t });
    };
    const aoTempo = (e: Event) => {
      const t = (e as CustomEvent<{ currentTime?: number }>).detail?.currentTime ?? el.currentTime;
      base = { tempo: t, quando: performance.now(), taxa: el.playbackRate || 1 };
      relogio.mudar({ tempo: t });
    };
    const aoTocar = () => { base = { tempo: el.currentTime, quando: performance.now(), taxa: el.playbackRate || 1 }; relogio.mudar({ tocando: true }); if (!quadro) quadro = proximoQuadro(andar); };
    const aoParar = () => { relogio.mudar({ tocando: false, tempo: el.currentTime }); if (quadro) { cancelarQuadro(quadro); quadro = 0; } };
    const aoDuracao = () => relogio.mudar({ duracao: el.duration });
    const aoErro = (e: Event) => {
      const m = (e as CustomEvent<{ message?: string }>).detail?.message;
      registrar(`erro_do_player ${(m ?? "").slice(0, 120)}`);
      setProblema(m ? `O preview não abriu: ${m}` : "O preview não abriu. Confira a composição (o agente pode rodar o lint).");
      erroRef.current?.(m ? `O preview não abriu: ${m}` : "O preview não abriu. Confira a composição (o agente pode rodar o lint).");
    };
    el.addEventListener("ready", aoPronto);
    el.addEventListener("timeupdate", aoTempo);
    el.addEventListener("play", aoTocar);
    el.addEventListener("pause", aoParar);
    el.addEventListener("ended", aoParar);
    el.addEventListener("durationchange", aoDuracao);
    el.addEventListener("error", aoErro);
    return () => {
      if (quadro) cancelarQuadro(quadro);
      el.removeEventListener("ready", aoPronto);
      el.removeEventListener("timeupdate", aoTempo);
      el.removeEventListener("play", aoTocar);
      el.removeEventListener("pause", aoParar);
      el.removeEventListener("ended", aoParar);
      el.removeEventListener("durationchange", aoDuracao);
      el.removeEventListener("error", aoErro);
    };
  }, [pronto, relogio, fps]);

  // Janela escondida ou sem foco na pausa de energia: pausa (o vídeo não toca para ninguém).
  useEffect(() => {
    const pausarSeLonge = () => {
      const p = player.current;
      if (!p || p.paused) return;
      if (document.hidden || (document.documentElement.dataset.energia === "pausa" && !document.hasFocus())) p.pause();
    };
    const observador = new MutationObserver(pausarSeLonge);
    observador.observe(document.documentElement, { attributes: true, attributeFilter: ["data-energia"] });
    document.addEventListener("visibilitychange", pausarSeLonge);
    return () => { observador.disconnect(); document.removeEventListener("visibilitychange", pausarSeLonge); };
  }, []);

  useImperativeHandle(ref, () => ({
    tocar: () => { const p = player.current; if (p?.ready) { if (relogio.ler().tempo >= relogio.ler().duracao - 1e-3) p.seek(0); p.play(); } },
    pausar: () => player.current?.pause(),
    alternar: () => { const p = player.current; if (!p?.ready) return; if (p.paused) { if (relogio.ler().tempo >= relogio.ler().duracao - 1e-3) p.seek(0); p.play(); } else p.pause(); },
    irPara: (t: number) => {
      const p = player.current;
      const d = relogio.ler().duracao;
      const alvo = Math.max(0, Math.min(t, d > 0 ? d - 1 / fps : t));
      relogio.mudar({ tempo: alvo });
      tempoGuardado.current = alvo;
      if (p?.ready) p.seek(alvo);
    },
    quadros: (n: number) => {
      const p = player.current;
      p?.pause();
      const atual = Math.round(relogio.ler().tempo * fps);
      const alvo = Math.max(0, (atual + n) / fps);
      const d = relogio.ler().duracao;
      const t = d > 0 ? Math.min(alvo, d - 1 / fps) : alvo;
      relogio.mudar({ tempo: t });
      tempoGuardado.current = t;
      if (p?.ready) p.seek(t);
    },
    paraPonte: (mensagem) => {
      try { player.current?.iframeElement?.contentWindow?.postMessage({ ...mensagem, medza: "editor" }, "*"); } catch { /* iframe trocando */ }
    },
  }), [relogio, fps]);

  const escala = tamanho.w ? tamanho.w / largura : 0;
  return (
    <div ref={area} className="video-preview-area">
      <div className="video-preview-palco" style={{ width: tamanho.w || undefined, height: tamanho.h || undefined }}>
        <div ref={caixa} className="video-preview-player" data-testid="preview-do-video" />
        {carregado && escala > 0 && sobreposicao?.(escala)}
        {!pronto && !problema && <div className="video-preview-carregando">Abrindo o preview…</div>}
        {pronto && !carregado && !problema && <div className="video-preview-carregando">Carregando o vídeo…</div>}
        {problema && (
          <div className="video-preview-carregando is-problema" role="alert">
            <p>{problema}</p>
            <button type="button" className="video-mini-botao video-preview-de-novo" onClick={() => { setProblema(null); setModo("opaco"); setTentativa((n) => n + 1); }}>Tentar de novo</button>
          </div>
        )}
      </div>
    </div>
  );
});
