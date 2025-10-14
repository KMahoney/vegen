import { Todo, run, type TodoInput } from "../views";

type Todo = {
  id: string;
  text: string;
  completed: boolean;
};

export function runTodo() {
  let nextId = 1;
  return run(Todo, (update) => {
    const addTodo = () => {
      update((current) => {
        const todoText = current.newTodoText.trim();
        if (!todoText) return current;

        const newTodo: Todo = {
          id: String(nextId++),
          text: todoText,
          completed: false,
        };
        const updatedTodos = [...current.todos, newTodo];
        return {
          ...current,
          todos: updatedTodos,
          newTodoText: "",
        };
      });
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
        const target = event.target as HTMLInputElement;
        update((current) => ({
          ...current,
          newTodoText: target.value,
        }));
      },

      handleKeyPress: (event: KeyboardEvent) => {
        if (event.key === "Enter") {
          addTodo();
        }
      },

      toggleHandler: (todoId: string) => () => {
        update((current) => {
          const updatedTodos = current.todos.map((todo: Todo) =>
            todo.id === todoId
              ? {
                  ...todo,
                  completed: !todo.completed,
                }
              : todo
          );
          return {
            ...current,
            todos: updatedTodos,
          };
        });
      },

      deleteHandler: (todoId) => () => {
        update((current) => {
          const updatedTodos = current.todos.filter(
            (todo: Todo) => todo.id !== todoId
          );
          return {
            ...current,
            todos: updatedTodos,
          };
        });
      },
    };

    return initialState;
  });
}
