import React from "react";
import ReactDOM from "react-dom/client";
import { App } from "./App";
import "./index.css";
import "./styles/fork.css";
import { usarTransporte } from "./mobile/api";
import { transporteDoEngine } from "./features/conversa/transporteDoEngine";
import { instalarRegistroDeErros } from "./lib/registroDeErros";

instalarRegistroDeErros();

// A visão Conversa das abas de agente usa o chat da web; aqui ele fala com o engine pelo protocolo
// local em vez de HTTP.
usarTransporte(transporteDoEngine);

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
