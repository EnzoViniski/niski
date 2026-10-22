// Fork (Editor de vídeo): timeline com faixas e clipes da composição.
//
// Arrastar o clipe move (no tempo e entre faixas); as pontas cortam; clicar na régua ou numa faixa
// vazia leva a agulha. Durante o arraste nada é gravado: só ao soltar vira uma edição (uma versão
// no histórico). Ímã nas bordas dos outros clipes, na agulha e no zero.
import { useCallback, useEffect, useMemo, useRef, useState, type PointerEvent as EventoDePonteiro } from "react";
import { duracaoTotal, duracaoVisivel, type Clipe, type Composicao } from "./composicao";
import { formatarTempo, useRelogio, type RelogioDoPreview } from "./relogio";

export const TIPO_DE_ARRASTE_DA_MIDIA = "application/x-medza-midia";

export interface Colega { nome: string; cor: string; tempo?: number; selecionado?: string }

interface Props {
  comp: Composicao;
  duracoes: Record<string, number>;
  relogio: RelogioDoPreview;
  selecionado: string | null;
  onSelecionar: (chave: string | null) => void;
  onIrPara: (t: number) => void;
  onMover: (chave: string, inicio: number, faixa: number) => void;
  onAparar: (chave: string, lado: "inicio" | "fim", t: number) => void;
  onSoltarMidia: (midia: string, inicio: number, faixa: number) => void;
  colegas?: Colega[];
  somenteLeitura?: boolean;
  /** Fork (0.23): endereço de uma mídia do projeto (miniatura do vídeo no clipe). */
  urlDaMidia?: (src: string) => string | null;
}

const ALTURA_DA_FAIXA = 40;
const LARGURA_DO_ROTULO = 76;
const IMA_PX = 8;

const ROTULO_DO_TIPO: Record<Clipe["tipo"], string> = {
  video: "Vídeo", audio: "Áudio", imagem: "Imagem", composicao: "Cena", texto: "Texto", elemento: "Bloco",
};

type Arraste =
  | { tipo: "mover"; chave: string; x0: number; y0: number; inicio: number; faixa: number; dur: number; dx: number; dy: number; mexeu: boolean }
  | { tipo: "aparar"; chave: string; lado: "inicio" | "fim"; x0: number; inicio: number; dur: number; dx: number }
  | { tipo: "agulha" };

function passoDaRegua(pxPorSeg: number) {
  for (const passo of [0.1, 0.25, 0.5, 1, 2, 5, 10, 15, 30, 60]) if (passo * pxPorSeg >= 56) return passo;
  return 120;
}

