// Fork (Rotina): Treino — registro rápido no intervalo da academia (a carga e as repetições já vêm da
// última série; "+ Série" grava em um toque), histórico, frequência por semana e evolução por exercício.
import { useEffect, useMemo, useState } from "react";
import { Button } from "../../components/ui";
import { GraficoDeBarras, GraficoDeLinha } from "./Graficos";
import {
  ajustarCarga, ajustarRepeticoes, dataCurta, formatarKg, formatarNumero, lerLinhaDeExercicio, mesmaChave, ultimaVez, ultimo, valoresIniciais,
} from "./dominio";
import { lerTreino, operar } from "./rotinaService";
import type { ExercicioDaFicha, Serie, Treino as DadosDoTreino } from "./tipos";

function resumoDasSeries(series: Serie[]): string {
  if (!series.length) return "";
  const iguais = series.every((s) => s.carga_kg === series[0].carga_kg && s.repeticoes === series[0].repeticoes);
  return iguais ? `${series.length}× ${formatarKg(series[0].carga_kg)} × ${series[0].repeticoes}`
    : series.map((s) => `${formatarNumero(s.carga_kg)}×${s.repeticoes}`).join(", ");
}

function LinhaDoExercicio({ exercicio, daFicha, hoje, anteriores, ocupado, onSerie, onRemover }: {
  exercicio: string; daFicha?: ExercicioDaFicha; hoje: { serie: Serie; indice: number }[]; anteriores: Serie[]; ocupado: boolean;
  onSerie: (carga: number, reps: number) => void; onRemover: (indice: number) => void;
}) {
  const inicial = valoresIniciais(hoje.map((h) => h.serie), anteriores, daFicha?.carga_kg ?? null, daFicha?.repeticoes ?? "");
  const [carga, setCarga] = useState(inicial.carga);
  const [reps, setReps] = useState(inicial.reps);
  // Nova série registrada: os valores seguem a última (normalmente a próxima é igual).
  const ultima = ultimo(hoje)?.serie;
  useEffect(() => { if (ultima) { setCarga(ultima.carga_kg); setReps(ultima.repeticoes); } }, [ultima?.carga_kg, ultima?.repeticoes, hoje.length]); // eslint-disable-line react-hooks/exhaustive-deps
  const alvo = daFicha ? `${daFicha.series}× ${daFicha.repeticoes}` : "";
  return (
    <li className="rotina-exercicio">
      <div className="rotina-exercicio-topo">
        <strong>{exercicio}</strong>
        <span className="rotina-muted">{[alvo && `Meta ${alvo}`, anteriores.length ? `Última vez ${resumoDasSeries(anteriores)}` : ""].filter(Boolean).join(" · ")}</span>
      </div>
      <div className="rotina-registro">
        <div className="rotina-passo" role="group" aria-label={`Carga de ${exercicio}`}>
          <button type="button" aria-label="Menos 2,5 kg" onClick={() => setCarga((c) => ajustarCarga(c, -1))}>−</button>
          <input inputMode="decimal" aria-label={`Carga em kg de ${exercicio}`} value={formatarNumero(carga)}
            onChange={(e) => { const n = Number(e.target.value.replace(",", ".")); if (Number.isFinite(n)) setCarga(Math.max(0, Math.min(1000, n))); }} />
          <span>kg</span>
          <button type="button" aria-label="Mais 2,5 kg" onClick={() => setCarga((c) => ajustarCarga(c, 1))}>+</button>
        </div>
        <div className="rotina-passo" role="group" aria-label={`Repetições de ${exercicio}`}>
          <button type="button" aria-label="Menos uma repetição" onClick={() => setReps((r) => ajustarRepeticoes(r, -1))}>−</button>
          <input inputMode="numeric" aria-label={`Repetições de ${exercicio}`} value={reps}
            onChange={(e) => { const n = Number.parseInt(e.target.value, 10); setReps(Number.isFinite(n) ? ajustarRepeticoes(n, 0) : 0); }} />
          <span>rep</span>
          <button type="button" aria-label="Mais uma repetição" onClick={() => setReps((r) => ajustarRepeticoes(r, 1))}>+</button>
        </div>
        <button type="button" className="rotina-serie-botao" disabled={ocupado || reps <= 0} onClick={() => onSerie(carga, reps)}>+ Série</button>
      </div>
      {hoje.length > 0 && (
        <ol className="rotina-series" aria-label={`Séries de hoje de ${exercicio}`}>
          {hoje.map(({ serie, indice }, n) => (
            <li key={indice}>
              <span>{n + 1}. {formatarNumero(serie.carga_kg)} kg × {serie.repeticoes}</span>
              <button type="button" aria-label={`Apagar a série ${n + 1} de ${exercicio}`} onClick={() => onRemover(indice)}>×</button>
            </li>
          ))}
        </ol>
      )}
    </li>
  );
}

