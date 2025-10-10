import {
  NestedFor,
  Counter,
  run,
  Component,
  UseTest,
  IfTest,
  SwitchTest,
} from "./tests.ts";

document.querySelector<HTMLDivElement>("#nestedFor")!.append(
  run(NestedFor, (get, set) => {
    return {
      foos: ["foo1"],
      bars: ["bar1"],
      addBar: () => {
        const testInput = get();
        set({
          ...testInput,
          bars: [...testInput.bars, `bar${testInput.bars.length + 1}`],
        });
      },
      addFoo: () => {
        const testInput = get();
        set({
          ...testInput,
          foos: [...testInput.foos, `foo${testInput.foos.length + 1}`],
        });
      },
    };
  })
);

document.querySelector<HTMLDivElement>("#component")!.append(
  run(Component, (outerGet, outerSet) => {
    return {
      counter: () =>
        run(Counter, (innerGet, innerSet) => ({
          count: 0,
          increment: () => {
            const input = innerGet();
            innerSet({ ...input, count: input.count + 1 });
            const outerInput = outerGet();
            outerSet({
              ...outerInput,
              total: outerInput.total + 1,
            });
          },
          decrement: () => {
            const input = innerGet();
            innerSet({ ...input, count: input.count - 1 });
            const outerInput = outerGet();
            outerSet({
              ...outerInput,
              total: outerInput.total - 1,
            });
          },
        })),
      total: 0,
    };
  })
);

document.querySelector<HTMLDivElement>("#useView")!.append(
  run(UseTest, (get, set) => {
    return {
      counter0: {
        count: 0,
        increment: () => {
          const input = get();
          set({
            ...input,
            counter0: { ...input.counter0, count: input.counter0.count + 1 },
          });
        },
        decrement: () => {
          const input = get();
          set({
            ...input,
            counter0: { ...input.counter0, count: input.counter0.count - 1 },
          });
        },
      },
      counter1: {
        count: 0,
        increment: () => {
          const input = get();
          set({
            ...input,
            counter1: { ...input.counter1, count: input.counter1.count + 1 },
          });
        },
        decrement: () => {
          const input = get();
          set({
            ...input,
            counter1: { ...input.counter1, count: input.counter1.count - 1 },
          });
        },
      },
    };
  })
);

document.querySelector<HTMLDivElement>("#ifTest")!.append(
  run(IfTest, (get, set) => ({
    show: true,
    toggle: () => {
      set({ ...get(), show: !get().show });
    },
    a: { b: { c: "abc" } },
    x: { y: { z: "xyz" } },
  }))
);

document.querySelector<HTMLDivElement>("#switchTest")!.append(
  run(SwitchTest, (get, set) => ({
    toggleHandler: () => {
      const cur = get();
      const ex = cur.example as any;
      if (ex.type === "a") {
        set({ ...cur, example: { type: "b", bar: "B1" } });
      } else if (ex.type === "b") {
        set({ ...cur, example: { type: "c", baz: 100 } });
      } else {
        set({ ...cur, example: { type: "a", foo: "A1" } });
      }
    },
    example: { type: "a" as const, foo: "A1" },
  }))
);
