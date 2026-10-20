// Fork (Onda 2.0): o `@` do composer saiu do `MobileApp.tsx` sem mudança de lógica.
import { useCallback } from "react";
import { request } from "../api";

/** `@` no campo: arquivos da pasta do projeto, filtrados pelo engine. */
export function useMencoes(conversaId: string) {
  return useCallback(async (busca: string) => {
    const resposta = await request<{ arquivos: string[] }>(
      `/conversas/${encodeURIComponent(conversaId)}/arquivos?q=${encodeURIComponent(busca)}`);
    return Array.isArray(resposta?.arquivos) ? resposta.arquivos : [];
  }, [conversaId]);
}
