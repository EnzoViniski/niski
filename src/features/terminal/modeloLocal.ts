export type ExecucaoCodex = "conta" | "ollama" | "lmstudio";

/** Só monta a chamada da CLI oficial. Não instala servidor, não baixa modelo e não altera conta. */
export function comandoCodexLocal(binario: string, provedor: ExecucaoCodex, modelo: string): string {
  if (provedor === "conta") return binario;
  const nome = modelo.trim();
  if (!/^[a-zA-Z0-9][a-zA-Z0-9._:/+@-]{0,199}$/.test(nome)) {
    throw new Error("Informe o nome do modelo instalado, usando letras, números, ponto, barra, dois-pontos ou hífen.");
  }
  return `${binario} --oss --local-provider ${provedor} --model ${nome}`;
}
