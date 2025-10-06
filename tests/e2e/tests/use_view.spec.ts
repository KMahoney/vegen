import { test, expect } from "@playwright/test";

test("useView renders two independent counters that update separately", async ({
  page,
}) => {
  await page.goto("http://localhost:5173/");

  const section = page.locator("#useView");

  const displays = section.locator(".display");
  await expect(displays).toHaveCount(2);
  await expect(displays.nth(0)).toHaveText(/Count:\s*0/);
  await expect(displays.nth(1)).toHaveText(/Count:\s*0/);

  const incButtons = section.locator('.controls button:has-text("Increment")');
  const decButtons = section.locator('.controls button:has-text("Decrement")');

  // Increment first counter twice
  await incButtons.nth(0).click();
  await incButtons.nth(0).click();

  await expect(displays.nth(0)).toHaveText(/Count:\s*2/);
  await expect(displays.nth(1)).toHaveText(/Count:\s*0/);

  // Decrement second counter once
  await decButtons.nth(1).click();

  await expect(displays.nth(0)).toHaveText(/Count:\s*2/);
  await expect(displays.nth(1)).toHaveText(/Count:\s*-1/);
});
