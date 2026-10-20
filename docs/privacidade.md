# Privacidade da distribuição

O Medza é distribuído como código e binários. As pastas de dados da pessoa não entram no repositório nem no instalador.

Ficam fora: tokens do engine, pareamentos do celular, credenciais e configurações das CLIs, contas registradas, workspace, conversas, arquivo de transcripts, imagens coladas, memórias pessoais, logs, chaves de assinatura e modelos de voz baixados. Exemplos de teste são sintéticos.

O contexto pessoal começa vazio e desativado; cada pessoa escolhe o que salvar em Configurações → Memória. O acesso remoto começa desligado. Na primeira execução, o app pode reconhecer CLIs que a própria pessoa já tem instaladas; o login pertence à CLI oficial.

Whisper: o modelo é baixado do Hugging Face. Depois disso, a captura e transcrição acontecem localmente, sem enviar áudio para uma API. Ao enviar uma mensagem, seu texto segue para o provedor de IA da conta escolhida.

Histórico: Claude/Codex gravam os próprios transcripts. O Medza arquiva cópias locais dos que observa e protege o índice com snapshots. Isso não substitui backup externo nem recupera arquivos apagados antes de haver cópia. Retenção/limpeza do arquivo local e indicador de saúde ainda estão no roadmap técnico.
