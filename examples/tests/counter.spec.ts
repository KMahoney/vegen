import { test, expect } from "@playwright/test";

test.describe("Counter App", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("http://localhost:5173/#counter");
  });

  test("should start with count of 0", async ({ page }) => {
    await expect(page.getByTestId("counter-button")).toContainText(
      "Clicked 0 times"
    );
  });

  test("should increment counter when button is clicked", async ({ page }) => {
    const button = page.getByTestId("counter-button");

    await expect(button).toContainText("Clicked 0 times");

    await button.click();
    await expect(button).toContainText("Clicked 1 times");

    await button.click();
    await expect(button).toContainText("Clicked 2 times");

    await button.click();
    await expect(button).toContainText("Clicked 3 times");
  });
});