function EditarFichas({ dados, onMudou, onErro }: { dados: DadosDoTreino; onMudou: () => void; onErro: (e: string) => void }) {
  const [nome, setNome] = useState("");
  const [titulo, setTitulo] = useState("");
  const [linhas, setLinhas] = useState("");
  const [meta, setMeta] = useState(dados.meta_semanal);
  const salvar = async () => {
    const exercicios = linhas.split("\n").map(lerLinhaDeExercicio).filter((e): e is NonNullable<typeof e> => e !== null);
    try {
      await operar("salvar_ficha", { nome, titulo, exercicios });
      setNome(""); setTitulo(""); setLinhas(""); onMudou();
    } catch (e) { onErro(e instanceof Error ? e.message : "Não consegui salvar a ficha."); }
  };
  return (
    <div className="rotina-fichas">
      <ul className="rotina-lista">
        {dados.fichas.map((f) => (
          <li key={f.id} className="rotina-linha-botoes">
            <strong>Ficha {f.nome}</strong><span className="flex-1 rotina-muted">{f.titulo} · {f.exercicios.map((e) => e.nome).join(", ")}</span>
            <button type="button" className="rotina-mini" onClick={() => {
              if (window.confirm(`Apagar a ficha ${f.nome}? O histórico dos treinos fica.`)) void operar("remover_ficha", { id: f.id }).then(onMudou, (e: Error) => onErro(e.message));
            }}>Apagar</button>
          </li>
        ))}
      </ul>
      <form className="rotina-form" onSubmit={(e) => { e.preventDefault(); if (nome.trim()) void salvar(); }}>
        <label>Ficha<input value={nome} onChange={(e) => setNome(e.target.value)} placeholder="A" maxLength={20} /></label>
        <label>Grupo<input value={titulo} onChange={(e) => setTitulo(e.target.value)} placeholder="Peito e tríceps" /></label>
        <label className="rotina-form-largo">Exercícios (um por linha)
          <textarea rows={4} value={linhas} onChange={(e) => setLinhas(e.target.value)} placeholder={"Supino reto 4x8-10 60kg\nCrucifixo 3x12"} /></label>
        <Button size="sm" type="submit" disabled={!nome.trim()}>Salvar ficha</Button>
      </form>
      <label className="rotina-linha-botoes">Treinos por semana
        <input type="number" min={0} max={14} value={meta} className="rotina-numero" onChange={(e) => setMeta(Number(e.target.value))}
          onBlur={() => void operar("ajustar_treino", { meta_semanal: meta }).then(onMudou, (e: Error) => onErro(e.message))} />
      </label>
    </div>
  );
}