export function Timeline({ comp, duracoes, relogio, selecionado, onSelecionar, onIrPara, onMover, onAparar, onSoltarMidia, colegas = [], somenteLeitura, urlDaMidia }: Props) {
  const area = useRef<HTMLDivElement>(null);
  const [largura, setLargura] = useState(800);
  const [zoom, setZoom] = useState(1);
  const [arraste, setArraste] = useState<Arraste | null>(null);
  const arrasteAtual = useRef<Arraste | null>(null);
  arrasteAtual.current = arraste;
  const [alvoDaMidia, setAlvoDaMidia] = useState<{ t: number; faixa: number } | null>(null);
  const total = duracaoTotal(comp, duracoes);
  const fps = comp.raiz?.fps ?? 30;

  useEffect(() => {
    const el = area.current;
    if (!el || typeof ResizeObserver === "undefined") return;
    const o = new ResizeObserver(([e]) => setLargura(Math.max(200, e.contentRect.width - LARGURA_DO_ROTULO)));
    o.observe(el);
    return () => o.disconnect();
  }, []);

  const pxPorSeg = Math.max(8, (largura / Math.max(1, total * 1.05)) * zoom);
  const larguraTotal = Math.max(largura, (total + 2) * pxPorSeg);
  const faixas = useMemo(() => Math.max(1, ...comp.clipes.map((c) => c.faixa + 1)) + 1, [comp.clipes]);
  const passo = passoDaRegua(pxPorSeg);

  const tempoNoPonto = useCallback((clientX: number) => {
    const caixa = area.current?.getBoundingClientRect();
    const rolagem = area.current?.scrollLeft ?? 0;
    if (!caixa) return 0;
    return Math.max(0, (clientX - caixa.left - LARGURA_DO_ROTULO + rolagem) / pxPorSeg);
  }, [pxPorSeg]);

  const faixaNoPonto = useCallback((clientY: number) => {
    const linhas = area.current?.querySelector(".video-tl-faixas")?.getBoundingClientRect();
    if (!linhas) return 0;
    return Math.max(0, Math.min(faixas - 1, Math.floor((clientY - linhas.top) / ALTURA_DA_FAIXA)));
  }, [faixas]);

  /** Bordas para o ímã: zero, agulha, fim e as pontas dos outros clipes. */
  const bordas = useCallback((exceto: string) => {
    const lista = [0, relogio.ler().tempo, total];
    for (const c of comp.clipes) if (c.chave !== exceto) { lista.push(c.inicio, c.inicio + duracaoVisivel(c, duracoes)); }
    return lista;
  }, [comp.clipes, duracoes, relogio, total]);

  const ima = useCallback((t: number, exceto: string) => {
    let melhor = t, distancia = IMA_PX / pxPorSeg;
    for (const b of bordas(exceto)) if (Math.abs(b - t) < distancia) { melhor = b; distancia = Math.abs(b - t); }
    return melhor;
  }, [bordas, pxPorSeg]);

  const resultadoDoMover = useCallback((a: Extract<Arraste, { tipo: "mover" }>) => {
    const bruto = Math.max(0, a.inicio + a.dx / pxPorSeg);
    const pelaFrente = ima(bruto, a.chave);
    const porTras = ima(bruto + a.dur, a.chave) - a.dur;
    const inicio = Math.abs(pelaFrente - bruto) <= Math.abs(porTras - bruto) ? pelaFrente : Math.max(0, porTras);
    const faixa = Math.max(0, Math.min(faixas - 1, a.faixa + Math.round(a.dy / ALTURA_DA_FAIXA)));
    return { inicio, faixa };
  }, [faixas, ima, pxPorSeg]);

  const resultadoDoAparar = useCallback((a: Extract<Arraste, { tipo: "aparar" }>) => {
    const ponta = a.lado === "inicio" ? a.inicio : a.inicio + a.dur;
    return ima(Math.max(0, ponta + a.dx / pxPorSeg), a.chave);
  }, [ima, pxPorSeg]);

  useEffect(() => {
    if (!arraste) return;
    const mover = (e: PointerEvent) => {
      if (arraste.tipo === "agulha") { onIrPara(Math.min(total, tempoNoPonto(e.clientX))); return; }
      setArraste((a) => {
        if (!a || a.tipo === "agulha") return a;
        if (a.tipo === "mover") {
          const dx = e.clientX - a.x0, dy = e.clientY - a.y0;
          return { ...a, dx, dy, mexeu: a.mexeu || Math.abs(dx) > 3 || Math.abs(dy) > 3 };
        }
        return { ...a, dx: e.clientX - a.x0 };
      });
    };
    const soltar = () => {
      const a = arrasteAtual.current;
      setArraste(null);
      if (a?.tipo === "mover" && a.mexeu) {
        const r = resultadoDoMover(a);
        const clipe = comp.clipes.find((c) => c.chave === a.chave);
        if (clipe && (Math.abs(r.inicio - clipe.inicio) > 1e-3 || r.faixa !== clipe.faixa)) onMover(a.chave, r.inicio, r.faixa);
      } else if (a?.tipo === "aparar" && Math.abs(a.dx) > 1) {
        onAparar(a.chave, a.lado, resultadoDoAparar(a));
      }
    };
    window.addEventListener("pointermove", mover);
    window.addEventListener("pointerup", soltar, { once: true });
    return () => { window.removeEventListener("pointermove", mover); window.removeEventListener("pointerup", soltar); };
  }, [arraste, comp.clipes, onAparar, onIrPara, onMover, resultadoDoAparar, resultadoDoMover, tempoNoPonto, total]);

  const comecarArraste = (e: EventoDePonteiro, clipe: Clipe, lado?: "inicio" | "fim") => {
    if (e.button !== 0) return;
    e.stopPropagation();
    onSelecionar(clipe.chave);
    if (somenteLeitura || !clipe.editavel) return;
    const dur = duracaoVisivel(clipe, duracoes);
    if (lado) setArraste({ tipo: "aparar", chave: clipe.chave, lado, x0: e.clientX, inicio: clipe.inicio, dur, dx: 0 });
    else setArraste({ tipo: "mover", chave: clipe.chave, x0: e.clientX, y0: e.clientY, inicio: clipe.inicio, faixa: clipe.faixa, dur, dx: 0, dy: 0, mexeu: false });
  };

  const marcas = [];
  for (let t = 0; t <= total + 2 + 1e-6; t += passo) marcas.push(Math.round(t * 1000) / 1000);

  return (
    <div className="video-tl" aria-label="Timeline">
      <div className="video-tl-barra">
        <span className="video-tl-dica">{somenteLeitura ? "Só leitura enquanto o agente edita" : "Arraste para mover · puxe as pontas para cortar · solte mídia da biblioteca numa faixa"}</span>
        <label className="video-tl-zoom">
          <span>Zoom</span>
          <input type="range" min={0.5} max={8} step={0.1} value={zoom} onChange={(e) => setZoom(Number(e.target.value))} aria-label="Zoom da timeline" />
        </label>
      </div>
      <div ref={area} className="video-tl-area" onWheel={(e) => { if (e.ctrlKey || e.metaKey) { e.preventDefault(); setZoom((z) => Math.max(0.5, Math.min(8, z * (e.deltaY < 0 ? 1.15 : 0.87)))); } }}>
        <div className="video-tl-conteudo" style={{ width: larguraTotal + LARGURA_DO_ROTULO }}>
          <div className="video-tl-regua" style={{ paddingLeft: LARGURA_DO_ROTULO }}
            onPointerDown={(e) => { if (e.button === 0) { onIrPara(Math.min(total, tempoNoPonto(e.clientX))); setArraste({ tipo: "agulha" }); } }}>
            {marcas.map((t) => (
              <span key={t} className="video-tl-marca" style={{ left: LARGURA_DO_ROTULO + t * pxPorSeg }}>{formatarTempo(t).replace(/\.00$/, "")}</span>
            ))}
            <span className="video-tl-fim" style={{ left: LARGURA_DO_ROTULO + total * pxPorSeg }} title={`Fim do vídeo: ${formatarTempo(total)}`} />
          </div>
          <div className="video-tl-faixas" style={{ height: faixas * ALTURA_DA_FAIXA }}
            onPointerDown={(e) => { if (e.button === 0 && e.target === e.currentTarget) { onSelecionar(null); onIrPara(Math.min(total, tempoNoPonto(e.clientX))); } }}
            onDragOver={(e) => {
              if (somenteLeitura || !e.dataTransfer.types.includes(TIPO_DE_ARRASTE_DA_MIDIA)) return;
              e.preventDefault();
              e.dataTransfer.dropEffect = "copy";
              setAlvoDaMidia({ t: ima(tempoNoPonto(e.clientX), ""), faixa: faixaNoPonto(e.clientY) });
            }}
            onDragLeave={() => setAlvoDaMidia(null)}
            onDrop={(e) => {
              const midia = e.dataTransfer.getData(TIPO_DE_ARRASTE_DA_MIDIA);
              setAlvoDaMidia(null);
              if (!midia || somenteLeitura) return;
              e.preventDefault();
              onSoltarMidia(midia, ima(tempoNoPonto(e.clientX), ""), faixaNoPonto(e.clientY));
            }}>
            {Array.from({ length: faixas }, (_, f) => (
              <div key={f} className="video-tl-linha" style={{ top: f * ALTURA_DA_FAIXA, height: ALTURA_DA_FAIXA }}>
                <span className="video-tl-rotulo" style={{ width: LARGURA_DO_ROTULO }}>{f === faixas - 1 ? "Nova" : `Faixa ${f + 1}`}</span>
              </div>
            ))}
            {comp.clipes.map((c) => {
              let inicio = c.inicio, dur = duracaoVisivel(c, duracoes), faixa = c.faixa;
              if (arraste?.tipo === "mover" && arraste.chave === c.chave && arraste.mexeu) {
                const r = resultadoDoMover(arraste);
                inicio = r.inicio; faixa = r.faixa;
              } else if (arraste?.tipo === "aparar" && arraste.chave === c.chave) {
                const t = resultadoDoAparar(arraste);
                if (arraste.lado === "inicio") { const fim = inicio + dur; inicio = Math.min(t, fim - 0.1); dur = fim - inicio; }
                else dur = Math.max(0.1, t - inicio);
              }
              const semDuracao = c.duracao === undefined;
              const deColegas = colegas.filter((g) => g.selecionado === c.chave);
              return (
                <div key={c.chave}
                  className={`video-tl-clipe is-${c.tipo}${selecionado === c.chave ? " is-selecionado" : ""}${c.oculto ? " is-oculto" : ""}${!c.editavel ? " is-fixo" : ""}${semDuracao ? " is-sem-duracao" : ""}${arraste && arraste.tipo !== "agulha" && arraste.chave === c.chave ? " is-arrastando" : ""}`}
                  style={{ left: LARGURA_DO_ROTULO + inicio * pxPorSeg, width: Math.max(6, dur * pxPorSeg), top: faixa * ALTURA_DA_FAIXA + 4, height: ALTURA_DA_FAIXA - 8, outlineColor: deColegas[0]?.cor }}
                  role="button" tabIndex={0} aria-pressed={selecionado === c.chave}
                  aria-label={`${ROTULO_DO_TIPO[c.tipo]} ${c.rotulo}, de ${formatarTempo(c.inicio)} a ${formatarTempo(c.inicio + duracaoVisivel(c, duracoes))}`}
                  title={`${ROTULO_DO_TIPO[c.tipo]}: ${c.rotulo}${c.aninhado ? " (dentro de outro clipe)" : ""}${!c.editavel ? " · o início depende de outro clipe" : ""}`}
                  onPointerDown={(e) => comecarArraste(e, c)}
                  onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); onSelecionar(c.chave); } }}>
                  {!somenteLeitura && c.editavel && <span className="video-tl-ponta is-inicio" onPointerDown={(e) => comecarArraste(e, c, "inicio")} aria-hidden="true" />}
                  {/* Fork (0.23): miniatura do vídeo e o conteúdo do clipe na frente (o tipo vai na cor, no
                      título e no leitor de tela; "Bloco …" escondia de que cena se tratava). */}
                  {c.tipo === "video" && c.src && dur * pxPorSeg > 48 && <MiniaturaDoClipe url={urlDaMidia?.(c.src) ?? null} inicio={c.midiaInicio} />}
                  <span className="video-tl-clipe-texto">{c.rotulo || ROTULO_DO_TIPO[c.tipo]}</span>
                  {deColegas.map((g) => <span key={g.nome} className="video-tl-colega" style={{ background: g.cor }}>{g.nome}</span>)}
                  {!somenteLeitura && c.editavel && <span className="video-tl-ponta is-fim" onPointerDown={(e) => comecarArraste(e, c, "fim")} aria-hidden="true" />}
                </div>
              );
            })}
            {alvoDaMidia && <div className="video-tl-alvo" style={{ left: LARGURA_DO_ROTULO + alvoDaMidia.t * pxPorSeg, top: alvoDaMidia.faixa * ALTURA_DA_FAIXA + 2, height: ALTURA_DA_FAIXA - 4 }} />}
          </div>
          <Agulha relogio={relogio} pxPorSeg={pxPorSeg} fps={fps} onPegar={() => setArraste({ tipo: "agulha" })} />
          {colegas.filter((g) => g.tempo !== undefined).map((g) => (
            <div key={g.nome} className="video-tl-agulha-colega" style={{ left: LARGURA_DO_ROTULO + (g.tempo ?? 0) * pxPorSeg, background: g.cor }} title={`${g.nome} está aqui`} />
          ))}
        </div>
      </div>
    </div>
  );
}

