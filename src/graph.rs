use std::collections::{HashMap, HashSet};
use std::hash::Hash;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cycle<K> {
    pub nodes: Vec<K>,
}

pub fn cycle_from_stack<K>(stack: &[K], repeated: &K) -> Vec<K>
where
    K: Eq + Clone,
{
    let mut cycle = stack.to_vec();
    cycle.push(repeated.clone());

    if let Some(start) = cycle.iter().position(|node| node == repeated) {
        cycle[start..].to_vec()
    } else {
        cycle
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum VisitState {
    Visiting,
    Visited,
}

pub fn topo_sort<K>(deps: &HashMap<K, HashSet<K>>) -> Result<Vec<K>, Cycle<K>>
where
    K: Eq + Hash + Clone + Ord,
{
    let mut order = Vec::new();
    let mut state = HashMap::<K, VisitState>::new();
    let mut stack = Vec::<K>::new();

    let mut nodes: Vec<K> = deps.keys().cloned().collect();
    nodes.sort();

    for node in nodes {
        visit(&node, deps, &mut state, &mut stack, &mut order)?;
    }

    Ok(order)
}

fn visit<K>(
    node: &K,
    deps: &HashMap<K, HashSet<K>>,
    state: &mut HashMap<K, VisitState>,
    stack: &mut Vec<K>,
    order: &mut Vec<K>,
) -> Result<(), Cycle<K>>
where
    K: Eq + Hash + Clone + Ord,
{
    use VisitState::{Visited, Visiting};

    match state.get(node) {
        Some(Visited) => return Ok(()),
        Some(Visiting) => {
            let cycle_start = stack.iter().position(|n| n == node).unwrap_or(0);
            let mut cycle = stack[cycle_start..].to_vec();
            cycle.push(node.clone());
            return Err(Cycle { nodes: cycle });
        }
        None => {}
    }

    state.insert(node.clone(), Visiting);
    stack.push(node.clone());

    if let Some(children) = deps.get(node) {
        let mut child_nodes: Vec<&K> = children.iter().collect();
        child_nodes.sort();

        for child in child_nodes {
            visit(child, deps, state, stack, order)?;
        }
    }

    state.insert(node.clone(), Visited);
    stack.pop();
    order.push(node.clone());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topo_sort_orders_nodes_without_cycles() {
        let mut deps: HashMap<&str, HashSet<&str>> = HashMap::new();
        deps.insert("A", HashSet::new());
        deps.insert("B", HashSet::from(["A"]));
        deps.insert("C", HashSet::from(["B"]));

        let order = topo_sort(&deps).unwrap();
        assert_eq!(order, vec!["A", "B", "C"]);
    }

    #[test]
    fn topo_sort_detects_cycles() {
        let mut deps: HashMap<&str, HashSet<&str>> = HashMap::new();
        deps.insert("A", HashSet::from(["B"]));
        deps.insert("B", HashSet::from(["A"]));

        let cycle = topo_sort(&deps).unwrap_err();
        assert!(cycle.nodes.len() >= 2);
        assert_eq!(cycle.nodes.first(), cycle.nodes.last());
        assert!(cycle.nodes.contains(&"A"));
        assert!(cycle.nodes.contains(&"B"));
    }

    #[test]
    fn cycle_from_stack_returns_suffix_starting_at_repeat() {
        let stack = vec!["root", "a", "b"];
        let cycle = cycle_from_stack(&stack, &"a");
        assert_eq!(cycle, vec!["a", "b", "a"]);
    }
}
