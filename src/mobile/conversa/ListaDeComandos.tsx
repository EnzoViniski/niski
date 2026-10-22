// Fork: a lista que abre acima do campo ao digitar `/` (mesmo visual da lista de arquivos do `@`).
import type { ItemDaBarra } from "./comandosDaBarra";

const ROTULO_DO_TIPO: Record<ItemDaBarra["tipo"], string> = { cli: "Comando", skill: "Skill", comando: "Personalizado" };

export function ListaDeComandos({ itens, escolhida, onEscolher, onApontar }: {
  itens: ItemDaBarra[];
  escolhida: number;
  onEscolher: (item: ItemDaBarra) => void;
  onApontar: (indice: number) => void;
}) {
  return (
    <ul id="comandos-da-barra" role="listbox" aria-label="Comandos e skills" className="composer-mencoes composer-comandos">
      {itens.map((item, indice) => (
        <li key={`${item.tipo}:${item.nome}`} role="option" aria-selected={indice === escolhida}
          className={`composer-mencao composer-comando${indice === escolhida ? " is-ativa" : ""}`}
          // `mouseDown` + `preventDefault`: o campo não perde o foco antes da escolha.
          onMouseDown={(event) => { event.preventDefault(); onEscolher(item); }}
          onMouseEnter={() => onApontar(indice)}
          ref={indice === escolhida ? (el) => el?.scrollIntoView?.({ block: "nearest" }) : undefined}>
          <span className="composer-mencao-nome">/{item.nome}</span>
          {item.descricao && <span className="composer-comando-descricao" title={item.descricao}>{item.descricao}</span>}
          <span className="composer-comando-tipo" data-tipo={item.tipo}>
            {ROTULO_DO_TIPO[item.tipo]}{item.origem === "projeto" ? " · projeto" : ""}
          </span>
        </li>
      ))}
    </ul>
  );
}
