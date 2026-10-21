// Fork (Roadmaps): o caminho de fases (módulos → etapas), com a etapa ao lado, a revisão e a IA.
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { Button } from "../../components/ui";
import { ChatIcon, CheckIcon, LockIcon, PlayIcon, StarIcon } from "../../components/ui/PixelIcon";
import { animarConclusao, rolarAte } from "./animacoes";
import { IconeDoRoadmap } from "./comuns";
import { lembranca, lembrar, useLoja } from "./loja";
import { PainelDaEtapa } from "./PainelDaEtapa";
import { PainelDaIa } from "./PainelDaIa";
import { TelaDeRevisao } from "./TelaDeRevisao";
import { estadosDasEtapas, etapaDeHoje } from "./progresso";
import { filaDeAgora } from "./revisao";
import type { RoadmapAberto } from "./armazenamento";
import type { EstadoDaEtapa } from "./tipos";

/** Onde a pessoa estava em cada roadmap, para voltar ao mesmo ponto (link aberto, outra tela). */
interface Lugar { etapa: string | null; lateral: "etapa" | "ia" | null }

const ROTULO: Record<EstadoDaEtapa, string> = { bloqueada: "Bloqueada", disponivel: "Disponível", andamento: "Em andamento", concluida: "Concluída" };

/** Zigue-zague do caminho: deslocamento horizontal de cada nó, em px (o CSS usa a mesma altura de linha). */
function desvio(i: number) { return Math.round(Math.sin(i * 0.9) * 72); }
const ALTURA = 104;
const LARGURA = 320;

/** Linha do caminho passando pelo centro dos nós; o trecho até a última concluída vai colorido. */
function LinhaDoCaminho({ inicio, estados }: { inicio: number; estados: EstadoDaEtapa[] }) {
  const pontos = estados.map((_, j) => [LARGURA / 2 + desvio(inicio + j), j * ALTURA + 32] as const);
  const trecho = (ate: number) => pontos.slice(0, ate + 1).map(([x, y], j) => {
    if (j === 0) return `M${x} ${y}`;
    const [px, py] = pontos[j - 1];
    return `C${px} ${py + ALTURA / 2} ${x} ${y - ALTURA / 2} ${x} ${y}`;
  }).join(" ");
  let feitas = -1;
  while (feitas + 1 < estados.length && estados[feitas + 1] === "concluida") feitas++;
  const altura = (estados.length - 1) * ALTURA + 64;
  return (
    <svg className="roadmap-linha" width={LARGURA} height={altura} viewBox={`0 0 ${LARGURA} ${altura}`} aria-hidden="true">
      <path d={trecho(pontos.length - 1)} className="roadmap-linha-resto" />
      {feitas > 0 && <path d={trecho(feitas)} className="roadmap-linha-feita" />}
    </svg>
  );
}

