// Fork: nomes e cores moram em `plataformasDeAgente.ts`; aqui ficam as duas funções de sempre.
import { nomeDaPlataforma, plataforma } from "./plataformasDeAgente";

export function corDoAgente(provider: string | null | undefined): string {
  return plataforma(provider)?.cor || "rgb(var(--c-accent))";
}

export function nomeDoAgente(provider: string | null | undefined): string {
  return nomeDaPlataforma(provider) || provider || "Terminal";
}
