import { invoke } from "@tauri-apps/api/core";

/**
 * Manda exceções não tratadas da interface para o log em disco do app (`app.log`). Só o tipo do
 * erro, a mensagem e o arquivo/linha de origem — nunca o estado da tela nem texto de conversa.
 * O lado Rust ainda tira trechos entre aspas e corta o tamanho.
 */
export function detalheDoErro(erro: unknown, origem?: string): string {
  const base =
    erro instanceof Error ? `${erro.name}: ${erro.message}` : typeof erro === "string" ? erro : "erro sem mensagem";
  return (origem ? `${base} @ ${origem}` : base).slice(0, 300);
}

export function instalarRegistroDeErros(alvo: Window = window): void {
  const enviar = (evento: string, detalhe: string) => {
    void invoke("registrar_evento_da_ui", { evento, detalhe }).catch(() => undefined);
  };
  alvo.addEventListener("error", (evento) => {
    const origem = evento.filename ? `${evento.filename.split("/").pop()}:${evento.lineno}` : undefined;
    enviar("erro_nao_tratado", detalheDoErro(evento.error ?? evento.message, origem));
  });
  alvo.addEventListener("unhandledrejection", (evento) => {
    enviar("promessa_rejeitada", detalheDoErro(evento.reason));
  });
}
