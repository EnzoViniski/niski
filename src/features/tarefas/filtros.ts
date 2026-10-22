// Fork (Tarefas): filtros da tela — saber se algum está ligado e desligar todos (o filtro "Trabalho"
// ligado escondia a tarefa de saúde que a IA tinha acabado de criar).
import { salvarAjustes } from "./loja";
import type { AjustesDasTarefas, Tarefa } from "./tipos";

export function filtrosAtivos(ajustes: AjustesDasTarefas, busca = ""): boolean {
  return ajustes.filtroCategoria !== "" || ajustes.filtroPessoa !== "todas" || busca.trim() !== "";
}

export function limparFiltros() {
  salvarAjustes({ filtroCategoria: "", filtroPessoa: "todas" });
}

/** A tarefa some com os filtros de agora? (mesma regra de `filtrar`, sem a busca). */
export function escondidaPelosFiltros(t: Tarefa, ajustes: AjustesDasTarefas): boolean {
  if (ajustes.filtroCategoria && t.categoria !== ajustes.filtroCategoria) return true;
  if (ajustes.filtroPessoa === "minhas") return t.responsavel !== ajustes.eu.id;
  return ajustes.filtroPessoa !== "todas" && t.responsavel !== ajustes.filtroPessoa;
}
