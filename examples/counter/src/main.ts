import "./style.css";
import { counter, run } from "./counter.ts";

const root = document.querySelector<HTMLDivElement>("#app")!;

root.append(
  run(counter, (get, set) => ({
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