export function Trilha({ r, onVoltar }: { r: RoadmapAberto; onVoltar: () => void }) {
  useLoja();
  const id = r.meta.id;
  const plano = r.plano;
  const guardado = lembranca<Lugar>(`lugar:${id}`);
  const [etapaAberta, setEtapaAberta] = useState<string | null>(guardado?.etapa ?? null);
  const [lateral, setLateral] = useState<"etapa" | "ia" | null>(guardado?.lateral ?? null);
  const [revisando, setRevisando] = useState(false);
  const nos = useRef(new Map<string, HTMLButtonElement>());
  const estados = useMemo(() => (plano ? estadosDasEtapas(plano, r.progresso) : {}), [plano, r.progresso]);
  const hoje = plano ? etapaDeHoje(plano, r.progresso) : undefined;
  const fila = filaDeAgora(r.cards, new Date()).length;
  const anteriores = useRef(estados);

  useEffect(() => { lembrar(`lugar:${id}`, { etapa: etapaAberta, lateral }); }, [id, etapaAberta, lateral]);

  // Ao abrir: rola até a etapa de hoje (ou a que estava aberta ao lado, voltando de outra tela).
  useLayoutEffect(() => {
    const alvo = (guardado?.lateral === "etapa" && guardado.etapa) || hoje?.id;
    if (alvo) rolarAte(nos.current.get(alvo) ?? null);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Etapa que acabou de virar concluída: animação no nó.
  useEffect(() => {
    const animacoes: Animation[] = [];
    for (const [etapaId, estado] of Object.entries(estados)) {
      if (estado === "concluida" && anteriores.current[etapaId] && anteriores.current[etapaId] !== "concluida") {
        animacoes.push(...animarConclusao(nos.current.get(etapaId) ?? null));
      }
    }
    anteriores.current = estados;
    return () => { for (const a of animacoes) a.cancel(); };
  }, [estados]);

  if (!plano) return null;
  const total = plano.modulos.reduce((n, m) => n + m.etapas.length, 0);
  const feitas = Object.values(estados).filter((e) => e === "concluida").length;
  const pct = total ? Math.round((feitas / total) * 100) : 0;
  let indice = 0;

  const abrirEtapa = (etapaId: string) => { setEtapaAberta(etapaId); setLateral("etapa"); };

  return (
    <div className="roadmap-tela" style={{ ["--roadmap-cor" as string]: r.meta.cor }}>
      <div className="roadmap-principal">
        <header className="roadmap-topo">
          <Button size="sm" variant="ghost" onClick={onVoltar}>← Roadmaps</Button>
          <IconeDoRoadmap icone={r.meta.icone} cor={r.meta.cor} tamanho={32} />
          <div className="min-w-0 flex-1">
            <h1 className="truncate text-base font-semibold text-text-primary">{r.meta.nome}</h1>
            <div className="flex items-center gap-2">
              <div className="roadmap-barra w-40" role="progressbar" aria-valuenow={pct} aria-valuemin={0} aria-valuemax={100} aria-label="Progresso do caminho">
                <span style={{ width: `${pct}%`, background: r.meta.cor }} />
              </div>
              <span className="text-xs text-text-secondary">{feitas}/{total}</span>
            </div>
          </div>
          <span className="roadmap-sequencia" title="Dias seguidos estudando">🔥 {r.meta.resumo.sequencia}</span>
          <Button size="sm" variant={fila > 0 ? "primary" : "glass"} disabled={r.cards.cards.length === 0} onClick={() => setRevisando(true)}>
            Revisar{fila > 0 ? ` (${fila})` : ""}
          </Button>
          <Button size="sm" variant={lateral === "ia" ? "primary" : "glass"} onClick={() => setLateral(lateral === "ia" ? null : "ia")} aria-pressed={lateral === "ia"}>
            <ChatIcon size={14} aria-hidden="true" /> IA
          </Button>
        </header>
        {r.somenteLeitura && <p role="alert" className="roadmap-erro mx-6">Este roadmap foi salvo por uma versão mais nova do app. Dá para ver, mas não mudar; atualize o app.</p>}

        <div className="roadmap-caminho" aria-label="Caminho">
          {plano.modulos.map((modulo, mi) => (
            <section key={modulo.id} className="roadmap-modulo" aria-label={modulo.titulo}>
              <div className="roadmap-modulo-titulo glass">
                <span className="text-xs uppercase tracking-wide text-text-muted">Módulo {mi + 1}</span>
                <strong className="block text-text-primary">{modulo.titulo}</strong>
                {modulo.descricao && <span className="block text-xs text-text-secondary">{modulo.descricao}</span>}
              </div>
              <ol className="roadmap-etapas">
                <LinhaDoCaminho inicio={indice} estados={modulo.etapas.map((e) => estados[e.id] ?? "bloqueada")} />
                {modulo.etapas.map((etapa) => {
                  const estado = estados[etapa.id] ?? "bloqueada";
                  const i = indice++;
                  const Icone = estado === "concluida" ? CheckIcon : estado === "andamento" ? PlayIcon : estado === "disponivel" ? StarIcon : LockIcon;
                  return (
                    <li key={etapa.id} className="roadmap-etapa" style={{ transform: `translateX(${desvio(i)}px)` }}>
                      <button
                        ref={(el) => { if (el) nos.current.set(etapa.id, el); else nos.current.delete(etapa.id); }}
                        className={`roadmap-no is-${estado} ${etapa.id === hoje?.id ? "is-hoje" : ""} ${etapa.id === etapaAberta && lateral === "etapa" ? "is-aberta" : ""}`}
                        onClick={() => abrirEtapa(etapa.id)}
                        aria-label={`${etapa.titulo}: ${ROTULO[estado]}${etapa.id === hoje?.id ? " (hoje)" : ""}`}
                      >
                        <Icone size={22} aria-hidden="true" />
                      </button>
                      <span className={`roadmap-no-rotulo ${desvio(i) > 24 ? "is-esquerda" : ""}`}>
                        <span className="block text-sm text-text-primary">{etapa.titulo}</span>
                        <span className="block text-xs text-text-muted">{etapa.minutos} min{etapa.id === hoje?.id ? " · hoje" : ""}</span>
                      </span>
                    </li>
                  );
                })}
              </ol>
            </section>
          ))}
          <p className="py-8 text-center text-xs text-text-muted">{feitas === total ? "Caminho concluído. Peça à IA o próximo passo." : "Fim do caminho"}</p>
        </div>
      </div>

      {lateral && (
        <aside className="roadmap-lateral" aria-label={lateral === "ia" ? "IA ao lado" : "Etapa"}>
          <div className="roadmap-abas" role="tablist">
            <button role="tab" aria-selected={lateral === "etapa"} disabled={!etapaAberta} onClick={() => setLateral("etapa")}>Etapa</button>
            <button role="tab" aria-selected={lateral === "ia"} onClick={() => setLateral("ia")}>IA ao lado</button>
            <button className="ml-auto" aria-label="Fechar painel" onClick={() => setLateral(null)}>×</button>
          </div>
          {lateral === "etapa" && etapaAberta
            ? <PainelDaEtapa key={etapaAberta} r={r} etapaId={etapaAberta} estado={estados[etapaAberta] ?? "bloqueada"} onPerguntar={() => setLateral("ia")} onAbrirEtapa={abrirEtapa} />
            : <PainelDaIa r={r} etapaId={etapaAberta ?? undefined} />}
        </aside>
      )}

      {revisando && <TelaDeRevisao r={r} onFechar={() => setRevisando(false)} />}
    </div>
  );
}
