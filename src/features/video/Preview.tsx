// Fork (Editor de vídeo): preview com o `<hyperframes-player>` oficial (0.8.183).
//
// - A composição roda num iframe com sandbox **opaco** (`sandbox-origin="opaque"`): sem acesso à
//   janela do app nem às APIs do Tauri; só conversa com o player por mensagem. O HTML vem do
//   protocolo `medzavideo://`, com CSP fechada (ver `src-tauri/src/videos/protocolo.rs`).
// - Recarrega quando `versao` muda e volta para o mesmo ponto do tempo.
// - Pausa quando a janela some ou perde o foco (pausa de energia, `src/lib/energia.ts`); nunca toca
//   sozinho (respeita "reduzir movimento"). A agulha anda por `proximoQuadro`, que para na pausa.
import { forwardRef, useEffect, useImperativeHandle, useRef, useState } from "react";
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
}

interface Props {
  projeto: string;
  versao: number;
  largura: number;
  altura: number;
  fps: number;
  relogio: RelogioDoPreview;
  onErro?: (mensagem: string | null) => void;
}

let carregarPlayer: Promise<unknown> | null = null;

export const Preview = forwardRef<ControleDoPreview, Props>(function Preview({ projeto, versao, largura, altura, fps, relogio, onErro }, ref) {
  const caixa = useRef<HTMLDivElement>(null);
  const player = useRef<Player | null>(null);
  const [pronto, setPronto] = useState(false);
  const [url, setUrl] = useState<string | null>(() => urlDoProjeto(projeto, "index.html", versao));
  const tempoGuardado = useRef(0);
  const erroRef = useRef(onErro);
  erroRef.current = onErro;

  useEffect(() => {
    carregarPlayer ??= import("@hyperframes/player").catch((e) => { carregarPlayer = null; throw e; });
    let vivo = true;
    void Promise.all([carregarPlayer, carregarEndereco()]).then(() => {
      if (!vivo) return;
      setPronto(true);
      setUrl(urlDoProjeto(projeto, "index.html", versao));
    }).catch(() => { if (vivo) erroRef.current?.("Não consegui abrir o preview."); });
    return () => { vivo = false; };
  }, [projeto, versao]);

  // Cria o elemento uma vez e só troca o `src` (recarregar não pisca o quadro inteiro).
  useEffect(() => {
    const alvo = caixa.current;
    if (!pronto || !alvo || !url) return;
    let el = player.current;
    if (!el) {
      el = document.createElement("hyperframes-player") as Player;
      el.setAttribute("sandbox-origin", "opaque");
      el.setAttribute("disable-click-to-play", "");
      el.setAttribute("low-power-idle", "");
      el.style.width = "100%";
      el.style.height = "100%";
      el.style.display = "block";
      alvo.appendChild(el);
      player.current = el;
    }
    el.setAttribute("width", String(largura));
    el.setAttribute("height", String(altura));
    tempoGuardado.current = relogio.ler().tempo;
    relogio.mudar({ pronto: false, tocando: false });
    el.setAttribute("src", url);
  }, [pronto, url, largura, altura, relogio]);

  useEffect(() => () => { player.current?.remove(); player.current = null; }, []);

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
  }), [relogio, fps]);

  return (
    <div className="video-preview-palco" style={{ aspectRatio: `${largura} / ${altura}` }}>
      <div ref={caixa} className="video-preview-player" data-testid="preview-do-video" />
      {!pronto && <div className="video-preview-carregando">Abrindo o preview…</div>}
    </div>
  );
});
