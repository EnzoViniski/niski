import { useEffect, useState } from "react";
import { CheckIcon, CloseIcon } from "../../components/ui/PixelIcon";

/** Uma tarefa sua — não do agente (essas moram no Kanban). */
export interface Todo {
  id: string;
  texto: string;
  feita: boolean;
}

const CHAVE = "omni-todo";

function lerTodos(): Todo[] {
  try {
    const salvos = JSON.parse(localStorage.getItem(CHAVE) ?? "[]");
    // Fork: item inválido sai sozinho, não a lista inteira (nem quebra a tela).
    return Array.isArray(salvos)
      ? salvos.filter((todo): todo is Todo => Boolean(todo) && typeof todo.id === "string" && typeof todo.texto === "string" && typeof todo.feita === "boolean")
      : [];
  } catch {
    return [];
  }
}

/** Lista de tarefas pessoal, a mesma em todos os workspaces e projetos. */
export function useTodos() {
  const [todos, setTodos] = useState<Todo[]>(lerTodos);
  useEffect(() => {
    try { localStorage.setItem(CHAVE, JSON.stringify(todos)); } catch { /* sem armazenamento: vale só nesta sessão */ }
  }, [todos]);
  return {
    todos,
    pendentes: todos.filter((todo) => !todo.feita).length,
    adicionar: (texto: string) => setTodos((atual) => [...atual, { id: `todo-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`, texto, feita: false }]),
    alternar: (id: string) => setTodos((atual) => atual.map((todo) => (todo.id === id ? { ...todo, feita: !todo.feita } : todo))),
    remover: (id: string) => setTodos((atual) => atual.filter((todo) => todo.id !== id)),
    limparFeitas: () => setTodos((atual) => atual.filter((todo) => !todo.feita)),
  };
}

export function TodoList({ todos, adicionar, alternar, remover, limparFeitas }: ReturnType<typeof useTodos>) {
  const [texto, setTexto] = useState("");
  // Pendentes em cima; as feitas descem riscadas até você limpar.
  const ordenadas = [...todos.filter((todo) => !todo.feita), ...todos.filter((todo) => todo.feita)];
  const feitas = todos.length - todos.filter((todo) => !todo.feita).length;

  return (
    <div className="space-y-1 px-1 pt-0.5">
      <form
        onSubmit={(evento) => {
          evento.preventDefault();
          const limpo = texto.trim();
          if (!limpo) return;
          adicionar(limpo);
          setTexto("");
        }}
      >
        <input
          type="text"
          value={texto}
          onChange={(evento) => setTexto(evento.target.value)}
          placeholder="Nova tarefa… (Enter)"
          aria-label="Nova tarefa"
          className="w-full border border-border-subtle bg-overlay/[0.04] px-2 py-1.5 text-xs text-text-primary placeholder:text-text-muted outline-none focus-visible:border-accent"
        />
      </form>
      <ul className="space-y-1">
        {ordenadas.map((todo) => (
          <li key={todo.id}
            className={`lateral-item group flex items-stretch border-l-2 ring-1 ring-inset ${
              todo.feita ? "border-transparent ring-border-subtle/30" : "border-accent ring-border-subtle/40 hover:ring-border-subtle"
            }`}>
            <label className="flex min-w-0 flex-1 cursor-pointer items-start gap-2 px-2 py-1.5">
              <input type="checkbox" checked={todo.feita} onChange={() => alternar(todo.id)} className="peer sr-only" />
              <span aria-hidden className={`mt-px grid h-3.5 w-3.5 shrink-0 place-items-center border peer-focus-visible:outline peer-focus-visible:outline-2 peer-focus-visible:outline-accent ${
                todo.feita ? "border-success bg-success/20 text-success" : "border-border text-transparent"
              }`}>
                <CheckIcon className="h-2.5 w-2.5" />
              </span>
              <span className={`min-w-0 flex-1 break-words text-xs leading-4 ${todo.feita ? "text-text-muted line-through" : "text-text-primary"}`}>
                {todo.texto}
              </span>
            </label>
            <button
              type="button"
              aria-label={`Apagar tarefa ${todo.texto}`}
              title="Apagar"
              onClick={() => remover(todo.id)}
              className="grid w-6 shrink-0 place-items-center text-text-muted opacity-0 hover:text-danger group-hover:opacity-60 group-focus-within:opacity-60 focus-visible:opacity-100 focus-visible:outline focus-visible:outline-2 focus-visible:outline-accent"
            >
              <CloseIcon className="h-3 w-3" aria-hidden />
            </button>
          </li>
        ))}
      </ul>
      {feitas > 0 && (
        <button type="button" onClick={limparFeitas}
          className="px-1 text-[10px] text-text-muted hover:text-text-primary focus-visible:outline focus-visible:outline-2 focus-visible:outline-accent">
          Limpar {feitas} {feitas === 1 ? "concluída" : "concluídas"}
        </button>
      )}
    </div>
  );
}
