// Fork (Rotina): gráficos pequenos em SVG, sem biblioteca. Uma série por gráfico (o título nomeia;
// sem legenda), um eixo só, marcas finas, grade discreta, dica ao passar o mouse/foco e a tabela
// com os mesmos números logo abaixo (leitor de tela e quem prefere ver os valores).
import { useState } from "react";
import { ultimo } from "./dominio";

export interface PontoDoGrafico { rotulo: string; valor: number; destaque?: boolean; detalhe?: string }

const L = 320, A = 140, M = { e: 36, d: 8, c: 10, b: 22 };

function escala(valores: number[]) {
  const max = Math.max(1, ...valores);
  const passo = max <= 5 ? 1 : max <= 20 ? 5 : max <= 50 ? 10 : max <= 200 ? 25 : max <= 500 ? 100 : 250;
  const topo = Math.ceil(max / passo) * passo;
  const marcas: number[] = [];
  for (let v = 0; v <= topo; v += passo) marcas.push(v);
  return { topo, marcas: marcas.length > 6 ? marcas.filter((_, i) => i % 2 === 0) : marcas };
}

const fmt = (n: number) => new Intl.NumberFormat("pt-BR", { maximumFractionDigits: 1 }).format(n);

function Tabela({ titulo, unidade, pontos }: { titulo: string; unidade: string; pontos: PontoDoGrafico[] }) {
  return (
    <table className="rotina-tabela">
      <caption className="sr-only">{titulo}</caption>
      <thead><tr><th scope="col">Quando</th><th scope="col">{unidade}</th></tr></thead>
      <tbody>{pontos.map((p) => <tr key={p.rotulo}><td>{p.rotulo}</td><td>{fmt(p.valor)}{p.destaque ? " · recorde" : ""}</td></tr>)}</tbody>
    </table>
  );
}

/** Linha com marcadores (evolução no tempo). `destaque` = recorde (marcador cheio e anel). */
export function GraficoDeLinha({ titulo, unidade, pontos }: { titulo: string; unidade: string; pontos: PontoDoGrafico[] }) {
  const [foco, setFoco] = useState<number | null>(null);
  const [tabela, setTabela] = useState(false);
  if (pontos.length === 0) return <p className="rotina-vazio">Sem registros ainda.</p>;
  const { topo, marcas } = escala(pontos.map((p) => p.valor));
  const x = (i: number) => M.e + (pontos.length === 1 ? (L - M.e - M.d) / 2 : (i * (L - M.e - M.d)) / (pontos.length - 1));
  const y = (v: number) => M.c + (A - M.c - M.b) * (1 - v / topo);
  const linha = pontos.map((p, i) => `${i ? "L" : "M"}${x(i).toFixed(1)},${y(p.valor).toFixed(1)}`).join(" ");
  const atual = foco === null ? null : pontos[foco];
  return (
    <figure className="rotina-grafico">
      <figcaption>{titulo} <small className="rotina-muted">({unidade})</small>
        <button type="button" className="rotina-mini" onClick={() => setTabela((t) => !t)}>{tabela ? "Ver gráfico" : "Ver tabela"}</button></figcaption>
      {tabela ? <Tabela titulo={titulo} unidade={unidade} pontos={pontos} /> : (
        <div className="rotina-grafico-area">
          <svg viewBox={`0 0 ${L} ${A}`} role="img" aria-label={`${titulo}: de ${fmt(pontos[0].valor)} a ${fmt(ultimo(pontos)!.valor)} ${unidade}`}
            onMouseLeave={() => setFoco(null)}>
            {marcas.map((m) => (
              <g key={m}>
                <line x1={M.e} x2={L - M.d} y1={y(m)} y2={y(m)} className="rotina-grade-linha" />
                <text x={M.e - 6} y={y(m) + 3} textAnchor="end" className="rotina-eixo">{fmt(m)}</text>
              </g>
            ))}
            <text x={M.e} y={A - 6} className="rotina-eixo">{pontos[0].rotulo}</text>
            {pontos.length > 1 && <text x={L - M.d} y={A - 6} textAnchor="end" className="rotina-eixo">{ultimo(pontos)!.rotulo}</text>}
            {foco !== null && <line x1={x(foco)} x2={x(foco)} y1={M.c} y2={A - M.b} className="rotina-mira" />}
            <path d={linha} className="rotina-serie" />
            {pontos.map((p, i) => (
              <g key={p.rotulo + i}>
                <circle cx={x(i)} cy={y(p.valor)} r={p.destaque ? 4.5 : 3.5} className={p.destaque ? "rotina-marca is-recorde" : "rotina-marca"} />
                {/* Alvo maior que a marca, para o mouse e o teclado. */}
                <rect x={x(i) - 10} y={M.c} width={20} height={A - M.c - M.b} fill="transparent" tabIndex={0}
                  aria-label={`${p.rotulo}: ${fmt(p.valor)} ${unidade}${p.destaque ? ", recorde" : ""}`}
                  onMouseEnter={() => setFoco(i)} onFocus={() => setFoco(i)} onBlur={() => setFoco(null)} />
              </g>
            ))}
          </svg>
          {atual && foco !== null && (
            <div className="rotina-dica" style={{ left: `${(x(foco) / L) * 100}%` }} role="status">
              <strong>{fmt(atual.valor)} {unidade}</strong><span>{atual.rotulo}{atual.detalhe ? ` · ${atual.detalhe}` : ""}</span>
              {atual.destaque && <span className="rotina-ok">Recorde</span>}
            </div>
          )}
        </div>
      )}
    </figure>
  );
}

