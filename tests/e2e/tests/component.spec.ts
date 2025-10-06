import { test, expect } from "@playwright/test";

test("component two counters update and aggregate total correctly", async ({
  page,
}) => {
  await page.goto("http://localhost:5173/");

  const component = page.locator("#component");

  const displays = component.locator(".grid .display");
  await expect(displays).toHaveCount(2);
  await expect(displays.nth(0)).toHaveText(/Count:\s*0/);
  await expect(displays.nth(1)).toHaveText(/Count:\s*0/);

  const total = component.locator(".highlight");
  await expect(total).toHaveText(/Total Count:\s*0/);

  const incButtons = component.locator(
    '.grid .controls button:has-text("Increment")'
  );
  const decButtons = component.locator(
    '.grid .controls button:has-text("Decrement")'
  );

  // Increment first counter 3x
  await incButtons.nth(0).click();
  await incButtons.nth(0).click();
  await incButtons.nth(0).click();

  await expect(displays.nth(0)).toHaveText(/Count:\s*3/);
  await expect(displays.nth(1)).toHaveText(/Count:\s*0/);
  await expect(total).toHaveText(/Total Count:\s*3/);

  // Increment second counter 2x
  await incButtons.nth(1).click();
  await incButtons.nth(1).click();

  await expect(displays.nth(0)).toHaveText(/Count:\s*3/);
  await expect(displays.nth(1)).toHaveText(/Count:\s*2/);
  await expect(total).toHaveText(/Total Count:\s*5/);

  // Decrement first counter 1x
  await decButtons.nth(0).click();

  await expect(displays.nth(0)).toHaveText(/Count:\s*2/);
  await expect(displays.nth(1)).toHaveText(/Count:\s*2/);
  await expect(total).toHaveText(/Total Count:\s*4/);
});
