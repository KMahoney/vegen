import { test, expect } from "@playwright/test";

test("nested for functionality", async ({ page }) => {
  await page.goto("http://localhost:5173");

  // Check initial rendering: foo1 - bar1
  await expect(page.locator("#foo1bar1")).toBeVisible();

  // Click Add Foo button
  await page.click('button:has-text("Add Foo")');
  await expect(page.locator("#foo2bar1")).toBeVisible();

  // Click Add Bar button
  await page.click('button:has-text("Add Bar")');
  await expect(page.locator("#foo1bar2")).toBeVisible();
  await expect(page.locator("#foo2bar2")).toBeVisible();
});
