import { act } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

// user-event e RTL resolvem versões distintas de @testing-library/dom neste projeto.
// Envolver a interação inteira garante que também suas atualizações assíncronas terminem em act.
function envolver<T extends unknown[], R>(acao: (...args: T) => Promise<R>) {
  return async (...args: T): Promise<R> => {
    let resultado!: R;
    await act(async () => { resultado = await acao(...args); });
    return resultado;
  };
}
export default {
  click: envolver(userEvent.click),
  clear: envolver(userEvent.clear),
  type: envolver(userEvent.type),
  selectOptions: envolver(userEvent.selectOptions),
};
