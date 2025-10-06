import "./style.css";
import { todo, run, type TodoInput } from "./todo.ts";

type Todo = {
  id: string;
  text: string;
  completed: boolean;
};

let nextId = 1;

document.querySelector<HTMLDivElement>("#app")!.append(
  run(todo, (get, set) => {
    // Helper function to add a todo
    const addTodo = () => {
      const currentState = get();
      const todoText = currentState.newTodoText.trim();

      if (todoText) {
        const newTodo: Todo = {
          id: String(nextId++),
          text: todoText,
          completed: false,
        };

        const updatedTodos = [...currentState.todos, newTodo];
        const completedCount = updatedTodos.filter(
          (t: Todo) => t.completed
        ).length;

        set({
          ...currentState,
          todos: updatedTodos,
          newTodoText: "",
          totalCount: updatedTodos.length,
          completedCount: completedCount,
        });
      }
    };

    const initialState: TodoInput = {
      todos: [] as Todo[],
      newTodoText: "",
      totalCount: 0,
      completedCount: 0,

      addTodoHandler: () => {
        addTodo();
      },

      updateNewTodoText: (event: Event) => {
        const currentState = get();
        const target = event.target as HTMLInputElement;
        set({
          ...currentState,
          newTodoText: target.value,
        });
      },

      handleKeyPress: (event: KeyboardEvent) => {
        if (event.key === "Enter") {
          addTodo();
        }
      },

      toggleHandler: (todoId: string) => (_event: Event) => {
        const currentState = get();

        const updatedTodos = currentState.todos.map((todo: Todo) =>
          todo.id === todoId
            ? {
                ...todo,
                completed: !todo.completed,
              }
            : todo
        );

        const completedCount = updatedTodos.filter(
          (t: Todo) => t.completed
        ).length;

        set({
          ...currentState,
          todos: updatedTodos,
          completedCount: completedCount,
        });
      },

      deleteHandler: (todoId) => (_event: Event) => {
        const currentState = get();

        const updatedTodos = currentState.todos.filter(
          (todo: Todo) => todo.id !== todoId
        );
        const completedCount = updatedTodos.filter(
          (t: Todo) => t.completed
        ).length;

        set({
          ...currentState,
          todos: updatedTodos,
          totalCount: updatedTodos.length,
          completedCount: completedCount,
        });
      },
    };

    return initialState;
  })
);
