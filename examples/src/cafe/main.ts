import { Cafe, run, type CafeInput } from "../views";

type Item = { id: string; name: string; price: number };
type Category = { name: string; items: Item[] };
type ItemCount = { [id: string]: number };

const categories: Category[] = [
  {
    name: "Coffee",
    items: [
      { id: "espresso", name: "Espresso", price: 300 },
      { id: "latte", name: "Latte", price: 350 },
    ],
  },
  {
    name: "Tea",
    items: [
      { id: "black-tea", name: "Black Tea", price: 250 },
      { id: "green-tea", name: "Green Tea", price: 280 },
    ],
  },
];

function sum(a: number[]) {
  return a.reduce((a, b) => a + b, 0);
}

export function runCafe() {
  return run(Cafe, (update) => {
    const initialState: CafeInput = {
      categories,
      cart: {
        items: {},
        decrement: (itemId) => () => {
          update((current) => {
            if ((current.cart.items[itemId] ?? 0) <= 0) return current;
            return {
              ...current,
              cart: {
                ...current.cart,
                items: {
                  ...current.cart.items,
                  [itemId]: (current.cart.items[itemId] ?? 0) - 1,
                },
              },
            };
          });
        },
        increment: (itemId) => () => {
          update((current) => ({
            ...current,
            cart: {
              ...current.cart,
              items: {
                ...current.cart.items,
                [itemId]: (current.cart.items[itemId] ?? 0) + 1,
              },
            },
          }));
        },
        getItems: function (categories, cart: ItemCount) {
          const items = Object.fromEntries(
            categories
              .flatMap((c) => c.items)
              .map((i) => [i.id, [i.name, i.price]] as const)
          );
          return Object.entries(cart)
            .map(([id, qty]) => {
              const [name, price] = items[id];
              return { qty, lineTotal: qty * price, name };
            })
            .filter((i) => i.qty > 0);
        },
        grandTotal: function (categories, cart: ItemCount) {
          const items = Object.fromEntries(
            categories.flatMap((c) => c.items).map((i) => [i.id, i.price])
          );
          return sum(
            Object.entries(cart).map(([id, qty]) => {
              const price = items[id];
              return qty * price;
            })
          );
        },
        subtotal: function (categories, cart: ItemCount) {
          const items = Object.fromEntries(
            categories.flatMap((c) => c.items).map((i) => [i.id, i.price])
          );
          return sum(
            Object.entries(cart).map(([id, qty]) => {
              const price = items[id];
              return qty * price;
            })
          );
        },
        totalQty: function (cart: ItemCount) {
          return sum(Object.values(cart));
        },
      },
      currency: function (pence: number): string {
        return (pence / 100).toLocaleString("en-GB", {
          style: "currency",
          currency: "GBP",
        });
      },
      order: {
        details: {
          type: "pickup",
        },
        selectOrder: (orderType) => () => {
          update((current) => {
            let order: CafeInput["order"]["details"];
            switch (orderType) {
              case "dinein":
                order = { type: "dinein", table: "" };
                break;
              case "delivery":
                order = { type: "delivery", address: "" };
                break;
              case "pickup":
                order = { type: "pickup" };
                break;
              default:
                order = current.order.details;
                break;
            }
            return {
              ...current,
              order: { ...current.order, details: order },
            };
          });
        },
        updateTable: (e) => {
          update((current) => ({
            ...current,
            order: {
              ...current.order,
              details: {
                type: "dinein",
                table: (e.target as HTMLInputElement).value,
              },
            },
          }));
        },
        updateAddress: (e) => {
          update((current) => ({
            ...current,
            order: {
              ...current.order,
              details: {
                type: "delivery",
                address: (e.target as HTMLInputElement).value,
              },
            },
          }));
        },
      },
    };

    return initialState;
  });
}
