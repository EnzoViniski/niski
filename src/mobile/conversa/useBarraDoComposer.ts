// Fork: estado da lista do `/` dentro do Composer — separado para o campo mudar pouco.
import { useEffect, useMemo, useState, type KeyboardEvent } from "react";
import { barraNoCursor, filtrarItens, type ItemDaBarra } from "./comandosDaBarra";

export function useBarraDoComposer({ texto, onTexto, buscarComandos, focar, enviar }: {
  texto: string;
  onTexto: (valor: string) => void;
  buscarComandos?: () => Promise<ItemDaBarra[]>;
  /** Devolve o foco ao campo com o cursor na posição dada. */
  focar: (cursor: number) => void;
  /** Enter com o comando já completo (`/compact`): fecha a lista e envia. */
  enviar: () => void;
}) {
  const [busca, setBusca] = useState<string | null>(null);
  const [todos, setTodos] = useState<ItemDaBarra[]>([]);
  const [escolhida, setEscolhida] = useState(0);

  // Carrega quando a lista abre (o hook de fora guarda por alguns segundos).
  const aberta = busca !== null;
  useEffect(() => {
    if (!aberta || !buscarComandos) return;
    let cancelado = false;
    buscarComandos().then((lista) => { if (!cancelado) setTodos(lista); }).catch(() => { if (!cancelado) setTodos([]); });
    return () => { cancelado = true; };
  }, [aberta, buscarComandos]);

  const itens = useMemo(() => (busca === null ? [] : filtrarItens(todos, busca)), [todos, busca]);
  useEffect(() => setEscolhida(0), [busca]);

  function acompanhar(valor: string, cursor: number) {
    const achada = buscarComandos ? barraNoCursor(valor, cursor) : null;
    setBusca(achada ? achada.busca : null);
  }

  function escolher(item: ItemDaBarra) {
    const fim = 1 + (busca?.length ?? 0);
    const novo = item.insere + texto.slice(fim);
    onTexto(novo);
    setBusca(null);
    focar(item.insere.length);
  }

  /** `true` quando a tecla foi da lista (não deve seguir para o campo/envio). */
  function tecla(event: KeyboardEvent<HTMLTextAreaElement>): boolean {
    if (!aberta || itens.length === 0) return false;
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      const passo = event.key === "ArrowDown" ? 1 : -1;
      setEscolhida((atual) => (atual + passo + itens.length) % itens.length);
      return true;
    }
    if ((event.key === "Enter" && !event.shiftKey && !event.nativeEvent.isComposing) || event.key === "Tab") {
      event.preventDefault();
      const item = itens[escolhida];
      // Já está escrito por inteiro e não pede argumento: Enter manda, como nas CLIs.
      if (event.key === "Enter" && item.insere === texto.trim()) { setBusca(null); enviar(); return true; }
      escolher(item);
      return true;
    }
    if (event.key === "Escape") { event.preventDefault(); setBusca(null); return true; }
    return false;
  }

  return { aberta: aberta && itens.length > 0, itens, escolhida, setEscolhida, acompanhar, escolher, tecla, fechar: () => setBusca(null) };
}