export function Treino({ fichaEscolhida, onFicha, onErro }: { fichaEscolhida: string; onFicha: (id: string) => void; onErro: (e: string) => void }) {
  const [dados, setDados] = useState<DadosDoTreino | null>(null);
  const [exercicio, setExercicio] = useState("");
  const [ocupado, setOcupado] = useState(false);
  const [aviso, setAviso] = useState("");
  const [extra, setExtra] = useState("");
  const [extras, setExtras] = useState<string[]>([]);
  const [verFichas, setVerFichas] = useState(false);
  const recarregar = () => lerTreino(exercicio || undefined, 12).then(setDados, (e: Error) => onErro(e.message));
  useEffect(() => { void recarregar(); }, [exercicio]); // eslint-disable-line react-hooks/exhaustive-deps

  const sessao = dados?.sessao_de_hoje ?? null;
  const fichaId = fichaEscolhida || sessao?.ficha || dados?.proxima_ficha || "";
  const ficha = dados?.fichas.find((f) => f.id === fichaId);
  const nomes = useMemo(() => {
    const lista = [...(ficha?.exercicios.map((e) => e.nome) ?? [])];
    for (const n of [...(sessao?.series.map((s) => s.exercicio) ?? []), ...extras]) if (!lista.some((x) => mesmaChave(x, n))) lista.push(n);
    return lista;
  }, [ficha, sessao, extras]);

  if (!dados) return <p className="rotina-vazio">Carregando o treino…</p>;

  const registrar = async (nome: string, carga: number, reps: number) => {
    setOcupado(true);
    try {
      const r = await operar<{ recorde: boolean; serie_do_exercicio: number }>("registrar_serie", {
        exercicio: nome, carga_kg: carga, repeticoes: reps, ...(fichaId ? { ficha: fichaId } : {}),
      });
      setAviso(r.recorde ? `Recorde em ${nome}: ${formatarKg(carga)}!` : `${nome}: série ${r.serie_do_exercicio} registrada.`);
      await recarregar();
    } catch (e) { onErro(e instanceof Error ? e.message : "Não consegui registrar."); } finally { setOcupado(false); }
  };
  const remover = async (indice: number) => {
    if (!sessao) return;
    try { await operar("remover_serie", { sessao: sessao.id, indice }); await recarregar(); }
    catch (e) { onErro(e instanceof Error ? e.message : "Não consegui apagar."); }
  };
  const evolucao = dados.evolucao;

  return (
    <div className="rotina-treino">
      <section className="rotina-cartao rotina-sessao" aria-label="Treino de hoje">
        <header className="rotina-secao-topo">
          <h2>Treino de hoje</h2>
          {sessao?.concluida ? <span className="rotina-ok">Concluído</span>
            : sessao ? <Button size="sm" variant="ghost" onClick={() => void operar("concluir_treino", { sessao: sessao.id }).then(recarregar, (e: Error) => onErro(e.message))}>Concluir treino</Button> : null}
        </header>
        <div className="rotina-fichas-escolha" role="radiogroup" aria-label="Ficha">
          {dados.fichas.map((f) => (
            <button key={f.id} type="button" role="radio" aria-checked={f.id === fichaId} className={`rotina-ficha${f.id === fichaId ? " is-on" : ""}`}
              onClick={() => onFicha(f.id)}>
              {f.nome}{f.titulo ? <small>{f.titulo}</small> : null}{f.id === dados.proxima_ficha && !sessao ? <em>sugerida</em> : null}
            </button>
          ))}
          <button type="button" className="rotina-mini" onClick={() => setVerFichas((v) => !v)} aria-expanded={verFichas}>{dados.fichas.length ? "Editar fichas" : "Criar ficha"}</button>
        </div>
        {verFichas && <EditarFichas dados={dados} onMudou={() => void recarregar()} onErro={onErro} />}
        <p className="rotina-aviso" role="status" aria-live="polite">{aviso}</p>
        {nomes.length === 0 ? <p className="rotina-vazio">Crie uma ficha ou digite um exercício abaixo.</p> : (
          <ul className="rotina-exercicios">
            {nomes.map((nome) => (
              <LinhaDoExercicio key={nome} exercicio={nome} daFicha={ficha?.exercicios.find((e) => mesmaChave(e.nome, nome))}
                hoje={(sessao?.series ?? []).map((serie, indice) => ({ serie, indice })).filter((x) => mesmaChave(x.serie.exercicio, nome))}
                anteriores={ultimaVez(dados.sessoes, nome, dados.hoje)} ocupado={ocupado}
                onSerie={(c, r) => void registrar(nome, c, r)} onRemover={(i) => void remover(i)} />
            ))}
          </ul>
        )}
        <form className="rotina-linha-botoes" onSubmit={(e) => { e.preventDefault(); const n = extra.trim(); if (n) { setExtras((x) => [...x, n]); setExtra(""); } }}>
          <input className="rotina-entrada" value={extra} onChange={(e) => setExtra(e.target.value)} placeholder="Outro exercício (ex.: Prancha)" aria-label="Outro exercício" />
          <Button size="sm" type="submit" variant="ghost" disabled={!extra.trim()}>Adicionar</Button>
        </form>
      </section>

      <section className="rotina-lado">
        <div className="rotina-cartao">
          <GraficoDeBarras titulo="Treinos por semana" unidade="treinos" meta={dados.meta_semanal || undefined}
            pontos={dados.frequencia.map((f) => ({ rotulo: dataCurta(f.semana), valor: f.treinos }))} />
        </div>
        <div className="rotina-cartao">
          <label className="rotina-linha-botoes">Evolução de
            <select value={exercicio} onChange={(e) => setExercicio(e.target.value)} aria-label="Exercício">
              <option value="">escolha um exercício</option>
              {dados.exercicios.map((e) => <option key={e.nome} value={e.nome}>{e.nome}</option>)}
            </select>
          </label>
          {evolucao && exercicio ? (
            <>
              <div className="rotina-numeros is-compacto">
                <div><strong>{formatarKg(evolucao.recorde_carga)}</strong><span>maior carga</span></div>
                <div><strong>{formatarKg(evolucao.recorde_um_rm)}</strong><span>1RM estimado</span></div>
                <div><strong>{formatarNumero(evolucao.recorde_volume)}</strong><span>maior volume (kg)</span></div>
              </div>
              <GraficoDeLinha titulo="Carga máxima por treino" unidade="kg"
                pontos={evolucao.pontos.map((p) => ({ rotulo: dataCurta(p.data), valor: p.carga_max, destaque: p.recorde, detalhe: `${p.series} séries, até ${p.repeticoes_max} rep` }))} />
              <GraficoDeLinha titulo="Volume por treino" unidade="kg"
                pontos={evolucao.pontos.map((p) => ({ rotulo: dataCurta(p.data), valor: p.volume }))} />
            </>
          ) : <p className="rotina-muted">Escolha um exercício para ver a evolução.</p>}
        </div>
        <div className="rotina-cartao">
          <h3 className="rotina-subtitulo">Histórico</h3>
          {dados.sessoes.length === 0 ? <p className="rotina-vazio">Nenhum treino registrado.</p> : (
            <ul className="rotina-lista">
              {dados.sessoes.slice(0, 12).map((s) => {
                const f = dados.fichas.find((x) => x.id === s.ficha);
                return (
                  <li key={s.id}>
                    <strong>{dataCurta(s.data)}</strong> {f ? `Ficha ${f.nome}` : "Treino livre"}
                    <span className="rotina-muted"> · {s.series.length} séries · {formatarNumero(s.volume ?? 0)} kg de volume{s.concluida ? "" : " · aberto"}</span>
                  </li>
                );
              })}
            </ul>
          )}
        </div>
      </section>
    </div>
  );
}
