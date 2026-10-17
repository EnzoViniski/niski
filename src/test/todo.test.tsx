import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import { TodoList, useTodos } from "../features/todo/TodoList";

function Lista() {
  return <TodoList {...useTodos()} />;
}

describe("TodoList", () => {
  beforeEach(() => localStorage.clear());

  it("adiciona, conclui, limpa e lembra depois de reabrir", () => {
    const { unmount } = render(<Lista />);
    const campo = screen.getByLabelText("Nova tarefa");
    fireEvent.change(campo, { target: { value: "revisar PR" } });
    fireEvent.submit(campo);
    fireEvent.change(campo, { target: { value: "  " } });
    fireEvent.submit(campo);
    expect(screen.getAllByRole("checkbox")).toHaveLength(1);

    unmount();
    render(<Lista />);
    fireEvent.click(screen.getByRole("checkbox"));
    expect(screen.getByRole("checkbox")).toBeChecked();
    fireEvent.click(screen.getByRole("button", { name: "Limpar 1 concluída" }));
    expect(screen.queryByRole("checkbox")).not.toBeInTheDocument();
  });
});
