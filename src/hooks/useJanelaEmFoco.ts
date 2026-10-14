import { useEffect, useState } from "react";

/**
 * A janela do app está na frente do usuário. Com o OMNI minimizado ou atrás de outra janela, a aba
 * que está ativa numa pane **não** foi vista — marcar como vista ali engolia o aviso do turno que
 * terminou enquanto você estava fora.
 */
export function useJanelaEmFoco(): boolean {
  const [emFoco, setEmFoco] = useState(() => (typeof document.hasFocus === "function" ? document.hasFocus() : true));
  useEffect(() => {
    const ganhou = () => setEmFoco(true);
    const perdeu = () => setEmFoco(false);
    window.addEventListener("focus", ganhou);
    window.addEventListener("blur", perdeu);
    return () => {
      window.removeEventListener("focus", ganhou);
      window.removeEventListener("blur", perdeu);
    };
  }, []);
  return emFoco;
}
