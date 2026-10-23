// Fork (Rotina): Semana — grade de segunda a domingo com a cor de cada área (sempre com o nome escrito:
// cor nunca sozinha), foco, metas, progresso, imprevistos registrados e os blocos fixos.
import { useState } from "react";
import { Button } from "../../components/ui";
import { dataCurta, fracao } from "./dominio";
import { operar } from "./rotinaService";
import { NOMES_CURTOS_DOS_DIAS, type SemanaDaRotina } from "./tipos";

const TIPOS = ["aula", "trabalho", "estudo", "academia", "refeicao", "sono", "deslocamento", "pessoal", "outro"];
const ROTULO_DO_TIPO: Record<string, string> = {
  aula: "Aula", trabalho: "Trabalho", estudo: "Estudo", academia: "Academia", refeicao: "Refeição", sono: "Sono",
  deslocamento: "Deslocamento", pessoal: "Pessoal", outro: "Outro",
};

function NovoBloco({ areas, onSalvar }: { areas: SemanaDaRotina["areas"]; onSalvar: (b: Record<string, unknown>) => Promise<void> }) {
  const [b, setB] = useState({ titulo: "", area: areas[0]?.id ?? "", tipo: "aula", dias: [] as number[], inicio: "", fim: "" });
  const valido = b.titulo.trim() && b.inicio && b.dias.length > 0;
  return (
    <form className="rotina-form" onSubmit={(e) => { e.preventDefault(); if (valido) void onSalvar(b).then(() => setB({ ...b, titulo: "", dias: [] })); }}>
      <label>Título<input value={b.titulo} onChange={(e) => setB({ ...b, titulo: e.target.value })} placeholder="Aula de Cálculo" /></label>
      <label>Área<select value={b.area} onChange={(e) => setB({ ...b, area: e.target.value })}>
        {areas.map((a) => <option key={a.id} value={a.id}>{a.nome}</option>)}</select></label>
      <label>Tipo<select value={b.tipo} onChange={(e) => setB({ ...b, tipo: e.target.value })}>
        {TIPOS.map((t) => <option key={t} value={t}>{ROTULO_DO_TIPO[t]}</option>)}</select></label>
      <label>Início<input type="time" value={b.inicio} onChange={(e) => setB({ ...b, inicio: e.target.value })} /></label>
      <label>Fim<input type="time" value={b.fim} onChange={(e) => setB({ ...b, fim: e.target.value })} /></label>
      <fieldset className="rotina-dias"><legend>Dias</legend>
        {NOMES_CURTOS_DOS_DIAS.map((nome, i) => (
          <label key={nome} className={b.dias.includes(i) ? "is-on" : ""}>
            <input type="checkbox" checked={b.dias.includes(i)}
              onChange={(e) => setB({ ...b, dias: e.target.checked ? [...b.dias, i].sort() : b.dias.filter((d) => d !== i) })} />{nome}
          </label>
        ))}
      </fieldset>
      <Button size="sm" type="submit" disabled={!valido}>Adicionar bloco fixo</Button>
    </form>
  );
}

