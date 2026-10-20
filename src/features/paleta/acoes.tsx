import { createContext, useCallback, useContext, useEffect, useId, useMemo, useRef, useState, type ReactNode } from "react";

export interface AcaoDaPaleta { id: string; titulo: string; executar: () => unknown }
const Contexto = createContext<{
  acoes: AcaoDaPaleta[];
  registrar: (grupo: string, acoes: AcaoDaPaleta[]) => () => void;
}>({ acoes: [], registrar: () => () => undefined });

/** Os componentes publicam seus próprios handlers; a paleta não reproduz a lógica dos botões. */
export function AcoesDaPaleta({ children }: { children: ReactNode }) {
  const [grupos, setGrupos] = useState<Record<string, AcaoDaPaleta[]>>({});
  const registrar = useCallback((grupo: string, acoes: AcaoDaPaleta[]) => {
    setGrupos((atuais) => ({ ...atuais, [grupo]: acoes }));
    return () => setGrupos((atuais) => { const copia = { ...atuais }; delete copia[grupo]; return copia; });
  }, []);
  const valor = useMemo(() => ({ acoes: Object.values(grupos).flat(), registrar }), [grupos, registrar]);
  return <Contexto.Provider value={valor}>{children}</Contexto.Provider>;
}

export function useAcoesDaPaleta(grupo: string, acoes: AcaoDaPaleta[]) {
  const { registrar } = useContext(Contexto);
  const instancia = useId();
  grupo = `${grupo}-${instancia}`;
  const atuais = useRef(acoes);
  atuais.current = acoes;
  const assinatura = JSON.stringify(acoes.map(({ id, titulo }) => [id, titulo]));
  useEffect(() => registrar(grupo, atuais.current.map(({ id, titulo }) => ({
    id, titulo, executar: () => atuais.current.find((acao) => acao.id === id)?.executar(),
  }))), [grupo, assinatura, registrar]);
}

export function useListaDeAcoes() {
  const { acoes } = useContext(Contexto);
  return useMemo(() => [...new Map(acoes.map((acao) => [acao.id, acao])).values()], [acoes]);
}
