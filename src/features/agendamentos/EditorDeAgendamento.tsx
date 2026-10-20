import { useState, type FormEvent } from "react";
import type { Conversation } from "../../mobile/api";
import { resolverDataLocal, salvarAgendamento, type Agendamento, type Destino, type Opcoes, type Pedido, type Regra } from "./agendamentosService";
const DIAS = ["Seg", "Ter", "Qua", "Qui", "Sex", "Sáb", "Dom"];
export interface Rascunho { conversa: string; titulo: string; prompt: string }
export function EditorDeAgendamento({ existente, inicial, opcoes, conversas, onSalvar, onCancelar }: {
  existente?: Agendamento; inicial?: Rascunho; opcoes: Opcoes; conversas: Conversation[]; onSalvar: () => void; onCancelar: () => void;
}) {
  const [titulo, setTitulo] = useState(existente?.titulo ?? inicial?.titulo ?? "");
  const [prompt, setPrompt] = useState(existente?.prompt ?? inicial?.prompt ?? "");
  const [tipo, setTipo] = useState<Destino["tipo"]>(existente?.destino.tipo ?? (inicial ? "conversa" : "novo_agente"));
  const [conversa, setConversa] = useState(existente?.destino.tipo === "conversa" ? existente.destino.conversa : inicial?.conversa ?? "");
  const conversaExistente = existente?.destino.tipo === "conversa" ? existente.destino.conversa : undefined;
  const novo = existente?.destino.tipo === "novo_agente" ? existente.destino : undefined;
  const [projeto, setProjeto] = useState(novo?.projeto ?? opcoes.projetos[0]?.id ?? "");
  const [pasta, setPasta] = useState(novo?.pasta ?? opcoes.projetos[0]?.path ?? "");
  const [plataforma, setPlataforma] = useState(novo?.plataforma ?? opcoes.plataformas[0]?.id ?? "claude");
  const [conta, setConta] = useState(novo?.conta ?? "");
  const [modelo, setModelo] = useState(novo?.modelo ?? "");
  const [worktree, setWorktree] = useState(novo?.worktree ?? false);
  const [frequencia, setFrequencia] = useState<Regra["tipo"]>(existente?.regra.tipo ?? "uma_vez");
  const [data, setData] = useState("");
  const [intervalo, setIntervalo] = useState(existente?.regra.tipo === "intervalo" ? existente.regra.minutos : 60);
  const [unidade, setUnidade] = useState(1);
  const [horario, setHorario] = useState(existente?.regra.tipo === "diario" || existente?.regra.tipo === "semanal" ? existente.regra.horario : "09:00");
  const [dias, setDias] = useState(existente?.regra.tipo === "semanal" ? existente.regra.dias : [0, 1, 2, 3, 4]);
  const [salvando, setSalvando] = useState(false);
  const [erro, setErro] = useState<string | null>(null);
  async function salvar(e: FormEvent) {
    e.preventDefault(); setSalvando(true); setErro(null);
    try {
      let regra: Regra;
      if (frequencia === "uma_vez") {
        // Conversão no engine, inclusive DST. Uma data única já salva conserva o instante ao editar.
        const em_ms = data ? (await resolverDataLocal(data)).em_ms : existente?.regra.tipo === "uma_vez" ? existente.regra.em_ms : NaN;
        if (!Number.isFinite(em_ms)) throw new Error("Escolha a data e hora");
        regra = { tipo: "uma_vez", em_ms };
      } else if (frequencia === "intervalo") regra = { tipo: "intervalo", minutos: intervalo * unidade };
      else if (frequencia === "diario") regra = { tipo: "diario", horario };
      else { if (!dias.length) throw new Error("Escolha pelo menos um dia"); regra = { tipo: "semanal", horario, dias }; }
      const destino: Destino = tipo === "conversa" ? { tipo: "conversa", conversa } : { tipo: "novo_agente", projeto, pasta, plataforma, conta: conta || null, modelo: modelo.trim() || null, worktree };
      const pedido: Pedido = { ...(existente ? { id: existente.id } : {}), titulo, prompt, regra, destino };
      await salvarAgendamento(pedido); onSalvar();
    } catch (e) { setErro(e instanceof Error ? e.message : String(e)); } finally { setSalvando(false); }
  }
  return <form className="agenda-editor" onSubmit={e => void salvar(e)}>
    <h3>{existente ? "Editar agendamento" : "Criar agendamento"}</h3>
    <fieldset disabled={salvando}>
      <label>Título<input required maxLength={120} value={titulo} onChange={e => setTitulo(e.target.value)} /></label>
      <label>Prompt<textarea required rows={4} value={prompt} onChange={e => setPrompt(e.target.value)} /></label>
      <small>Somente o texto do prompt é agendado; anexos ficam na conversa.</small>
      <label>Destino<select value={tipo} onChange={e => setTipo(e.target.value as Destino["tipo"])}><option value="conversa">Esta conversa</option><option value="novo_agente">Abrir agente novo</option></select></label>
      {tipo === "conversa" ? <label>Conversa<select required value={conversa} onChange={e => setConversa(e.target.value)}>
        <option value="">Escolha a conversa</option>
        {inicial && !conversas.some(c => c.id === inicial.conversa) && <option value={inicial.conversa}>{inicial.titulo}</option>}
        {conversaExistente && !conversas.some(c => c.id === conversaExistente) && <option value={conversaExistente}>{conversaExistente}</option>}
        {conversas.map(c => <option key={c.id} value={c.id}>{c.title}</option>)}
      </select></label> : <>
        <label>Projeto<select required value={projeto} onChange={e => { setProjeto(e.target.value); setPasta(opcoes.projetos.find(p => p.id === e.target.value)?.path ?? ""); }}>
          <option value="">Escolha o projeto</option>{opcoes.projetos.map(p => <option key={p.id} value={p.id}>{p.name}</option>)}
        </select></label>
        <label>Pasta<input required value={pasta} onChange={e => setPasta(e.target.value)} /><small>Dentro do projeto selecionado.</small></label>
        <div className="agenda-colunas">
          <label>Plataforma<select value={plataforma} onChange={e => { setPlataforma(e.target.value); setConta(""); setModelo(""); }}>
            {opcoes.plataformas.map(p => <option key={p.id} value={p.id}>{p.label}</option>)}
          </select></label>
          <label>Conta<select value={conta} onChange={e => setConta(e.target.value)}><option value="">Padrão da plataforma</option>
            {opcoes.contas.filter(c => c.provider === plataforma).map(c => <option key={c.id} value={c.id}>{c.name}</option>)}
            <option disabled value="automatica">Automática (em breve)</option>
          </select></label>
        </div>
        <label>Modelo (opcional)<input value={modelo} onChange={e => setModelo(e.target.value)} placeholder="Padrão da CLI" /></label>
        <label className="agenda-check"><input type="checkbox" checked={worktree} onChange={e => setWorktree(e.target.checked)} />Isolar em worktree</label>
      </>}
      <label>Frequência<select value={frequencia} onChange={e => setFrequencia(e.target.value as Regra["tipo"])}>
        <option value="uma_vez">Uma vez</option><option value="intervalo">A cada intervalo</option><option value="diario">Diário</option><option value="semanal">Dias da semana</option>
      </select></label>
      {frequencia === "uma_vez" && <label>Data e hora<input type="datetime-local" required={existente?.regra.tipo !== "uma_vez"} value={data} onChange={e => setData(e.target.value)} />
        {existente?.regra.tipo === "uma_vez" && <small>Sem alteração, mantém {new Date(existente.regra.em_ms).toLocaleString("pt-BR")}.</small>}</label>}
      {frequencia === "intervalo" && <div className="agenda-colunas"><label>A cada<input required type="number" min={unidade === 1 ? 5 : 1} max={525600 / unidade} step="1" value={intervalo} onChange={e => setIntervalo(Number(e.target.value))} /></label>
        <label>Unidade<select value={unidade} onChange={e => setUnidade(Number(e.target.value))}><option value={1}>Minutos</option><option value={60}>Horas</option></select></label></div>}
      {(frequencia === "diario" || frequencia === "semanal") && <label>Horário<input required type="time" value={horario} onChange={e => setHorario(e.target.value)} /></label>}
      {frequencia === "semanal" && <div className="agenda-dias" role="group" aria-label="Dias da semana">{DIAS.map((dia, n) => <button key={dia} type="button" aria-pressed={dias.includes(n)} onClick={() => setDias(dias.includes(n) ? dias.filter(d => d !== n) : [...dias, n])}>{dia}</button>)}</div>}
      <small>Horários no fuso local do computador do engine (agora UTC{opcoes.fuso}). No horário de verão, uma hora inexistente avança ao primeiro minuto válido; uma hora repetida roda uma vez.</small>
      {erro && <p role="alert" className="text-danger">{erro}</p>}
      <div className="agenda-acoes"><button type="button" onClick={onCancelar}>Cancelar</button><button className="agenda-primario" type="submit">{salvando ? "Salvando…" : "Salvar agendamento"}</button></div>
    </fieldset>
  </form>;
}
