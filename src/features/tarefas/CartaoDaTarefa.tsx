// Fork (Tarefas): cartão de uma tarefa (quadro, meu dia, semana) e peças pequenas reaproveitadas.
import { createContext, useContext, type CSSProperties, type KeyboardEvent, type PointerEvent } from "react";

/** Fork (Copiloto): ids que mudaram há pouco por fora (IA, copiloto, outra pessoa) — o cartão brilha. */
export const DestaquesContext = createContext<Record<string, number>>({});
import { CheckIcon, ChatIcon, ClockIcon } from "../../components/ui/PixelIcon";
import { atrasada, progressoDoChecklist, rotuloDoDia } from "./dominio";
import { categoria, ROTULO_DA_PRIORIDADE, type Pessoa, type Tarefa } from "./tipos";

export function Avatar({ pessoa, grande = false }: { pessoa?: Pessoa; grande?: boolean }) {
  if (!pessoa) return null;
  const iniciais = pessoa.nome.split(/\s+/).filter(Boolean).slice(0, 2).map((p) => p[0]?.toUpperCase()).join("");
  return <span className={`avatar${grande ? " is-grande" : ""}`} style={{ background: pessoa.cor }} title={pessoa.nome} aria-label={pessoa.nome}>{iniciais || "?"}</span>;
}

export function BotaoDeConcluir({ feita, onClick, titulo }: { feita: boolean; onClick: () => void; titulo: string }) {
  return (
    <button type="button" className={`tarefa-check${feita ? " is-feita" : ""}`} aria-pressed={feita}
      aria-label={feita ? `Reabrir "${titulo}"` : `Concluir "${titulo}"`}
      onClick={(e) => { e.stopPropagation(); onClick(); }}>
      <CheckIcon className="h-3 w-3" aria-hidden="true" />
    </button>
  );
}

export function FichaDeData({ t, hoje, comHora = true }: { t: Tarefa; hoje: string; comHora?: boolean }) {
  if (!t.data && !t.hora) return null;
  const classe = atrasada(t, hoje) ? " is-atrasada" : t.data === hoje && t.status !== "feito" ? " is-hoje" : "";
  return (
    <span className={`ficha${classe}`}>
      <ClockIcon className="h-3 w-3" aria-hidden="true" />
      {t.data ? rotuloDoDia(t.data, hoje) : ""}{comHora && t.hora ? `${t.data ? " · " : ""}${t.hora}` : ""}
    </span>
  );
}

export function FichaDePrioridade({ t }: { t: Tarefa }) {
  if (t.prioridade !== "alta" && t.prioridade !== "urgente") return null;
  return <span className={`ficha prioridade-${t.prioridade}`}>{t.prioridade === "urgente" ? "⚡ " : "↑ "}{ROTULO_DA_PRIORIDADE[t.prioridade]}</span>;
}

interface Props {
  t: Tarefa;
  hoje: string;
  pessoas: Pessoa[];
  aberta?: boolean;
  esconderData?: boolean;
  onAbrir: (id: string) => void;
  onAlternar: (t: Tarefa) => void;
  arrastando?: boolean;
  /** Começa o arrasto com o ponteiro (`arrastar.ts`). */
  onIniciarArrasto?: (e: PointerEvent<HTMLElement>, id: string) => void;
  /** Falso logo depois de soltar: o clique do fim do arrasto não abre o cartão. */
  cliqueValido?: () => boolean;
  /** Teclas extras (⌥← / ⌥→ no quadro). */
  onTecla?: (e: KeyboardEvent<HTMLElement>, t: Tarefa) => void;
}

export function CartaoDaTarefa({ t, hoje, pessoas, aberta, esconderData, onAbrir, onAlternar, arrastando, onIniciarArrasto, cliqueValido, onTecla }: Props) {
  const cat = categoria(t.categoria);
  const { feitos, total } = progressoDoChecklist(t);
  const responsavel = pessoas.find((p) => p.id === t.responsavel);
  const feita = t.status === "feito";
  const destacada = (useContext(DestaquesContext)[t.id] ?? 0) > Date.now();
  return (
    <div role="button" tabIndex={0}
      className={`tarefa-cartao${destacada ? " is-destacada" : ""}${onIniciarArrasto ? " is-arrastavel" : ""}${feita ? " is-feita" : ""}${arrastando ? " is-arrastando" : ""}${aberta ? " is-aberta" : ""}`}
      style={{ "--cor-cat": cat.cor } as CSSProperties}
      onPointerDown={onIniciarArrasto ? (e) => onIniciarArrasto(e, t.id) : undefined}
      onClick={() => { if (!cliqueValido || cliqueValido()) onAbrir(t.id); }}
      onKeyDown={(e) => { if (e.key === "Enter") onAbrir(t.id); else if (e.key === " ") { e.preventDefault(); onAlternar(t); } else onTecla?.(e, t); }}
      data-tarefa={t.id}>
      <BotaoDeConcluir feita={feita} titulo={t.titulo} onClick={() => onAlternar(t)} />
      <div className="min-w-0 flex-1">
        <div className="tarefa-cartao-titulo">{t.titulo}</div>
        {t.descricao && <div className="tarefa-cartao-desc">{t.descricao}</div>}
        <div className="tarefa-cartao-fichas">
          {!esconderData && <FichaDeData t={t} hoje={hoje} />}
          {esconderData && t.hora && <span className="ficha"><ClockIcon className="h-3 w-3" aria-hidden="true" />{t.hora}</span>}
          <FichaDePrioridade t={t} />
          <span className="ficha" title={cat.rotulo}>{cat.emoji} {cat.rotulo.split(" ")[0]}</span>
          {total > 0 && <span className={`ficha${feitos === total ? " is-hoje" : ""}`}>☑ {feitos}/{total}</span>}
          {t.comentarios.length > 0 && <span className="ficha"><ChatIcon className="h-3 w-3" aria-hidden="true" />{t.comentarios.length}</span>}
          {t.recorrencia !== "nunca" && <span className="ficha" title="Repete">↻</span>}
          <span className="flex-1" />
          <Avatar pessoa={responsavel} />
        </div>
      </div>
    </div>
  );
}
