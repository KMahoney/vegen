import { Todo, run, type TodoInput } from "../views";

type Todo = {
  id: string;
  text: string;
  completed: boolean;
};

export function runTodo() {
  let nextId = 1;
  return run(Todo, (get, set) => {
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

        set({
          ...currentState,
          todos: updatedTodos,
          newTodoText: "",
        });
      }
    };

    const initialState: TodoInput = {
      todos: [],
      newTodoText: "",

      totalCount: (todos) => {
        return todos.length;
      },

      completedCount: (todos) => {
        return todos.filter((t: Todo) => t.completed).length;
      },

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

        set({
          ...currentState,
          todos: updatedTodos,
        });
      },

      deleteHandler: (todoId) => (_event: Event) => {
        const currentState = get();

        const updatedTodos = currentState.todos.filter(
          (todo: Todo) => todo.id !== todoId
        );

        set({
          ...currentState,
          todos: updatedTodos,
        });
      },
    };

    return initialState;
  });
}
