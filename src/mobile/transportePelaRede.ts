// Fork (Rotina e Telegram no celular): leva um pedido no formato do IPC do PC ({status, body}) pela
// rede do celular pareado (`request`, com o token de dispositivo).
import { ApiError, request } from "./api";

export async function pelaRede(metodo: "GET" | "POST", caminho: string, corpo: string | null): Promise<{ status: number; body: string }> {
  try {
    const valor = await request<unknown>(caminho, { method: metodo, ...(corpo === null ? {} : { headers: { "Content-Type": "application/json" }, body: corpo }) });
    return { status: 200, body: JSON.stringify(valor) };
  } catch (erro) {
    if (erro instanceof ApiError) return { status: erro.status, body: JSON.stringify({ error: erro.message }) };
    throw erro;
  }
}
