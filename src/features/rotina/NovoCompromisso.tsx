// Fork (Google Agenda): "Novo compromisso" no topo da Rotina — cria direto no Google Agenda pessoal e o
// compromisso aparece em Hoje e na Semana logo em seguida (o engine sincroniza depois de criar).
import { useState } from "react";
import { Button } from "../../components/ui";
import { hojeLocal } from "./dominio";
import { criarCompromisso, type NovoCompromisso as Dados } from "./googleAgendaService";

const VAZIO = (): Dados => ({ titulo: "", data: hojeLocal(), inicio: "", fim: "", local: "", notas: "" });

export function NovoCompromisso({ onCriado, onErro }: { onCriado: () => void; onErro: (e: string) => void }) {
  const [aberto, setAberto] = useState(false);
  const [c, setC] = useState<Dados>(VAZIO);
  const [ocupado, setOcupado] = useState(false);
  const [feito, setFeito] = useState<string | null>(null);
  const mudar = (m: Partial<Dados>) => setC((x) => ({ ...x, ...m }));

  if (!aberto) {
    return <>
      <Button size="sm" variant="ghost" onClick={() => { setAberto(true); setFeito(null); }}>Novo compromisso</Button>
      {feito && <span className="rotina-muted" role="status">{feito}</span>}
    </>;
  }
  const salvar = async () => {
    setOcupado(true);
    try {
      await criarCompromisso(c);
      setFeito(`“${c.titulo.trim()}” está no seu Google Agenda.`);
      setC(VAZIO()); setAberto(false); onCriado();
    } catch (e) { onErro(e instanceof Error ? e.message : "Não consegui criar o compromisso."); } finally { setOcupado(false); }
  };
  return (
    <form className="rotina-form rotina-novo-compromisso" aria-label="Novo compromisso no Google Agenda"
      onSubmit={(e) => { e.preventDefault(); void salvar(); }}>
      <label className="rotina-form-largo">Compromisso<input value={c.titulo} required maxLength={200} autoFocus onChange={(e) => mudar({ titulo: e.target.value })} /></label>
      <label>Data<input type="date" value={c.data} required onChange={(e) => mudar({ data: e.target.value })} /></label>
      <label>Início<input type="time" value={c.inicio} onChange={(e) => mudar({ inicio: e.target.value })} /></label>
      <label>Fim<input type="time" value={c.fim} disabled={!c.inicio} onChange={(e) => mudar({ fim: e.target.value })} /></label>
      <label className="rotina-form-largo">Local<input value={c.local} maxLength={300} onChange={(e) => mudar({ local: e.target.value })} /></label>
      <span className="rotina-muted">Sem início = dia inteiro; sem fim = 1 hora.</span>
      <Button size="sm" type="submit" disabled={ocupado || !c.titulo.trim() || !c.data}>{ocupado ? "Criando…" : "Criar no Google"}</Button>
      <Button size="sm" variant="ghost" type="button" disabled={ocupado} onClick={() => setAberto(false)}>Cancelar</Button>
    </form>
  );
}
