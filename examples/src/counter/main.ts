import { Counter, run } from "../views";

export function runCounter() {
  return run(Counter, (update) => ({
    clickHandler: () => {
      update((current) => ({
        ...current,
        count: current.count + 1,
      }));
    },
    count: 0,
  }));
}
