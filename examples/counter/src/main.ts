import "./style.css";
import { Counter, run } from "./counter.ts";

const root = document.querySelector<HTMLDivElement>("#app")!;

root.append(
  run(Counter, (get, set) => ({
    clickHandler: () => {
      const currentState = get();
      set({
        ...currentState,
        count: currentState.count + 1,
      });
    },
    count: 0,
  }))
);
