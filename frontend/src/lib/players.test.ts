import { describe, expect, it } from "vitest";
import { mySide, myUsernames, rememberChoice, rememberUsername } from "./players";

function memory() {
  const data = new Map<string, string>();
  return {
    getItem: (k: string) => data.get(k) ?? null,
    setItem: (k: string, v: string) => void data.set(k, v),
  };
}

describe("players", () => {
  it("remembers the chosen side's username and preselects it later", () => {
    const store = memory();
    expect(myUsernames(store)).toEqual([]);
    rememberChoice({ white: "Alice", black: "Bob" }, "black", store);
    expect(myUsernames(store)).toEqual(["bob"]);
    expect(mySide({ white: " BOB ", black: "Carol" }, myUsernames(store))).toBe("white");
    expect(mySide({ white: "Dave", black: "bob" }, myUsernames(store))).toBe("black");
    expect(mySide({ white: "Dave", black: "Erin" }, myUsernames(store))).toBeNull();
  });

  it("forgets a name the user says is not them", () => {
    const store = memory();
    rememberChoice({ white: "Alice", black: "Bob" }, "white", store);
    rememberChoice({ white: "Alice", black: "Bob" }, null, store);
    expect(myUsernames(store)).toEqual([]);
  });

  it("does not guess when both players match", () => {
    expect(mySide({ white: "Bob", black: "bob" }, ["bob"])).toBeNull();
    expect(mySide({ white: null, black: null }, ["bob"])).toBeNull();
  });

  it("survives broken or missing storage", () => {
    expect(myUsernames({ getItem: () => "{not json", setItem: () => {} })).toEqual([]);
    expect(myUsernames(null)).toEqual([]);
    expect(() => rememberChoice({ white: "A", black: "B" }, "white", null)).not.toThrow();
  });

  it("remembers a username the user fetched their games with", () => {
    const store = memory();
    rememberUsername(" Khhizr7 ", store);
    rememberUsername("khhizr7", store);
    expect(myUsernames(store)).toEqual(["khhizr7"]);
    expect(mySide({ white: "bob", black: "KHHIZR7" }, myUsernames(store))).toBe("black");
  });
});
