import { test, expect, type Page } from "@playwright/test";

test.describe("Todo App", () => {
  const selectors = {
    todoInput: '[data-testid="todo-input"]',
    addButton: '[data-testid="add-todo-btn"]',
    todoList: '[data-testid="todo-list"]',
    totalCount: '[data-testid="total-count"]',
    completedCount: '[data-testid="completed-count"]',
  };

  // Helper functions for common operations
  const addTodo = async (page: Page, text: string, useEnter = false) => {
    const todoInput = page.locator(selectors.todoInput);
    const addButton = page.locator(selectors.addButton);

    await todoInput.fill(text);
    if (useEnter) {
      await todoInput.press("Enter");
    } else {
      await addButton.click();
    }
  };

  const getTodoItems = (page: Page) =>
    page.locator(selectors.todoList).locator("li");

  const getTodoCount = async (page: Page) => {
    return await getTodoItems(page).count();
  };

  const verifyCounts = async (page: Page, total: number, completed: number) => {
    await expect(page.locator(selectors.totalCount)).toHaveText(
      `Total todos: ${total}`
    );
    await expect(page.locator(selectors.completedCount)).toHaveText(
      `Completed: ${completed}`
    );
  };

  const getTodoCheckbox = (page: Page, index = 0) =>
    page.locator("li input[type='checkbox']").nth(index);

  const getTodoDeleteButton = (page: Page, index = 0) =>
    page.locator("li button:has-text('Delete')").nth(index);

  test.beforeEach(async ({ page }) => {
    await page.goto("http://localhost:5173/#todo");
  });

  test("should start with empty todo list", async ({ page }) => {
    await expect(page.locator(selectors.todoList)).toBeEmpty();
    await verifyCounts(page, 0, 0);
  });

  test("should add a new todo via button", async ({ page }) => {
    const todoInput = page.locator(selectors.todoInput);

    await addTodo(page, "Learn Playwright");

    await expect(getTodoItems(page)).toHaveCount(1);
    await expect(getTodoItems(page).first()).toContainText("Learn Playwright");
    await expect(todoInput).toHaveValue("");

    await verifyCounts(page, 1, 0);
  });

  test("should add a new todo via Enter key", async ({ page }) => {
    const todoInput = page.locator(selectors.todoInput);

    await addTodo(page, "Test with Enter key", true);

    await expect(getTodoItems(page)).toHaveCount(1);
    await expect(getTodoItems(page).first()).toContainText(
      "Test with Enter key"
    );
    await expect(todoInput).toHaveValue("");

    await verifyCounts(page, 1, 0);
  });

  test("should not add empty todos", async ({ page }) => {
    const todoInput = page.locator(selectors.todoInput);
    const addButton = page.locator(selectors.addButton);

    // Test empty string
    await todoInput.fill("");
    await addButton.click();
    await expect(getTodoItems(page)).toHaveCount(0);

    // Test whitespace only
    await todoInput.fill("   ");
    await addButton.click();
    await expect(getTodoItems(page)).toHaveCount(0);
  });

  test("should toggle todo completion status", async ({ page }) => {
    await addTodo(page, "Complete this task");

    const todoCheckbox = getTodoCheckbox(page, 0);

    // Initially not completed
    await expect(todoCheckbox).not.toBeChecked();
    await verifyCounts(page, 1, 0);

    // Mark as completed
    await todoCheckbox.check();
    await expect(todoCheckbox).toBeChecked();
    await verifyCounts(page, 1, 1);

    // Mark as not completed
    await todoCheckbox.uncheck();
    await expect(todoCheckbox).not.toBeChecked();
    await verifyCounts(page, 1, 0);
  });

  test("should delete a todo", async ({ page }) => {
    await addTodo(page, "Delete me");

    await expect(getTodoItems(page)).toHaveCount(1);

    // Click delete button
    const deleteButton = getTodoDeleteButton(page, 0);
    await deleteButton.click();

    // Todo should be removed
    await expect(getTodoItems(page)).toHaveCount(0);
    await verifyCounts(page, 0, 0);
  });

  test("should handle multiple todos correctly", async ({ page }) => {
    const todos = ["Task 1", "Task 2", "Task 3"];

    // Add multiple todos
    for (const todoText of todos) {
      await addTodo(page, todoText);
    }

    await expect(await getTodoCount(page)).toBe(3);

    // Check all todos are present
    for (let i = 0; i < todos.length; i++) {
      await expect(getTodoItems(page).nth(i)).toContainText(todos[i]);
    }

    await verifyCounts(page, 3, 0);

    // Complete first and third todos
    await getTodoCheckbox(page, 0).check();
    await getTodoCheckbox(page, 2).check();
    await verifyCounts(page, 3, 2);

    // Delete second todo
    await getTodoDeleteButton(page, 1).click();
    await expect(await getTodoCount(page)).toBe(2);
    await verifyCounts(page, 2, 2);

    // Add another todo
    await addTodo(page, "Task 4");
    await expect(await getTodoCount(page)).toBe(3);
    await verifyCounts(page, 3, 2);
  });
});