export function Semana({ semana, onMudou, onIr, onErro }: {
  semana: SemanaDaRotina; onMudou: () => void; onIr: (dias: number) => void; onErro: (e: string) => void;
}) {
  const [verBlocos, setVerBlocos] = useState(false);
  const agir = async (f: () => Promise<unknown>) => {
    try { await f(); onMudou(); } catch (e) { onErro(e instanceof Error ? e.message : "Não deu certo."); }
  };
  const p = semana.progresso;
  const nomeDaArea = (id: string) => semana.areas.find((a) => a.id === id)?.nome ?? "";
  const domingo = semana.dias[6]?.data ?? semana.segunda;

  return (
    <div className="rotina-semana">
      <header className="rotina-secao-topo">
        <div className="rotina-linha-botoes">
          <button type="button" className="rotina-mini" aria-label="Semana anterior" onClick={() => onIr(-7)}>‹</button>
          <h2>Semana de {dataCurta(semana.segunda)} a {dataCurta(domingo)}</h2>
          <button type="button" className="rotina-mini" aria-label="Próxima semana" onClick={() => onIr(7)}>›</button>
        </div>
        <span className="rotina-muted">{semana.planejada_em ? "Planejada" : "Ainda não planejada"}</span>
      </header>

      <div className="rotina-numeros" role="list" aria-label="Progresso da semana">
        <div role="listitem"><strong>{p.itens_feitos}/{p.itens_total}</strong><span>itens feitos até hoje</span></div>
        <div role="listitem"><strong>{p.treinos}/{p.meta_treinos}</strong><span>treinos</span></div>
        <div role="listitem"><strong>{p.dias_com_agua_na_meta}</strong><span>dias com a meta de água</span></div>
        <div role="listitem"><strong>{p.metas_concluidas}/{p.metas_total}</strong><span>metas concluídas</span></div>
      </div>

      {(semana.foco || semana.metas.length > 0) && (
        <section className="rotina-cartao">
          {semana.foco && <p className="rotina-foco"><span>Foco</span>{semana.foco}</p>}
          {semana.metas.length > 0 && (
            <ul className="rotina-metas">
              {semana.metas.map((m) => (
                <li key={m.id}>
                  <label className="rotina-meta-texto">
                    <input type="checkbox" checked={m.concluida}
                      onChange={(e) => void agir(() => operar("salvar_meta", { id: m.id, semana: semana.segunda, concluida: e.target.checked }))} />
                    {m.texto}{m.area ? <small className="rotina-muted"> · {nomeDaArea(m.area)}</small> : null}
                  </label>
                  <span className="rotina-muted">{m.alvo ? `${m.progresso}/${m.alvo}${m.unidade ? ` ${m.unidade}` : ""}` : ""}</span>
                  {m.alvo ? (
                    <button type="button" className="rotina-mini" aria-label={`Somar 1 em ${m.texto}`}
                      onClick={() => void agir(() => operar("salvar_meta", { id: m.id, semana: semana.segunda, somar: 1 }))}>+1</button>
                  ) : null}
                  <div className="rotina-barra is-fina" aria-hidden><span style={{ width: `${(m.concluida ? 1 : m.alvo ? fracao(m.progresso, m.alvo) : 0) * 100}%` }} /></div>
                </li>
              ))}
            </ul>
          )}
        </section>
      )}

      <ul className="rotina-legenda" aria-label="Áreas">
        {semana.areas.map((a) => <li key={a.id}><span style={{ background: a.cor }} aria-hidden />{a.nome}</li>)}
      </ul>

      <div className="rotina-grade" role="table" aria-label="Semana por dia">
        {semana.dias.map((dia, i) => (
          <div key={dia.data} role="rowgroup" className={`rotina-coluna${dia.data === semana.hoje ? " is-hoje" : ""}`}>
            <div role="row" className="rotina-coluna-topo"><span role="columnheader">{NOMES_CURTOS_DOS_DIAS[i]} {dataCurta(dia.data)}</span></div>
            {dia.itens.filter((it) => it.tipo !== "refeicao" && it.categoria !== "sono").map((it) => (
              <div key={it.id} role="row" className={`rotina-bloco${it.estado ? ` is-${it.estado}` : ""}`} style={{ borderLeftColor: it.cor }}
                title={`${it.inicio}${it.fim ? `–${it.fim}` : ""} ${it.titulo}${it.detalhe ? ` · ${it.detalhe}` : ""}`}>
                <span role="cell" className="rotina-bloco-hora">{it.inicio}</span>
                <span role="cell" className="rotina-bloco-titulo">{it.titulo}</span>
                {it.area && <span role="cell" className="rotina-bloco-area">{nomeDaArea(it.area)}</span>}
              </div>
            ))}
            {dia.sem_hora.filter(() => dia.data === semana.hoje).length > 0 && (
              <div role="row" className="rotina-bloco is-sem-hora"><span role="cell">{dia.sem_hora.length} sem hora</span></div>
            )}
          </div>
        ))}
      </div>

      {semana.imprevistos.length > 0 && (
        <section className="rotina-cartao">
          <h3 className="rotina-subtitulo">Imprevistos da semana</h3>
          <ul className="rotina-lista">
            {semana.imprevistos.slice().reverse().map((imp) => <li key={imp.em}><span className="rotina-muted">{dataCurta(imp.data)}</span> {imp.texto}</li>)}
          </ul>
        </section>
      )}

      <section className="rotina-cartao">
        <button type="button" className="rotina-expandir" aria-expanded={verBlocos} onClick={() => setVerBlocos((v) => !v)}>
          Blocos fixos ({semana.blocos_fixos.filter((b) => !b.data).length})
        </button>
        {verBlocos && (
          <>
            <ul className="rotina-lista">
              {semana.blocos_fixos.filter((b) => !b.data).map((b) => (
                <li key={b.id} className="rotina-linha-botoes">
                  <span className="rotina-pinta" style={{ background: semana.areas.find((a) => a.id === b.area)?.cor ?? "#64748b" }} aria-hidden />
                  <span className="flex-1">{b.titulo} <small className="rotina-muted">· {b.dias.map((d) => NOMES_CURTOS_DOS_DIAS[d]).join(", ")} · {b.inicio}–{b.fim}</small></span>
                  <button type="button" className="rotina-mini" aria-label={`Remover ${b.titulo}`}
                    onClick={() => { if (window.confirm(`Remover o bloco “${b.titulo}” de todas as semanas?`)) void agir(() => operar("remover_bloco", { id: b.id })); }}>Remover</button>
                </li>
              ))}
            </ul>
            <NovoBloco areas={semana.areas} onSalvar={(b) => agir(() => operar("salvar_bloco", b))} />
          </>
        )}
      </section>
    </div>
  );
}
