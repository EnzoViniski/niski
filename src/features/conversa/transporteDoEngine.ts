import { engineApi } from "../terminal/terminalService";

/** Arquivo → base64. `readAsDataURL` já entrega codificado, sem montar string byte a byte. */
function paraBase64(arquivo: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const leitor = new FileReader();
    leitor.onload = () => resolve(String(leitor.result).slice(String(leitor.result).indexOf(",") + 1));
    leitor.onerror = () => reject(leitor.error ?? new Error("Não foi possível ler o arquivo"));
    leitor.readAsDataURL(arquivo);
  });
}

/**
 * O `fetch` do chat, trocado no app do PC: a requisição vai pelo protocolo local do engine
 * (`EngineRequest::Api`) para as mesmas rotas que o celular usa por HTTP. Assim a visão Conversa
 * funciona com o acesso remoto desligado, e o chat é um código só nos três lugares.
 *
 * Anexo (imagem colada, arquivo escolhido) vai em base64 com os cabeçalhos que a rota lê — o nome
 * do arquivo viaja no `X-Omni-Nome`, como no celular.
 */
export async function transporteDoEngine(path: string, init: RequestInit): Promise<Response> {
  // O token de dispositivo é do HTTP do celular; aqui quem autentica é o token do engine.
  const headers = Object.entries((init.headers ?? {}) as Record<string, string>)
    .filter(([nome]) => nome.toLowerCase() !== "x-omni-token");
  const binario = init.body instanceof Blob ? init.body : null;
  const resposta = await engineApi(
    init.method ?? "GET",
    path,
    typeof init.body === "string" ? init.body : null,
    { headers, bodyBase64: binario ? await paraBase64(binario) : undefined },
  );
  // 204/304 não podem ter corpo no construtor do `Response`.
  const semCorpo = resposta.status === 204 || resposta.status === 304;
  return new Response(semCorpo ? null : resposta.body, {
    status: resposta.status,
    headers: { "content-type": "application/json" },
  });
}
