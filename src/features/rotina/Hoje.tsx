// Fork (Rotina): Hoje — a linha do tempo do dia (blocos, tarefas com hora, refeições, treino), com
// "agora" e "próximo" em destaque, marcar feito/pular num toque, água e o treino do dia ao lado.
import { useState } from "react";
import { Button } from "../../components/ui";
import { formatarLitros, fracao } from "./dominio";
import { operar } from "./rotinaService";
import type { Dia, ItemDoDia } from "./tipos";

const ROTULO_DO_TIPO: Record<string, string> = { bloco: "Bloco", treino: "Treino", tarefa: "Tarefa", refeicao: "Refeição" };

function Item({ item, destaque, onMarcar }: { item: ItemDoDia; destaque: "agora" | "proximo" | null; onMarcar: (estado: "feito" | "pulado" | "nada") => void }) {
  const feito = item.estado === "feito";
  return (
    <li className={`rotina-item is-${item.tipo}${item.estado ? ` is-${item.estado}` : ""}${destaque ? ` is-${destaque}` : ""}`}>
      <span className="rotina-item-hora">{item.inicio || "—"}{item.fim && item.fim !== item.inicio ? <small>{item.fim}</small> : null}</span>
      <span className="rotina-item-faixa" style={{ background: item.cor }} aria-hidden />
      <div className="rotina-item-corpo">
        <div className="rotina-item-linha">
          {destaque && <span className={`rotina-selo is-${destaque}`}>{destaque === "agora" ? "Agora" : "A seguir"}</span>}
          <strong className="rotina-item-titulo">{item.titulo}</strong>
        </div>
        <span className="rotina-item-detalhe">{ROTULO_DO_TIPO[item.tipo]}{item.detalhe ? ` · ${item.detalhe}` : ""}</span>
      </div>
      <div className="rotina-item-acoes">
        <button type="button" className={`rotina-check${feito ? " is-on" : ""}`} aria-pressed={feito}
          aria-label={feito ? `Desmarcar ${item.titulo}` : `Marcar ${item.titulo} como feito`} onClick={() => onMarcar(feito ? "nada" : "feito")}>
          <svg viewBox="0 0 16 16" aria-hidden><path d="M3.5 8.5l3 3 6-7" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" /></svg>
        </button>
        {!item.estado && item.tipo !== "tarefa" && (
          <button type="button" className="rotina-mini" onClick={() => onMarcar("pulado")} aria-label={`Pular ${item.titulo}`}>Pular</button>
        )}
        {item.estado === "pulado" && <button type="button" className="rotina-mini" onClick={() => onMarcar("nada")}>Desfazer</button>}
      </div>
    </li>
  );
}

