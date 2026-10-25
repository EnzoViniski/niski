// Fork (Caderno): a estante — cada caderno é uma capa (cor, elástico, etiqueta com o nome). Criar,
// arquivar e mandar para a lixeira (a lixeira guarda tudo em `cadernos/.lixeira/`).
import { useState, type FormEvent } from "react";
import { Button, Input, SegmentedControl } from "../../components/ui";
import { EscolhaDaIa, textoDoErro } from "../roadmaps/comuns";
import type { IaEscolhida } from "../roadmaps/tipos";
import { criarCaderno, mandarCadernoParaALixeira, mudarCaderno } from "./loja";
import { CAPAS, SOBRE_O_TIPO, type Letra, type MetaDoCaderno, type Papel, type TipoDeCaderno } from "./tipos";

export const ROTULO_DO_PAPEL: Record<Papel, string> = { pautado: "Pautado", quadriculado: "Quadriculado", pontilhado: "Pontilhado", liso: "Liso" };
export const ROTULO_DA_LETRA: Record<Letra, string> = { manuscrita: "À mão", serifa: "Livro", normal: "Normal" };

export function Estante({ lista, onAbrir }: { lista: MetaDoCaderno[]; onAbrir: (id: string) => void }) {
  const [filtro, setFiltro] = useState<"ativos" | "arquivados">("ativos");
  const [criando, setCriando] = useState(false);
  const [erro, setErro] = useState("");
  const visiveis = lista.filter((c) => (filtro === "ativos" ? !c.arquivado : c.arquivado));
  const ativos = lista.filter((c) => !c.arquivado).length;

  return (
    <div className="caderno-estante h-full overflow-y-auto px-6 py-5 [scrollbar-gutter:stable]">
      <div className="mx-auto w-full max-w-[1100px] space-y-5">
        <header className="flex flex-wrap items-center gap-3">
          <div className="flex-1 min-w-[12rem]">
            <h1 className="caderno-estante-titulo">Cadernos</h1>
            <p className="text-xs text-text-muted">Livros, estudos, a vida, qualquer coisa. A IA lê junto e conversa com você.</p>
          </div>
          <SegmentedControl value={filtro} onChange={setFiltro} grow={false}
            options={[{ value: "ativos", label: `Na estante (${ativos})` }, { value: "arquivados", label: `Arquivados (${lista.length - ativos})` }]} />
          <Button variant="primary" size="sm" onClick={() => setCriando(true)}>Novo caderno</Button>
        </header>
        {erro && <p role="alert" className="text-xs text-danger">{erro}</p>}
        {criando && <NovoCaderno onCancelar={() => setCriando(false)} onCriado={(id) => { setCriando(false); onAbrir(id); }} />}
        {visiveis.length === 0 && !criando && (
          <div className="glass p-8 text-center text-sm text-text-secondary">
            {filtro === "ativos"
              ? <>Nenhum caderno ainda. <button className="text-accent underline" onClick={() => setCriando(true)}>Crie o primeiro</button>: um livro que você está lendo é um ótimo começo.</>
              : "Nada arquivado."}
          </div>
        )}
        <ul className="caderno-prateleira" aria-label="Cadernos">
          {visiveis.map((c) => (
            <li key={c.id} className="caderno-item">
              <button type="button" className="caderno-capa" style={{ "--capa": c.capa } as React.CSSProperties} onClick={() => onAbrir(c.id)} aria-label={`Abrir ${c.nome}`}>
                <span className="caderno-capa-elastico" aria-hidden="true" />
                <span className="caderno-capa-etiqueta">
                  <span className="caderno-capa-tipo">{SOBRE_O_TIPO[c.tipo].emoji} {SOBRE_O_TIPO[c.tipo].nome}</span>
                  <strong className="caderno-capa-nome">{c.nome}</strong>
                  {c.tipo === "livro" && c.autor && <span className="caderno-capa-autor">{c.autor}</span>}
                </span>
                <span className="caderno-capa-rodape">{c.paginas.length} {c.paginas.length === 1 ? "página" : "páginas"} · {new Date(c.atualizadoEm).toLocaleDateString("pt-BR", { day: "numeric", month: "short" })}</span>
              </button>
              <div className="caderno-item-acoes">
                <button type="button" className="caderno-mini" onClick={() => void mudarCaderno(c.id, { arquivado: !c.arquivado }).catch((e) => setErro(textoDoErro(e)))}>
                  {c.arquivado ? "Voltar para a estante" : "Arquivar"}
                </button>
                {c.arquivado && (
                  <button type="button" className="caderno-mini is-perigo" onClick={() => {
                    if (window.confirm(`Mandar "${c.nome}" para a lixeira dos cadernos? Os arquivos ficam guardados em cadernos/.lixeira.`))
                      void mandarCadernoParaALixeira(c.id).catch((e) => setErro(textoDoErro(e)));
                  }}>Lixeira</button>
                )}
              </div>
            </li>
          ))}
        </ul>
      </div>
    </div>
  );
}

