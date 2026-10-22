// Fork (Tarefas): número de tarefas minhas para hoje (abertas, incluindo atrasadas) ao lado de "Tarefas" na lateral.
import { aberta, diaDe } from "./dominio";
import { useTarefas } from "./loja";

export function ContadorDeHoje() {
  const { tarefas, ajustes } = useTarefas();
  const hoje = diaDe(new Date());
  const eu = ajustes.eu.id;
  const n = tarefas.filter((t) => aberta(t) && t.data && t.data <= hoje && (!eu || !t.responsavel || t.responsavel === eu)).length;
  if (!n) return null;
  return <span className="ml-auto rounded-full bg-accent/15 px-1.5 text-[11px] font-semibold text-accent" aria-label={`${n} para hoje`}>{n}</span>;
}