/** Barras verticais (contagem por período). `meta` desenha uma linha de referência. */
export function GraficoDeBarras({ titulo, unidade, pontos, meta }: { titulo: string; unidade: string; pontos: PontoDoGrafico[]; meta?: number }) {
  const [foco, setFoco] = useState<number | null>(null);
  const [tabela, setTabela] = useState(false);
  if (pontos.length === 0) return <p className="rotina-vazio">Sem registros ainda.</p>;
  const { topo, marcas } = escala([...pontos.map((p) => p.valor), meta ?? 0]);
  const largura = (L - M.e - M.d) / pontos.length;
  const y = (v: number) => M.c + (A - M.c - M.b) * (1 - v / topo);
  const atual = foco === null ? null : pontos[foco];
  return (
    <figure className="rotina-grafico">
      <figcaption>{titulo} <small className="rotina-muted">({unidade})</small>
        <button type="button" className="rotina-mini" onClick={() => setTabela((t) => !t)}>{tabela ? "Ver gráfico" : "Ver tabela"}</button></figcaption>
      {tabela ? <Tabela titulo={titulo} unidade={unidade} pontos={pontos} /> : (
        <div className="rotina-grafico-area">
          <svg viewBox={`0 0 ${L} ${A}`} role="img" aria-label={`${titulo}: ${pontos.map((p) => `${p.rotulo} ${fmt(p.valor)}`).join(", ")}`}
            onMouseLeave={() => setFoco(null)}>
            {marcas.map((m) => (
              <g key={m}>
                <line x1={M.e} x2={L - M.d} y1={y(m)} y2={y(m)} className="rotina-grade-linha" />
                <text x={M.e - 6} y={y(m) + 3} textAnchor="end" className="rotina-eixo">{fmt(m)}</text>
              </g>
            ))}
            {pontos.map((p, i) => {
              const bx = M.e + i * largura + 1, bw = Math.max(2, largura - 2), topoDaBarra = y(p.valor), base = A - M.b;
              const h = Math.max(0, base - topoDaBarra), r = Math.min(4, bw / 2, h);
              return (
                <g key={p.rotulo + i}>
                  {h > 0 && <path className={`rotina-barra-marca${foco === i ? " is-foco" : ""}`}
                    d={`M${bx},${base} V${topoDaBarra + r} Q${bx},${topoDaBarra} ${bx + r},${topoDaBarra} H${bx + bw - r} Q${bx + bw},${topoDaBarra} ${bx + bw},${topoDaBarra + r} V${base} Z`} />}
                  <rect x={bx - 1} y={M.c} width={largura} height={A - M.c - M.b} fill="transparent" tabIndex={0}
                    aria-label={`${p.rotulo}: ${fmt(p.valor)} ${unidade}`} onMouseEnter={() => setFoco(i)} onFocus={() => setFoco(i)} onBlur={() => setFoco(null)} />
                </g>
              );
            })}
            {meta ? <line x1={M.e} x2={L - M.d} y1={y(meta)} y2={y(meta)} className="rotina-meta-linha" /> : null}
            <text x={M.e} y={A - 6} className="rotina-eixo">{pontos[0].rotulo}</text>
            <text x={L - M.d} y={A - 6} textAnchor="end" className="rotina-eixo">{ultimo(pontos)!.rotulo}</text>
          </svg>
          {atual && foco !== null && (
            <div className="rotina-dica" style={{ left: `${((M.e + (foco + 0.5) * largura) / L) * 100}%` }} role="status">
              <strong>{fmt(atual.valor)} {unidade}</strong><span>{atual.rotulo}</span>
            </div>
          )}
          {meta ? <p className="rotina-muted rotina-nota">Linha tracejada: meta de {fmt(meta)} por semana.</p> : null}
        </div>
      )}
    </figure>
  );
}
