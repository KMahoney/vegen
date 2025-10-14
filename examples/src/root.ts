import { runCafe } from "./cafe/main";
import { runCounter } from "./counter/main";
import "./style.css";
import { runTodo } from "./todo/main";
import { Root, run } from "./views";

const components: { [example: string]: () => Element } = {
  cafe: runCafe,
  counter: runCounter,
  todo: runTodo,
};

const component: () => Element =
  components[document.location.hash.slice(1)] ?? runCafe;

function runRoot() {
  return run(Root, (update) => {
    return {
      runCafe,
      runCounter,
      runTodo,
      component,
      setExample: (name) => () => {
        document.location.hash = name;
        update((s) => ({
          ...s,
          component: components[name] ?? runCafe,
        }));
      },
    };
  });
}

document.getElementById("app")!.append(runRoot());
