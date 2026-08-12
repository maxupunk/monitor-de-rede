# Diretrizes do Projeto para Agentes IA

> **As diretrizes vivem em [`AGENTS.md`](../AGENTS.md), na raiz do repositório.**
> Leia aquele arquivo.

Este arquivo já foi uma segunda cópia das diretrizes. As duas divergiram: a da
raiz acompanhou a migração para o backend Rust e esta continuou mandando rodar
`npx tsc --noEmit` e `node ace test`, com uma seção inteira sobre Japa. Enquanto
isso durou, qual das duas um agente lia era sorte — e metade das vezes a sorte
mandava rodar o comando de um backend que não existe mais.

Por isso virou ponteiro em vez de ser reescrito: duas cópias das mesmas regras
voltam a divergir, e o custo do erro recai em quem confiou no arquivo errado.
