// Fork (Roadmaps): o mapa do roadmap — regiões (módulos) com medalhões (etapas) e um baú no fim de cada
// uma, placar de nível/XP/meta do dia no topo, a etapa ao lado e o estudo em tela cheia.
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { Button } from "../../components/ui";
import { ChatIcon, CheckIcon, LockIcon, PlayIcon, StarIcon } from "../../components/ui/PixelIcon";
import { chaves, gerarCardsDeTudo } from "./acoes";
import { animarConclusao, rolarAte } from "./animacoes";
import { Caderno } from "./Caderno";
import { BotaoDeFoco, ChipDaIa, IconeDoRoadmap, PedidoEmAndamento, textoDoErro } from "./comuns";
import { foiCancelado } from "./ia";
import { bauAberto, estrelas, metaDoDia, nivel, xp, xpDaEtapa } from "./jogo";
import { lembranca, lembrar, tarefaEmAndamento, useLoja } from "./loja";
import { PainelDaEtapa } from "./PainelDaEtapa";
import { PainelDaIa, pedirAoPainel } from "./PainelDaIa";
import { PEDIDO_DO_QUIZ } from "./prompts";
import { TelaDeEstudo } from "./TelaDeEstudo";
import { TelaDeRevisao } from "./TelaDeRevisao";
import { estadosDasEtapas, etapaDeHoje, etapasEmOrdem } from "./progresso";
import { filaDeAgora } from "./revisao";
import type { RoadmapAberto } from "./armazenamento";
import type { EstadoDaEtapa } from "./tipos";

type Lado = "etapa" | "caderno" | "ia";
/** Onde a pessoa estava em cada roadmap, para voltar ao mesmo ponto (outra tela, link, reabrir a tela). */
interface Lugar { etapa: string | null; lateral: Lado | null; estudo: { etapa: string; url: string | null } | null }

const ROTULO: Record<EstadoDaEtapa, string> = { bloqueada: "Bloqueada", disponivel: "Disponível", andamento: "Em andamento", concluida: "Concluída" };

/** Zigue-zague do caminho: deslocamento horizontal de cada nó, em px (o CSS usa a mesma altura de linha). */
function desvio(i: number) { return Math.round(Math.sin(i * 0.9) * 72); }
const ALTURA = 112;
const LARGURA = 320;

/** Linha do caminho passando pelo centro dos nós (e chegando no baú); o trecho feito vai colorido. */
function LinhaDoCaminho({ inicio, estados }: { inicio: number; estados: EstadoDaEtapa[] }) {
  const pontos = [...estados.map((_, j) => [LARGURA / 2 + desvio(inicio + j), j * ALTURA + 34] as const), [LARGURA / 2, estados.length * ALTURA + 30] as const];
  const trecho = (ate: number) => pontos.slice(0, ate + 1).map(([x, y], j) => {
    if (j === 0) return `M${x} ${y}`;
    const [px, py] = pontos[j - 1];
    return `C${px} ${py + ALTURA / 2} ${x} ${y - ALTURA / 2} ${x} ${y}`;
  }).join(" ");
  let feitas = -1;
  while (feitas + 1 < estados.length && estados[feitas + 1] === "concluida") feitas++;
  const ate = feitas === estados.length - 1 ? pontos.length - 1 : feitas;
  const altura = estados.length * ALTURA + 64;
  return (
    <svg className="roadmap-linha" width={LARGURA} height={altura} viewBox={`0 0 ${LARGURA} ${altura}`} aria-hidden="true">
      <path d={trecho(pontos.length - 1)} className="roadmap-linha-resto" />
      {ate > 0 && <path d={trecho(ate)} className="roadmap-linha-feita" />}
    </svg>
  );
}

/** Anel da meta do dia (SVG estático: só muda quando o número muda). */
function AnelDoDia({ feito, meta }: { feito: number; meta: number }) {
  const pct = Math.min(1, meta ? feito / meta : 0);
  const r = 15, c = 2 * Math.PI * r;
  return (
    <span className="roadmap-anel-dia" title={`Meta de hoje: ${feito} de ${meta} XP`} aria-label={`Meta de hoje: ${feito} de ${meta} XP`} role="img">
      <svg width="38" height="38" viewBox="0 0 38 38" aria-hidden="true">
        <circle cx="19" cy="19" r={r} className="roadmap-anel-fundo" />
        <circle cx="19" cy="19" r={r} className="roadmap-anel-feito" strokeDasharray={c} strokeDashoffset={c * (1 - pct)} transform="rotate(-90 19 19)" />
      </svg>
      <span className="roadmap-anel-texto">{pct >= 1 ? "✓" : `${Math.round(pct * 100)}%`}</span>
    </span>
  );
}

