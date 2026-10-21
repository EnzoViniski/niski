/**
 * Fork (energia): tira as linhas do terminal e do editor do reset universal de variáveis do Tailwind 3.
 *
 * O Tailwind 3 declara ~49 variáveis `--tw-*` em `*, ::before, ::after`. Cada elemento passa a ter o
 * próprio mapa de variáveis (o WebKit não compartilha mais o estilo herdado), e cada `<span>` novo
 * paga as 49. O xterm (renderizador DOM) recria spans a cada quadro enquanto um agente escreve, e o
 * Monaco faz o mesmo ao digitar: o `sample` do WebContent mostrava o tempo em
 * `applyCustomPropertyImpl`. Nenhum utilitário do Tailwind mora dentro dessas linhas, então elas não
 * precisam do reset. `:where()` mantém a especificidade zero do seletor original.
 */
import type { Plugin } from "postcss";

const SEM_RESET = ".xterm-rows *, .view-lines *";
const UNIVERSAL = new Set(["*", "::before", "::after", ":before", ":after"]);

export function variaveisForaDoTerminal(): Plugin {
  return {
    postcssPlugin: "medza-variaveis-fora-do-terminal",
    Rule(regra) {
      const seletores = regra.selectors.map((s) => s.trim());
      if (seletores.length !== 3 || !seletores.every((s) => UNIVERSAL.has(s))) return;
      if (!regra.nodes?.some((no) => no.type === "decl" && no.prop.startsWith("--tw-"))) return;
      const base = `:where(*:not(${SEM_RESET}))`;
      regra.selectors = [base, `${base}::before`, `${base}::after`];
    },
  };
}
variaveisForaDoTerminal.postcss = true;
