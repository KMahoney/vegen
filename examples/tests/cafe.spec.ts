import { test, expect, type Page } from "@playwright/test";

test.describe("Cafe App", () => {
  const selectors = {
    cafeApp: '[data-testid="cafe-app"]',
    grandTotal: '[data-testid="grand-total"]',
    totalQty: '[data-testid="total-qty"]',
    subtotalAmount: '[data-testid="subtotal-amount"]',
    menuCategoryCoffee: '[data-testid="menu-category-Coffee"]',
    menuCategoryTea: '[data-testid="menu-category-Tea"]',
    pickupBtn: '[data-testid="pickup-btn"]',
    dineinBtn: '[data-testid="dinein-btn"]',
    deliveryBtn: '[data-testid="delivery-btn"]',
    tableInput: '[data-testid="table-input"]',
    addressInput: '[data-testid="address-input"]',
  };

  // Helper functions for common operations
  const addItem = async (page: Page, itemName: string) => {
    await page.locator(`[data-testid="add-${itemName}"]`).click();
  };

  const removeItem = async (page: Page, itemName: string) => {
    await page.locator(`[data-testid="remove-${itemName}"]`).click();
  };

  const verifyCartItem = async (
    page: Page,
    itemName: string,
    qty: number,
    total: string
  ) => {
    await expect(
      page
        .locator(selectors.cafeApp)
        .getByText(`${itemName} × ${qty} — ${total}`)
    ).toBeVisible();
  };

  test.beforeEach(async ({ page }) => {
    await page.goto("http://localhost:5173/#cafe");
  });

  test("should start with empty cart", async ({ page }) => {
    await expect(page.locator(selectors.grandTotal)).toHaveText(
      "Grand Total: £0.00"
    );
    await expect(page.locator(selectors.totalQty)).toHaveText("0");
  });

  test("should add items to cart using increment button", async ({ page }) => {
    await addItem(page, "Espresso");
    await expect(page.locator(`[data-testid="item-Espresso"]`)).toBeVisible();
    await expect(page.locator(selectors.grandTotal)).toHaveText(
      "Grand Total: £3.00"
    );
  });

  test("should not decrement below zero", async ({ page }) => {
    await removeItem(page, "Espresso");
    await removeItem(page, "Espresso");

    // Should remain empty
    await expect(page.locator(selectors.grandTotal)).toHaveText(
      "Grand Total: £0.00"
    );
  });

  test("should handle multiple different items", async ({ page }) => {
    // Add espresso from coffee menu
    await addItem(page, "Espresso");

    // Add black tea from tea menu
    await addItem(page, "Black Tea");

    // Check cart shows both items
    await verifyCartItem(page, "Espresso", 1, "£3.00");
    await verifyCartItem(page, "Black Tea", 1, "£2.50");

    // Check totals: 3.00 + 2.50 = 5.50
    await expect(page.locator(selectors.totalQty)).toHaveText("2");
    await expect(page.locator(selectors.subtotalAmount)).toHaveText("£5.50");
    await expect(page.locator(selectors.grandTotal)).toHaveText(
      "Grand Total: £5.50"
    );
  });

  test("should handle order type selection", async ({ page }) => {
    await expect(page.locator(selectors.pickupBtn)).toBeVisible();
    await expect(page.locator(selectors.dineinBtn)).toBeVisible();
    await expect(page.locator(selectors.deliveryBtn)).toBeVisible();

    // Switch to dine in
    await page.locator(selectors.dineinBtn).click();
    await expect(page.locator(selectors.tableInput)).toBeVisible();

    // Switch to delivery
    await page.locator(selectors.deliveryBtn).click();
    await expect(page.locator(selectors.addressInput)).toBeVisible();

    // Switch back to pickup
    await page.locator(selectors.pickupBtn).click();
    await expect(page.locator(selectors.tableInput)).not.toBeVisible();
    await expect(page.locator(selectors.addressInput)).not.toBeVisible();
  });

  test("should allow entering table number for dine in", async ({ page }) => {
    await page.locator(selectors.dineinBtn).click();

    const tableInput = page.locator(selectors.tableInput);
    await expect(tableInput).toBeVisible();

    await tableInput.fill("Table 12");
    await expect(tableInput).toHaveValue("Table 12");
  });

  test("should allow entering delivery address", async ({ page }) => {
    await page.locator(selectors.deliveryBtn).click();

    const addressInput = page.locator(selectors.addressInput);
    await expect(addressInput).toBeVisible();

    await addressInput.fill("123 Main St");
    await expect(addressInput).toHaveValue("123 Main St");
  });
});