function NovoCaderno({ onCancelar, onCriado }: { onCancelar: () => void; onCriado: (id: string) => void }) {
  const [nome, setNome] = useState("");
  const [tipo, setTipo] = useState<TipoDeCaderno>("livro");
  const [autor, setAutor] = useState("");
  const [capa, setCapa] = useState<string>(CAPAS[0]);
  const [papel, setPapel] = useState<Papel>("pautado");
  const [letra, setLetra] = useState<Letra>("manuscrita");
  const [ia, setIa] = useState<IaEscolhida>({ cli: "claude" });
  const [erro, setErro] = useState("");
  const [criando, setCriando] = useState(false);

  const criar = (e: FormEvent) => {
    e.preventDefault();
    if (!nome.trim() || criando) return;
    setCriando(true);
    void criarCaderno({ nome, tipo, autor: tipo === "livro" ? autor.trim() : "", capa, papel, letra, ia })
      .then((meta) => onCriado(meta.id))
      .catch((er) => { setErro(textoDoErro(er)); setCriando(false); });
  };

  return (
    <form className="glass space-y-4 p-5" onSubmit={criar} aria-label="Novo caderno">
      <SegmentedControl label="Tipo" value={tipo} onChange={setTipo}
        options={(Object.keys(SOBRE_O_TIPO) as TipoDeCaderno[]).map((t) => ({ value: t, label: `${SOBRE_O_TIPO[t].emoji} ${SOBRE_O_TIPO[t].nome}` }))}
        description={SOBRE_O_TIPO[tipo].dica} />
      <div className="grid gap-3 sm:grid-cols-2">
        <label className="space-y-1 text-sm text-text-primary">
          <span>{tipo === "livro" ? "Nome do livro" : "Nome do caderno"}</span>
          <Input autoFocus value={nome} onChange={(e) => setNome(e.target.value)} placeholder={tipo === "livro" ? "Ex.: O Poder do Hábito" : tipo === "vida" ? "Ex.: Diário 2026" : "Ex.: Cálculo I"} />
        </label>
        {tipo === "livro" && (
          <label className="space-y-1 text-sm text-text-primary">
            <span>Autor (opcional)</span>
            <Input value={autor} onChange={(e) => setAutor(e.target.value)} placeholder="Ex.: Charles Duhigg" />
          </label>
        )}
      </div>
      <div className="space-y-1">
        <span className="text-sm text-text-primary">Capa</span>
        <div className="flex flex-wrap gap-2" role="radiogroup" aria-label="Cor da capa">
          {CAPAS.map((c) => (
            <button key={c} type="button" role="radio" aria-checked={capa === c} aria-label={`Capa ${c}`} onClick={() => setCapa(c)}
              className={`caderno-amostra${capa === c ? " is-escolhida" : ""}`} style={{ background: c }} />
          ))}
        </div>
      </div>
      <div className="grid gap-3 sm:grid-cols-2">
        <SegmentedControl label="Papel" value={papel} onChange={setPapel} options={(Object.keys(ROTULO_DO_PAPEL) as Papel[]).map((p) => ({ value: p, label: ROTULO_DO_PAPEL[p] }))} />
        <SegmentedControl label="Letra" value={letra} onChange={setLetra} options={(Object.keys(ROTULO_DA_LETRA) as Letra[]).map((l) => ({ value: l, label: ROTULO_DA_LETRA[l] }))} />
      </div>
      <details className="text-sm text-text-secondary">
        <summary className="cursor-pointer">IA do caderno</summary>
        <div className="pt-2"><EscolhaDaIa valor={ia} onChange={setIa} /></div>
      </details>
      {erro && <p role="alert" className="text-xs text-danger">{erro}</p>}
      <div className="flex justify-end gap-2">
        <Button variant="ghost" onClick={onCancelar}>Cancelar</Button>
        <Button variant="primary" type="submit" disabled={!nome.trim() || criando}>Criar caderno</Button>
      </div>
    </form>
  );
}
