// Fork (Rotina): Alimentação — água em um toque, refeição do plano em um toque ("Comi"), refeição livre
// em texto (kcal e proteína opcionais) e o histórico dos últimos dias.
import { useEffect, useState } from "react";
import { Button } from "../../components/ui";
import { GraficoDeBarras } from "./Graficos";
import { dataCurta, formatarLitros, fracao } from "./dominio";
import { lerAlimentacao, operar } from "./rotinaService";
import type { Alimentacao as Dados } from "./tipos";

function PlanoAlimentar({ dados, onMudou, onErro }: { dados: Dados; onMudou: () => void; onErro: (e: string) => void }) {
  const [plano, setPlano] = useState(dados.plano.map((r) => ({ ...r })));
  const [meta, setMeta] = useState(dados.meta_agua_ml);
  const [copo, setCopo] = useState(dados.copo_ml);
  const salvar = () => void operar("salvar_plano_alimentar", {
    refeicoes: plano.filter((r) => r.nome.trim()).map(({ id, nome, hora, sugestao }) => ({ id, nome, hora, sugestao })), meta_agua_ml: meta, copo_ml: copo,
  }).then(onMudou, (e: Error) => onErro(e.message));
  return (
    <div className="rotina-fichas">
      {plano.map((r, i) => (
        <div key={r.id || i} className="rotina-form">
          <label>Refeição<input value={r.nome} onChange={(e) => setPlano(plano.map((x, j) => j === i ? { ...x, nome: e.target.value } : x))} /></label>
          <label>Hora<input type="time" value={r.hora} onChange={(e) => setPlano(plano.map((x, j) => j === i ? { ...x, hora: e.target.value } : x))} /></label>
          <label className="rotina-form-largo">O que comer<input value={r.sugestao} onChange={(e) => setPlano(plano.map((x, j) => j === i ? { ...x, sugestao: e.target.value } : x))} /></label>
          <button type="button" className="rotina-mini" onClick={() => setPlano(plano.filter((_, j) => j !== i))}>Tirar</button>
        </div>
      ))}
      <div className="rotina-linha-botoes">
        <Button size="sm" variant="ghost" onClick={() => setPlano([...plano, { id: "", nome: "", hora: "", sugestao: "", kcal: null, proteina_g: null }])}>+ Refeição</Button>
        <label>Meta de água (ml)<input type="number" className="rotina-numero" min={0} max={10000} step={250} value={meta} onChange={(e) => setMeta(Number(e.target.value))} /></label>
        <label>Copo (ml)<input type="number" className="rotina-numero" min={50} max={2000} step={50} value={copo} onChange={(e) => setCopo(Number(e.target.value))} /></label>
        <Button size="sm" onClick={salvar}>Salvar plano</Button>
      </div>
    </div>
  );
}