export function Hoje({ dia, onMudou, onTreino, onErro }: { dia: Dia; onMudou: () => void; onTreino: () => void; onErro: (e: string) => void }) {
  const [ocupado, setOcupado] = useState(false);
  const agir = async (f: () => Promise<unknown>) => {
    setOcupado(true);
    try { await f(); onMudou(); } catch (e) { onErro(e instanceof Error ? e.message : "Não deu certo."); } finally { setOcupado(false); }
  };
  const marcar = (item: ItemDoDia, estado: "feito" | "pulado" | "nada") => agir(() => operar("marcar", { data: dia.data, item: item.id, estado }));
  const agua = fracao(dia.agua.ml, dia.agua.meta_ml);
  const feitos = dia.itens.filter((i) => i.estado === "feito").length;

  return (
    <div className="rotina-hoje">
      <section className="rotina-linha-do-tempo" aria-label="Linha do tempo de hoje">
        <header className="rotina-secao-topo">
          <h2>Hoje, {dia.dia_da_semana}</h2>
          <span className="rotina-muted">{feitos} de {dia.itens.length} feitos</span>
        </header>
        {dia.foco && <p className="rotina-foco"><span>Foco da semana</span>{dia.foco}</p>}
        {dia.itens.length === 0 ? (
          <p className="rotina-vazio">Nada marcado para hoje. Use “Planejar a semana” para montar sua rotina com o agente.</p>
        ) : (
          <ol className="rotina-itens">
            {dia.itens.map((item) => (
              <Item key={item.id} item={item} destaque={item.id === dia.agora ? "agora" : item.id === dia.proximo ? "proximo" : null}
                onMarcar={(estado) => void marcar(item, estado)} />
            ))}
          </ol>
        )}
        {dia.sem_hora.length > 0 && (
          <>
            <h3 className="rotina-subtitulo">Sem hora e atrasadas</h3>
            <ol className="rotina-itens">
              {dia.sem_hora.map((item) => <Item key={item.id} item={item} destaque={null} onMarcar={(estado) => void marcar(item, estado)} />)}
            </ol>
          </>
        )}
      </section>

      <aside className="rotina-lado">
        <section className="rotina-cartao" aria-label="Água">
          <header className="rotina-secao-topo"><h3>Água</h3><span className="rotina-muted">{formatarLitros(dia.agua.ml)} de {formatarLitros(dia.agua.meta_ml)}</span></header>
          <div className="rotina-barra" role="progressbar" aria-valuemin={0} aria-valuemax={dia.agua.meta_ml} aria-valuenow={dia.agua.ml}
            aria-label="Água bebida hoje"><span style={{ width: `${agua * 100}%` }} /></div>
          <div className="rotina-linha-botoes">
            <Button size="sm" disabled={ocupado} onClick={() => void agir(() => operar("registrar_agua", { data: dia.data }))}>+1 copo ({dia.agua.copo_ml} ml)</Button>
            <Button size="sm" variant="ghost" disabled={ocupado || dia.agua.ml === 0} aria-label="Tirar um copo"
              onClick={() => void agir(() => operar("registrar_agua", { data: dia.data, copos: -1 }))}>−1</Button>
          </div>
        </section>

        <section className="rotina-cartao" aria-label="Treino de hoje">
          <header className="rotina-secao-topo"><h3>Treino</h3>
            {dia.treino.sessao?.concluida && <span className="rotina-ok">Concluído</span>}</header>
          {dia.treino.ficha ? (
            <p className="rotina-texto">Ficha {dia.treino.ficha.nome}{dia.treino.ficha.titulo ? ` · ${dia.treino.ficha.titulo}` : ""} ({dia.treino.ficha.exercicios} exercícios)</p>
          ) : <p className="rotina-muted">Sem ficha cadastrada ainda.</p>}
          {dia.treino.sessao && <p className="rotina-muted">{dia.treino.sessao.series} séries registradas hoje</p>}
          <Button size="sm" onClick={onTreino}>{dia.treino.sessao ? "Continuar o treino" : "Registrar treino"}</Button>
        </section>

        <section className="rotina-cartao" aria-label="Alimentação de hoje">
          <header className="rotina-secao-topo"><h3>Alimentação</h3><span className="rotina-muted">{dia.alimentacao.refeicoes} refeições</span></header>
          {(dia.alimentacao.kcal > 0 || dia.alimentacao.proteina_g > 0) && (
            <p className="rotina-texto">{Math.round(dia.alimentacao.kcal)} kcal · {Math.round(dia.alimentacao.proteina_g)} g de proteína</p>
          )}
        </section>

        {dia.metas.length > 0 && (
          <section className="rotina-cartao" aria-label="Metas da semana">
            <header className="rotina-secao-topo"><h3>Metas da semana</h3></header>
            <ul className="rotina-metas">
              {dia.metas.map((m) => {
                const f = m.concluida ? 1 : m.alvo ? fracao(m.progresso, m.alvo) : 0;
                return (
                  <li key={m.id}>
                    <span className="rotina-meta-texto">{m.texto}</span>
                    <span className="rotina-muted">{m.concluida ? "feita" : m.alvo ? `${m.progresso}/${m.alvo}${m.unidade ? ` ${m.unidade}` : ""}` : ""}</span>
                    <div className="rotina-barra is-fina" aria-hidden><span style={{ width: `${f * 100}%` }} /></div>
                  </li>
                );
              })}
            </ul>
          </section>
        )}
      </aside>
    </div>
  );
}
