import { runCafe } from "./cafe/main";
import { runCounter } from "./counter/main";
import "./style.css";
import { runTodo } from "./todo/main";
import { Root, type View, run } from "./views";

function wrapView(view: () => Element): View<{}> {
  return () => {
    const root = view();
    return { root, update: (_: {}) => {} };
  };
}

const components: { [example: string]: View<{}> } = {
  cafe: wrapView(runCafe),
  counter: wrapView(runCounter),
  todo: wrapView(runTodo),
};

const component: View<{}> =
  components[document.location.hash.slice(1)] ?? components["cafe"];

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
          component: components[name] ?? components["cafe"],
        }));
      },
    };
  });
}

document.getElementById("app")!.append(runRoot());
