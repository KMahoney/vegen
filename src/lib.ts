function h<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  props: Partial<HTMLElementTagNameMap[K]> = {},
  children: Node[] = [],
  dataset: Record<string, string> = {}
): HTMLElementTagNameMap[K] {
  const element = document.createElement(tag);
  Object.assign(element, props);
  Object.assign(element.dataset, dataset);
  element.append(...children);
  return element;
}
const t = (s: string) => document.createTextNode(s);

// Builtin functions
function numberToString(value: number): string {
  return value.toString();
}
function boolean<T>(value: boolean, t: T, f: T): T {
  return value ? t : f;
}
function lookup<V>(m: { [k: string]: V }, k: string, d: V) {
  return m[k] ?? d;
}

type ViewState<Input> = {
  root: any;
  update: (input: Input) => void;
};
type View<Input> = (input: Input) => ViewState<Input>;
function updateForLoop<Input>({
  anchor,
  prevStates,
  nextInputs,
  subView,
}: {
  anchor: Comment;
  prevStates: ViewState<Input>[];
  nextInputs: Input[];
  subView: View<Input>;
}) {
  const parent = anchor.parentNode!;
  let states = prevStates.slice();

  // Remove extra items (from end, working backwards)
  while (states.length > nextInputs.length) {
    const removed = states.pop()!;
    parent.removeChild(removed.root);
  }

  // Update existing items
  for (let i = 0; i < Math.min(states.length, nextInputs.length); i++) {
    states[i].update(nextInputs[i]);
  }

  // Add new items (insert before anchor)
  for (let i = states.length; i < nextInputs.length; i++) {
    const state = subView(nextInputs[i]);
    parent.insertBefore(state.root, anchor);
    states.push(state);
  }

  return states;
}
export function run<Input>(
  view: View<Input>,
  buildComponent: (
    get: () => Input,
    set: (stateUpdater: Input | ((current: Input) => Input)) => void
  ) => Input
): Element {
  let state: ViewState<Input>;
  let currentInput: Input;

  const get = () => currentInput;

  const set = (stateUpdater: Input | ((current: Input) => Input)) => {
    currentInput =
      typeof stateUpdater === "function"
        ? (stateUpdater as (current: Input) => Input)(currentInput)
        : stateUpdater;
    state.update(currentInput);
  };

  // Build the initial input and state
  currentInput = buildComponent(get, set);
  state = view(currentInput);

  return state.root;
}