export function Trilha({ r, onVoltar }: { r: RoadmapAberto; onVoltar: () => void }) {
  useLoja();
  const id = r.meta.id;
  const plano = r.plano;
  const guardado = lembranca<Lugar>(`lugar:${id}`);
  const [etapaAberta, setEtapaAberta] = useState<string | null>(guardado?.etapa ?? null);
  const [lateral, setLateral] = useState<Lado | null>(guardado?.lateral ?? null);
  const [estudo, setEstudo] = useState<Lugar["estudo"]>(guardado?.estudo ?? null);
  const [revisando, setRevisando] = useState(false);
  const [menuDaRevisao, setMenuDaRevisao] = useState(false);
  const [aviso, setAviso] = useState("");
  const nos = useRef(new Map<string, HTMLElement>());
  const baus = useRef(new Map<string, HTMLElement>());
  const estados = useMemo(() => (plano ? estadosDasEtapas(plano, r.progresso) : {}), [plano, r.progresso]);
  const hoje = plano ? etapaDeHoje(plano, r.progresso) : undefined;
  const agora = new Date();
  const fila = filaDeAgora(r.cards, agora).length;
  const pontos = xp(plano, r.progresso, r.cards, agora);
  const lvl = nivel(pontos.total);
  const anteriores = useRef(estados);

  useEffect(() => { lembrar(`lugar:${id}`, { etapa: etapaAberta, lateral, estudo }); }, [id, etapaAberta, lateral, estudo]);

  // Ao abrir: rola até a etapa de hoje (ou a que estava aberta ao lado, voltando de outra tela).
  useLayoutEffect(() => {
    if (estudo) return;
    const alvo = (guardado?.lateral === "etapa" && guardado.etapa) || hoje?.id;
    if (alvo) rolarAte(nos.current.get(alvo) ?? null);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [estudo === null]);

  // Etapa que acabou de virar concluída: o medalhão pulsa e sobe "+XP"; o baú do módulo completo abre.
  useEffect(() => {
    const animacoes: Animation[] = [];
    for (const [etapaId, estado] of Object.entries(estados)) {
      if (estado !== "concluida" || !anteriores.current[etapaId] || anteriores.current[etapaId] === "concluida") continue;
      const etapa = plano ? etapasEmOrdem(plano).find((e) => e.id === etapaId) : undefined;
      animacoes.push(...animarConclusao(nos.current.get(etapaId) ?? null, etapa ? `+${xpDaEtapa(etapa)} XP` : undefined));
      const modulo = plano?.modulos.find((m) => m.etapas.some((e) => e.id === etapaId));
      if (modulo && bauAberto(modulo, r.progresso)) animacoes.push(...animarConclusao(baus.current.get(modulo.id) ?? null, "Baú aberto!"));
    }
    anteriores.current = estados;
    return () => { for (const a of animacoes) a.cancel(); };
  }, [estados, plano, r.progresso]);

  if (!plano) return null;

  const abrirEtapa = (etapaId: string) => { setEtapaAberta(etapaId); setLateral("etapa"); };
  /** Link clicado na trilha (etapa ao lado, IA, caderno): abre o estudo daquela etapa com a página. */
  const abrirLink = (url: string) => {
    const etapaId = etapaAberta ?? hoje?.id;
    if (etapaId) setEstudo({ etapa: etapaId, url });
  };

  if (estudo && etapasEmOrdem(plano).some((e) => e.id === estudo.etapa)) {
    return (
      <TelaDeEstudo r={r} etapaId={estudo.etapa} estado={estados[estudo.etapa] ?? "bloqueada"} url={estudo.url}
        onUrl={(url) => setEstudo({ ...estudo, url })}
        onSair={() => { setEtapaAberta(estudo.etapa); setLateral("etapa"); setEstudo(null); }}
        onAbrirEtapa={(etapaId) => setEstudo({ etapa: etapaId, url: null })} />
    );
  }

  const total = etapasEmOrdem(plano).length;
  const feitas = Object.values(estados).filter((e) => e === "concluida").length;
  let indice = 0;

  const rodar = (acao: () => Promise<unknown>, depois?: (v: unknown) => void) => {
    setAviso("");
    void acao().then(depois).catch((e) => { if (!foiCancelado(e)) setAviso(textoDoErro(e)); });
  };

  return (
    <div className="roadmap-tela" style={{ ["--roadmap-cor" as string]: r.meta.cor }}>
      <div className="roadmap-principal">
        <header className="roadmap-topo roadmap-placar">
          <Button size="sm" variant="ghost" onClick={onVoltar}>← Roadmaps</Button>
          <IconeDoRoadmap icone={r.meta.icone} cor={r.meta.cor} tamanho={34} />
          <div className="min-w-0 flex-1">
            <h1 className="truncate text-base font-semibold text-text-primary">{r.meta.nome}</h1>
            <div className="flex items-center gap-2">
              <span className="roadmap-nivel" title={`${lvl.total} XP no total`}>Nv {lvl.nivel}</span>
              <div className="roadmap-barra roadmap-barra-xp" role="progressbar" aria-valuenow={lvl.noNivel} aria-valuemin={0} aria-valuemax={lvl.paraOProximo} aria-label="XP para o próximo nível">
                <span style={{ width: `${Math.round((lvl.noNivel / lvl.paraOProximo) * 100)}%` }} />
              </div>
              <span className="text-xs text-text-secondary">{lvl.noNivel}/{lvl.paraOProximo} XP · {feitas}/{total} etapas</span>
            </div>
          </div>
          <AnelDoDia feito={pontos.hoje} meta={metaDoDia(plano)} />
          <span className="roadmap-sequencia" title="Dias seguidos estudando">🔥 {r.meta.resumo.sequencia}</span>
          <ChipDaIa meta={r.meta} />
          <Button size="sm" variant={fila > 0 ? "primary" : "glass"} disabled={r.cards.cards.length === 0} onClick={() => setRevisando(true)}>
            Revisar{fila > 0 ? ` (${fila})` : ""}
          </Button>
          <div className="roadmap-chip-ia">
            <Button size="sm" variant="glass" aria-expanded={menuDaRevisao} onClick={() => setMenuDaRevisao(!menuDaRevisao)}>Revisão geral ▾</Button>
            {menuDaRevisao && (
              <div className="roadmap-chip-painel glass" role="menu" aria-label="Revisão geral" onKeyDown={(e) => { if (e.key === "Escape") setMenuDaRevisao(false); }}>
                <p className="text-xs text-text-muted">Tudo que você já concluiu ou está fazendo.</p>
                <Button size="sm" role="menuitem" disabled={r.somenteLeitura || Boolean(tarefaEmAndamento(chaves.cardsDeTudo(id)))}
                  onClick={() => { setMenuDaRevisao(false); rodar(() => gerarCardsDeTudo(id), (n) => setAviso(n ? `${String(n)} cards novos entraram na revisão.` : "Nenhum card novo: os que existem já cobrem tudo.")); }}>
                  Gerar cards de tudo que estudei
                </Button>
                <Button size="sm" role="menuitem" disabled={r.somenteLeitura} onClick={() => { setMenuDaRevisao(false); pedirAoPainel(id, PEDIDO_DO_QUIZ); setLateral("ia"); }}>
                  Quiz com a IA
                </Button>
                <Button size="sm" role="menuitem" onClick={() => { setMenuDaRevisao(false); setLateral("caderno"); }}>Revisar meu caderno</Button>
              </div>
            )}
          </div>
          <BotaoDeFoco />
          <Button size="sm" variant={lateral === "ia" ? "primary" : "glass"} onClick={() => setLateral(lateral === "ia" ? null : "ia")} aria-pressed={lateral === "ia"}>
            <ChatIcon size={14} aria-hidden="true" /> IA
          </Button>
        </header>
        {r.somenteLeitura && <p role="alert" className="roadmap-erro mx-6">Este roadmap foi salvo por uma versão mais nova do app. Dá para ver, mas não mudar; atualize o app.</p>}
        <div className="mx-6"><PedidoEmAndamento chave={chaves.cardsDeTudo(id)} /></div>
        {aviso && <p role="status" className="mx-6 mt-2 text-xs text-text-secondary">{aviso}</p>}

        <div className="roadmap-caminho" aria-label="Caminho">
          {plano.modulos.map((modulo, mi) => {
            const daRegiao = modulo.etapas.map((e) => estados[e.id] ?? "bloqueada");
            const feitasAqui = daRegiao.filter((e) => e === "concluida").length;
            const estrelasAqui = modulo.etapas.reduce((n, e) => n + estrelas(e, r.progresso, r.cards), 0);
            const aberto = bauAberto(modulo, r.progresso);
            return (
              <section key={modulo.id} className={`roadmap-regiao ${feitasAqui === modulo.etapas.length ? "is-completa" : ""}`} aria-label={modulo.titulo}>
                <div className="roadmap-regiao-faixa">
                  <span className="roadmap-regiao-numero">{mi + 1}</span>
                  <span className="min-w-0 flex-1">
                    <strong className="block truncate text-text-primary">{modulo.titulo}</strong>
                    {modulo.descricao && <span className="block truncate text-xs text-text-secondary">{modulo.descricao}</span>}
                  </span>
                  <span className="roadmap-regiao-placar" title="Etapas concluídas e estrelas da região">{feitasAqui}/{modulo.etapas.length} · ★ {estrelasAqui}/{modulo.etapas.length * 3}</span>
                </div>
                <ol className="roadmap-etapas">
                  <LinhaDoCaminho inicio={indice} estados={daRegiao} />
                  {modulo.etapas.map((etapa) => {
                    const estado = estados[etapa.id] ?? "bloqueada";
                    const i = indice++;
                    const Icone = estado === "concluida" ? CheckIcon : estado === "andamento" ? PlayIcon : estado === "disponivel" ? StarIcon : LockIcon;
                    const n = estrelas(etapa, r.progresso, r.cards);
                    const ehHoje = etapa.id === hoje?.id && estado !== "concluida";
                    return (
                      <li key={etapa.id} className="roadmap-etapa" style={{ transform: `translateX(${desvio(i)}px)` }}>
                        {ehHoje && <span className="roadmap-aqui" aria-hidden="true">Você está aqui</span>}
                        <button
                          ref={(el) => { if (el) nos.current.set(etapa.id, el); else nos.current.delete(etapa.id); }}
                          className={`roadmap-no is-${estado} ${ehHoje ? "is-hoje" : ""} ${etapa.id === etapaAberta && lateral === "etapa" ? "is-aberta" : ""}`}
                          onClick={() => abrirEtapa(etapa.id)}
                          aria-label={`${etapa.titulo}: ${ROTULO[estado]}${ehHoje ? " (hoje)" : ""}`}
                        >
                          <span className="roadmap-no-base" aria-hidden="true" />
                          <span className="roadmap-no-face" aria-hidden="true"><Icone size={22} /></span>
                        </button>
                        <span className={`roadmap-no-rotulo ${desvio(i) > 24 ? "is-esquerda" : ""}`}>
                          <span className="block text-sm text-text-primary">{etapa.titulo}</span>
                          <span className="block text-xs text-text-muted">{etapa.minutos} min · {xpDaEtapa(etapa)} XP</span>
                          {estado === "concluida" && <span className="roadmap-estrelas" aria-label={`${n} de 3 estrelas`}>{[1, 2, 3].map((k) => <span key={k} className={k <= n ? "is-ganha" : ""}>★</span>)}</span>}
                        </span>
                      </li>
                    );
                  })}
                  <li className="roadmap-bau-linha">
                    <span ref={(el) => { if (el) baus.current.set(modulo.id, el); else baus.current.delete(modulo.id); }}
                      className={`roadmap-bau ${aberto ? "is-aberto" : ""}`} role="img"
                      aria-label={aberto ? `Baú de ${modulo.titulo}: aberto` : `Baú de ${modulo.titulo}: conclua a região para abrir`}
                      title={aberto ? "Região completa!" : "Conclua todas as etapas da região para abrir"}>
                      {aberto ? "🏆" : "🔒"}
                    </span>
                  </li>
                </ol>
              </section>
            );
          })}
          <p className="py-8 text-center text-xs text-text-muted">{feitas === total ? "Mapa completo. Peça à IA o próximo desafio." : "Fim do mapa"}</p>
        </div>
      </div>

      {lateral && (
        <aside className="roadmap-lateral" aria-label={lateral === "ia" ? "IA ao lado" : lateral === "caderno" ? "Caderno" : "Etapa"}>
          <div className="roadmap-abas" role="tablist">
            <button role="tab" aria-selected={lateral === "etapa"} disabled={!etapaAberta} onClick={() => setLateral("etapa")}>Etapa</button>
            <button role="tab" aria-selected={lateral === "caderno"} onClick={() => setLateral("caderno")}>Caderno</button>
            <button role="tab" aria-selected={lateral === "ia"} onClick={() => setLateral("ia")}>IA ao lado</button>
            <button className="ml-auto" aria-label="Fechar painel" onClick={() => setLateral(null)}>×</button>
          </div>
          {lateral === "etapa" && etapaAberta
            ? <PainelDaEtapa key={etapaAberta} r={r} etapaId={etapaAberta} estado={estados[etapaAberta] ?? "bloqueada"} onPerguntar={() => setLateral("ia")}
                onAbrirEtapa={abrirEtapa} onAbrirLink={(url) => setEstudo({ etapa: etapaAberta, url })} onTelaCheia={() => setEstudo({ etapa: etapaAberta, url: null })} />
            : lateral === "caderno"
              ? <div className="roadmap-estudo-aba"><Caderno r={r} etapa={etapaAberta && plano ? etapasEmOrdem(plano).find((e) => e.id === etapaAberta) : hoje} onAbrirLink={abrirLink} /></div>
              : <PainelDaIa r={r} etapaId={etapaAberta ?? undefined} onAbrirLink={abrirLink} />}
        </aside>
      )}

      {revisando && <TelaDeRevisao r={r} onFechar={() => setRevisando(false)} />}
    </div>
  );
}
