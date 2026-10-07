//! Ordem de criação de objetos que dependem uns dos outros (views).
//!
//! Uma view que lê outra view só pode nascer depois dela. O catálogo devolve as
//! views em ordem de criação, que costuma servir — até alguém recriar a de
//! baixo com `CREATE OR REPLACE`, e a ordem virar. Aqui a ordem é a das
//! dependências, e o empate fica na ordem original.

use std::collections::HashSet;

/// Ordena `items` para que cada um venha depois do que ele usa.
///
/// `deps(i)` devolve os índices de que o item `i` depende. Dependência fora da
/// lista é ignorada (tabela, função: já criadas antes). Ciclo — que o SGBD não
/// deixaria existir, mas um catálogo estranho poderia sugerir — não trava: o
/// que sobrar entra na ordem original.
#[must_use]
pub fn dependency_order<T>(items: Vec<T>, deps: impl Fn(usize) -> Vec<usize>) -> Vec<T> {
    let total = items.len();
    let dependencies: Vec<Vec<usize>> = (0..total)
        .map(|index| {
            deps(index)
                .into_iter()
                .filter(|&dep| dep < total && dep != index)
                .collect()
        })
        .collect();

    let mut placed: HashSet<usize> = HashSet::new();
    let mut order = Vec::with_capacity(total);
    while order.len() < total {
        let ready = (0..total).find(|index| {
            !placed.contains(index) && dependencies[*index].iter().all(|dep| placed.contains(dep))
        });
        // Ciclo: o primeiro que faltar entra assim mesmo.
        let next = ready.unwrap_or_else(|| {
            (0..total)
                .find(|index| !placed.contains(index))
                .expect("ainda há item por posicionar")
        });
        placed.insert(next);
        order.push(next);
    }

    let mut slots: Vec<Option<T>> = items.into_iter().map(Some).collect();
    order
        .into_iter()
        .filter_map(|index| slots[index].take())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quem_depende_vem_depois() {
        // "resumo" lê "ativos", que foi recriada depois dele.
        let views = vec!["resumo", "ativos", "solta"];
        let ordered = dependency_order(views, |index| if index == 0 { vec![1] } else { vec![] });
        assert_eq!(ordered, vec!["ativos", "resumo", "solta"]);
    }

    #[test]
    fn sem_dependencia_mantem_a_ordem_original() {
        let ordered = dependency_order(vec![3, 1, 2], |_| vec![]);
        assert_eq!(ordered, vec![3, 1, 2]);
    }

    #[test]
    fn ciclo_nao_trava_nem_perde_item() {
        let ordered = dependency_order(vec!["a", "b"], |index| vec![1 - index]);
        assert_eq!(ordered.len(), 2);
    }

    #[test]
    fn dependencia_fora_da_lista_e_ignorada() {
        let ordered = dependency_order(vec!["a"], |_| vec![7, 0]);
        assert_eq!(ordered, vec!["a"]);
    }
}
