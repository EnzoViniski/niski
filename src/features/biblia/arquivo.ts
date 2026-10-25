// Fork (Bíblia do dia): carrega o texto (`biblia_ler`) e, na primeira vez, baixa (`biblia_baixar`).
// Fica na memória enquanto o app está aberto; fechar a leitura não descarta.
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useSyncExternalStore } from "react";
import { lerBiblia, type Biblia } from "./biblia";

export type EstadoDoArquivo =
  | { fase: "parado" }
  | { fase: "abrindo" }
  | { fase: "baixando" }
  | { fase: "pronta"; biblia: Biblia }
  | { fase: "erro"; erro: string };

let estado: EstadoDoArquivo = { fase: "parado" };
const ouvintes = new Set<() => void>();
function mudar(novo: EstadoDoArquivo) { estado = novo; ouvintes.forEach((o) => o()); }

/** Abre (e baixa, se faltar). Só um pedido por vez; "Tentar de novo" chama outra vez depois do erro. */
export async function abrirBiblia(): Promise<void> {
  if (estado.fase === "abrindo" || estado.fase === "baixando" || estado.fase === "pronta") return;
  mudar({ fase: "abrindo" });
  try {
    let texto = await invoke<string | null>("biblia_ler");
    if (!texto) {
      mudar({ fase: "baixando" });
      texto = await invoke<string>("biblia_baixar");
    }
    mudar({ fase: "pronta", biblia: lerBiblia(texto) });
  } catch (e) {
    mudar({ fase: "erro", erro: e instanceof Error ? e.message : String(e) });
  }
}

export function useBiblia(): EstadoDoArquivo {
  const atual = useSyncExternalStore((o) => { ouvintes.add(o); return () => ouvintes.delete(o); }, () => estado);
  useEffect(() => { if (atual.fase === "parado") void abrirBiblia(); }, [atual.fase]);
  return atual;
}

/** Só para testes. */
export function definirBibliaParaTestes(biblia: Biblia | null) { mudar(biblia ? { fase: "pronta", biblia } : { fase: "parado" }); }