/** Um quadro do vídeo no começo do clipe (só metadados e um pedaço; nada toca). */
function MiniaturaDoClipe({ url, inicio }: { url: string | null; inicio: number }) {
  const [falhou, setFalhou] = useState(false);
  const [pronta, setPronta] = useState(false);
  if (!url || falhou) return null;
  // Escondida até ter o quadro: o WebKit desenha a caixa em branco antes de decodificar.
  return <video className={`video-tl-miniatura${pronta ? " is-pronta" : ""}`} src={`${url}#t=${Math.max(0.05, inicio)}`} muted preload="auto" playsInline aria-hidden="true"
    onLoadedData={() => setPronta(true)} onError={() => setFalhou(true)} />;
}

/** Só a agulha assina o relógio: o resto da timeline não re-renderiza enquanto o vídeo toca. */
function Agulha({ relogio, pxPorSeg, fps, onPegar }: { relogio: RelogioDoPreview; pxPorSeg: number; fps: number; onPegar: () => void }) {
  const { tempo } = useRelogio(relogio);
  const quadro = Math.round(tempo * fps);
  return (
    <div className="video-tl-agulha" style={{ transform: `translateX(${LARGURA_DO_ROTULO + tempo * pxPorSeg}px)` }}
      onPointerDown={(e) => { if (e.button === 0) { e.stopPropagation(); onPegar(); } }}
      role="slider" aria-label="Agulha" aria-valuenow={Number(tempo.toFixed(2))} aria-valuetext={`${formatarTempo(tempo)}, quadro ${quadro}`}>
      <span className="video-tl-agulha-cabeca" />
    </div>
  );
}
