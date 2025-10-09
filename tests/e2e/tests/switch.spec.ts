import { test, expect } from "@playwright/test";

test("switch renders and updates when discriminant changes", async ({
  page,
}) => {
  await page.goto("http://localhost:5173/");

  const container = page.locator("#switchTest");
  const valueDiv = container.locator(".container > div").first();
  const toggleBtn = container.getByRole("button", { name: "Toggle" });

  await expect(valueDiv).toHaveText("A1");

  await toggleBtn.click();
  await expect(valueDiv).toHaveText("B1");

  await toggleBtn.click();
  await expect(valueDiv).toHaveText("100");

  await toggleBtn.click();
  await expect(valueDiv).toHaveText("A1");
});
