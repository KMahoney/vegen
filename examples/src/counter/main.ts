import { Counter, run } from "../views";

export function runCounter() {
  return run(Counter, (get, set) => ({
    clickHandler: () => {
      const currentState = get();
      set({
        ...currentState,
        count: currentState.count + 1,
      });
    },
    count: 0,
  }));
}