export function Alimentacao({ onErro }: { onErro: (e: string) => void }) {
  const [dados, setDados] = useState<Dados | null>(null);
  const [texto, setTexto] = useState("");
  const [kcal, setKcal] = useState("");
  const [proteina, setProteina] = useState("");
  const [ocupado, setOcupado] = useState(false);
  const [editar, setEditar] = useState(false);
  const recarregar = () => lerAlimentacao(undefined, 14).then(setDados, (e: Error) => onErro(e.message));
  useEffect(() => { void recarregar(); }, []); // eslint-disable-line react-hooks/exhaustive-deps
  if (!dados) return <p className="rotina-vazio">Carregando a alimentação…</p>;

  const agir = async (f: () => Promise<unknown>) => {
    setOcupado(true);
    try { await f(); await recarregar(); } catch (e) { onErro(e instanceof Error ? e.message : "Não deu certo."); } finally { setOcupado(false); }
  };
  const feitas = new Set(dados.dia.refeicoes.map((r) => r.planejada).filter(Boolean));
  const agua = fracao(dados.dia.agua_ml, dados.meta_agua_ml);
  const numeroOpcional = (v: string) => { const n = Number(v.replace(",", ".")); return v.trim() && Number.isFinite(n) ? n : undefined; };

  return (
    <div className="rotina-treino">
      <section className="rotina-sessao">
        <div className="rotina-cartao rotina-agua">
          <header className="rotina-secao-topo"><h2>Água</h2><span className="rotina-muted">{formatarLitros(dados.dia.agua_ml)} de {formatarLitros(dados.meta_agua_ml)}</span></header>
          <div className="rotina-barra is-grossa" role="progressbar" aria-label="Água bebida hoje" aria-valuemin={0} aria-valuemax={dados.meta_agua_ml} aria-valuenow={dados.dia.agua_ml}>
            <span style={{ width: `${agua * 100}%` }} /></div>
          <div className="rotina-linha-botoes">
            <button type="button" className="rotina-serie-botao" disabled={ocupado} onClick={() => void agir(() => operar("registrar_agua", {}))}>+1 copo ({dados.copo_ml} ml)</button>
            <Button size="sm" variant="ghost" disabled={ocupado} onClick={() => void agir(() => operar("registrar_agua", { ml: 500 }))}>+500 ml</Button>
            <Button size="sm" variant="ghost" disabled={ocupado || dados.dia.agua_ml === 0} aria-label="Tirar um copo" onClick={() => void agir(() => operar("registrar_agua", { copos: -1 }))}>−1</Button>
          </div>
        </div>

        <div className="rotina-cartao">
          <header className="rotina-secao-topo"><h2>Refeições de hoje</h2>
            <span className="rotina-muted">{Math.round(dados.totais.kcal)} kcal · {Math.round(dados.totais.proteina_g)} g proteína</span></header>
          <ul className="rotina-itens">
            {dados.plano.map((r) => (
              <li key={r.id} className={`rotina-item is-refeicao${feitas.has(r.id) ? " is-feito" : ""}`}>
                <span className="rotina-item-hora">{r.hora}</span>
                <div className="rotina-item-corpo"><strong className="rotina-item-titulo">{r.nome}</strong>
                  <span className="rotina-item-detalhe">{dados.dia.refeicoes.find((x) => x.planejada === r.id)?.texto || r.sugestao || "Sem plano"}</span></div>
                {!feitas.has(r.id) && (
                  <button type="button" className="rotina-mini" disabled={ocupado} onClick={() => void agir(() => operar("registrar_refeicao", { planejada: r.id }))}>Comi</button>
                )}
              </li>
            ))}
            {dados.dia.refeicoes.filter((r) => !r.planejada).map((r) => (
              <li key={r.id} className="rotina-item is-refeicao is-feito">
                <span className="rotina-item-hora">{r.hora}</span>
                <div className="rotina-item-corpo"><strong className="rotina-item-titulo">{r.nome}</strong>
                  <span className="rotina-item-detalhe">{r.texto}{r.kcal ? ` · ${r.kcal} kcal` : ""}{r.proteina_g ? ` · ${r.proteina_g} g prot.` : ""}</span></div>
                <button type="button" className="rotina-mini" aria-label={`Apagar ${r.texto}`} onClick={() => void agir(() => operar("remover_refeicao", { id: r.id }))}>Apagar</button>
              </li>
            ))}
          </ul>
          <form className="rotina-form" onSubmit={(e) => {
            e.preventDefault();
            if (!texto.trim()) return;
            void agir(() => operar("registrar_refeicao", { texto, kcal: numeroOpcional(kcal), proteina_g: numeroOpcional(proteina) }))
              .then(() => { setTexto(""); setKcal(""); setProteina(""); });
          }}>
            <label className="rotina-form-largo">O que você comeu?<input value={texto} onChange={(e) => setTexto(e.target.value)} placeholder="Pão com ovo e café" /></label>
            <label>kcal<input inputMode="numeric" className="rotina-numero" value={kcal} onChange={(e) => setKcal(e.target.value)} placeholder="opcional" /></label>
            <label>Proteína (g)<input inputMode="numeric" className="rotina-numero" value={proteina} onChange={(e) => setProteina(e.target.value)} placeholder="opcional" /></label>
            <Button size="sm" type="submit" disabled={ocupado || !texto.trim()}>Registrar</Button>
          </form>
        </div>
        <div className="rotina-cartao">
          <button type="button" className="rotina-expandir" aria-expanded={editar} onClick={() => setEditar((v) => !v)}>Plano do dia e meta de água</button>
          {editar && <PlanoAlimentar dados={dados} onMudou={() => void recarregar()} onErro={onErro} />}
        </div>
      </section>
      <section className="rotina-lado">
        <div className="rotina-cartao">
          <GraficoDeBarras titulo="Água por dia" unidade="ml" meta={dados.meta_agua_ml || undefined}
            pontos={dados.historico.slice().reverse().map((d) => ({ rotulo: dataCurta(d.data), valor: d.agua_ml }))} />
        </div>
        <div className="rotina-cartao">
          <h3 className="rotina-subtitulo">Últimos dias</h3>
          <table className="rotina-tabela">
            <thead><tr><th scope="col">Dia</th><th scope="col">Refeições</th><th scope="col">kcal</th><th scope="col">Proteína</th><th scope="col">Água</th></tr></thead>
            <tbody>
              {dados.historico.map((d) => (
                <tr key={d.data}><td>{dataCurta(d.data)}</td><td>{d.refeicoes}</td><td>{d.kcal ? Math.round(d.kcal) : "—"}</td>
                  <td>{d.proteina_g ? `${Math.round(d.proteina_g)} g` : "—"}</td><td>{formatarLitros(d.agua_ml)}</td></tr>
              ))}
            </tbody>
          </table>
        </div>
      </section>
    </div>
  );
}
