import { test, expect } from "@playwright/test";

test("nestedFor adds rows/cols and renders all combinations", async ({
  page,
}) => {
  await page.goto("http://localhost:5173/");

  const section = page.locator("#nestedFor");
  const cards = section.locator(".card");

  // Initial state: 1x1 => 1 card
  await expect(cards).toHaveCount(1);
  await expect(section.locator("#foo1bar1")).toBeVisible();
  await expect(section.locator("#foo1bar1")).toHaveText("foo1 - bar1");

  const addFoo = page.locator('button:has-text("Add Foo")');
  const addBar = page.locator('button:has-text("Add Bar")');

  // Add a foo: 2x1 => 2 cards
  await addFoo.click();
  await expect(cards).toHaveCount(2);
  await expect(section.locator("#foo2bar1")).toBeVisible();
  await expect(section.locator("#foo2bar1")).toHaveText("foo2 - bar1");

  // Add a bar: 2x2 => 4 cards
  await addBar.click();
  await expect(cards).toHaveCount(4);
  await expect(section.locator("#foo1bar2")).toBeVisible();
  await expect(section.locator("#foo2bar2")).toBeVisible();
  await expect(section.locator("#foo1bar2")).toHaveText("foo1 - bar2");
  await expect(section.locator("#foo2bar2")).toHaveText("foo2 - bar2");

  // Add another bar: 2x3 => 6 cards
  await addBar.click();
  await expect(cards).toHaveCount(6);
  await expect(section.locator("#foo1bar3")).toBeVisible();
  await expect(section.locator("#foo2bar3")).toBeVisible();
  await expect(section.locator("#foo1bar3")).toHaveText("foo1 - bar3");
  await expect(section.locator("#foo2bar3")).toHaveText("foo2 - bar3");

  // Add another foo: 3x3 => 9 cards
  await addFoo.click();
  await expect(cards).toHaveCount(9);
  await expect(section.locator("#foo3bar1")).toBeVisible();
  await expect(section.locator("#foo3bar2")).toBeVisible();
  await expect(section.locator("#foo3bar3")).toBeVisible();
  await expect(section.locator("#foo3bar1")).toHaveText("foo3 - bar1");
  await expect(section.locator("#foo3bar2")).toHaveText("foo3 - bar2");
  await expect(section.locator("#foo3bar3")).toHaveText("foo3 - bar3");
});
